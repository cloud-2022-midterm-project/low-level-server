use crate::{app_state::AppState, utils::get_content_length};

use self::{delete::handle_delete, get::handle_get, post::handle_post, put::handle_put};

mod delete;
mod get;
mod post;
mod put;

pub use put::PutMessage;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub async fn handle_request(stream: &mut TcpStream, state: &mut AppState) {
    let mut buf = [0; 1024];
    let n = stream
        .read(&mut buf)
        .await
        .expect("Failed to read from socket");
    let request = String::from_utf8_lossy(&buf[..n]).to_string();
    let response = match_request(&request, state).await;

    if let Err(e) = stream.write(response.as_bytes()).await {
        eprintln!("Failed to send response: {}", e);
    }
}

async fn match_request(request: &str, state: &mut AppState) -> String {
    let mut lines = request.lines();
    let first_line = lines.next().unwrap_or("");
    let method = first_line.split_whitespace().next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("");

    match method {
        "GET" => handle_get(state).await,
        "POST" => {
            let content_length = get_content_length(&mut lines);
            let body = &request[(request.len() - content_length)..];
            handle_post(body, state).await
        }
        "PUT" => {
            let content_length = get_content_length(&mut lines);
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
