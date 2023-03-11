use std::sync::Arc;

use crate::{
    app_state::AppState,
    request::{method::Method, Request},
    response::Response,
};

use self::{
    clear::clear,
    delete::handle_delete,
    get::{get_pagination_meta, handle_get},
    post::handle_post,
    put::handle_put,
};

mod clear;
mod delete;
mod get;
mod post;
mod put;

pub use get::{CompleteMessage, CompletePutUpdate, PaginationMetadata, PaginationType};
pub use put::{BindValue, PutMessage};
use tokio::{io::AsyncWriteExt, net::TcpStream};

pub async fn handle_connection(mut stream: TcpStream, state: Arc<AppState>) {
    let request = match Request::from_stream(&mut stream).await {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Failed to read from stream: {}", e);
            let response = Response::new()
                .status_line("HTTP/1.1 500 INTERNAL SERVER ERROR")
                .to_string();
            if let Err(e) = stream.write_all(response.as_bytes()).await {
                eprintln!("Failed to send response: {}", e);
            }
            return;
        }
    };

    let response = match request.method() {
        Method::Get => {
            let uri = request.uri().trim_start_matches("/api/messages");
            match uri {
                "" | "/" => get_pagination_meta(state).await,
                "/get-page" => handle_get(state).await,
                uri => {
                    // unknown GET request
                    let body = format!("GET uri not found, {}", uri);
                    Response::new()
                        .status_line("HTTP/1.1 404 NOT FOUND")
                        .append_header(&format!("Content-Length: {}", body.len()))
                        .append_header("Content-Type: text/plain")
                        .body(&body)
                        .to_string()
                }
            }
        }
        Method::Post => match request.body() {
            Some(body) => handle_post(body, state).await,
            None => Response::new()
                .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                .to_string(),
        },
        Method::Put => match request.body() {
            Some(body) => {
                let uuid = request.uri().trim_start_matches("/api/messages/");
                handle_put(uuid, body, state).await
            }
            None => Response::new()
                .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                .to_string(),
        },
        Method::Delete => {
            let uuid = request.uri().trim_start_matches("/api/messages/");
            handle_delete(uuid, state).await
        }
        Method::Patch => clear(state).await,
    };

    if let Err(e) = stream.write_all(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
}
