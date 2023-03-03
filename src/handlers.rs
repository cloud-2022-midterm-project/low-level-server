use crate::{app_state::AppState, response::Response, utils::read_stream_request};

use self::{delete::handle_delete, get::handle_get, post::handle_post, put::handle_put};

mod delete;
mod get;
mod post;
mod put;

pub use put::{BindValue, PutMessage};
use tokio::{io::AsyncWriteExt, net::TcpStream};

pub async fn handle_connection(stream: &mut TcpStream, state: &mut AppState) {
    let (request, content_length) = match read_stream_request(stream).await {
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
    let response = match_request(request, content_length, state).await;

    if let Err(e) = stream.write_all(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }

    if let Err(e) = stream.flush().await {
        eprintln!("Failed to flush stream: {}", e);
    }
}

async fn match_request(
    request: String,
    content_length: Option<usize>,
    state: &mut AppState,
) -> String {
    let mut lines = request.lines();
    let first_line = lines.next().unwrap_or("");
    let method = first_line.split_whitespace().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    match method {
        "GET" => handle_get(state).await,
        "POST" => {
            let content_length = match content_length {
                Some(len) => len,
                None => {
                    return Response::new()
                        .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                        .to_string();
                }
            };
            let body = &request[(request.len() - content_length)..];
            handle_post(body, state).await
        }
        "PUT" => {
            let content_length = match content_length {
                Some(len) => len,
                None => {
                    return Response::new()
                        .status_line("HTTP/1.1 411 LENGTH REQUIRED")
                        .to_string();
                }
            };
            let body = &request[(request.len() - content_length)..];
            let uuid = path.trim_start_matches("/api/messages/");
            handle_put(uuid, body, state).await
        }
        "DELETE" => {
            let uuid = path.trim_start_matches("/api/messages/");
            handle_delete(uuid, state).await
        }
        _ => "HTTP/1.1 404 NOT FOUND\r\n\r".to_string(),
    }
}
