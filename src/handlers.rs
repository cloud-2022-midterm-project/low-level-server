use std::sync::Arc;

use crate::{
    app_state::AppState,
    request::{method::Method, Request},
    response::Response,
};

use self::{
    delete::handle_delete,
    get::{get_pagination_meta, handle_get},
    post::handle_post,
    put::handle_put,
};

mod delete;
mod get;
mod post;
mod put;

pub use get::{CompleteMessage, CompletePutUpdate, PaginationMetadata};
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

    // if GET request, spawn a new task to handle it
    if matches!(request.method(), Method::Get) {
        let uri = request.uri().trim_start_matches("/api/messages");
        match uri {
            "/" | "" => {
                tokio::spawn(async move {
                    let response = handle_get(state).await;
                    if let Err(e) = stream.write_all(response.as_bytes()).await {
                        eprintln!("Failed to send response: {}", e);
                    }
                });
            }
            "/trigger-pagination" => {
                let response = get_pagination_meta(state).await;
                if let Err(e) = stream.write_all(response.as_bytes()).await {
                    eprintln!("Failed to send response: {}", e);
                }
            }
            uri => {
                // unknown GET request
                let body = format!("GET uri not found, {}", uri);
                let response = Response::new()
                    .status_line("HTTP/1.1 404 NOT FOUND")
                    .append_header(&format!("Content-Length: {}", body.len()))
                    .append_header("Content-Type: text/plain")
                    .body(&body)
                    .to_string();
                if let Err(e) = stream.write_all(response.as_bytes()).await {
                    eprintln!("Failed to send response: {}", e);
                }
            }
        }
        return;
    }

    let response = process_request(request, state).await;

    if let Err(e) = stream.write_all(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
}

async fn process_request(request: Request, state: Arc<AppState>) -> String {
    match request.method() {
        // Method::Get => handle_get(state).await,
        Method::Post => match request.body() {
            Some(body) => handle_post(body, state).await,
            None => Response::new()
                .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                .to_string(),
        },
        Method::Put => {
            let body = match request.body() {
                Some(body) => body,
                None => {
                    return Response::new()
                        .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                        .to_string();
                }
            };
            let uuid = request.uri().trim_start_matches("/api/messages/");
            handle_put(uuid, body, state).await
        }
        Method::Delete => {
            let uuid = request.uri().trim_start_matches("/api/messages/");
            handle_delete(uuid, state).await
        }
        Method::Get => unreachable!(),
    }
}
