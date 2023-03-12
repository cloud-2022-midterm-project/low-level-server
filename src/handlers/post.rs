use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{app_state::AppState, image, response::Response};

use super::CompleteMessage;

#[derive(Deserialize, Serialize)]
pub struct PostMessage {
    uuid: String,
    author: String,
    message: Option<String>,
    likes: i32,
    imageUpdate: bool,
    image: Option<String>,
}

pub async fn handle_post(body: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    let PostMessage {
        uuid,
        author,
        message,
        likes,
        imageUpdate,
        image,
    } = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            let body = format!("{e} {body}");
            return response
                .status_line("HTTP/1.1 400 BAD REQUEST")
                .body(&body)
                .to_string();
        }
    };

    // check for conflicting uuid
    if !state.all_uuids.lock().await.insert(uuid.clone()) {
        return response.status_line("HTTP/1.1 409 CONFLICT").to_string();
    }

    if let (true, Some(image)) = (imageUpdate, &image) {
        if image::save(&state.image_base_path, image, &uuid).is_err() {
            return response
                .status_line("HTTP/1.1 500 Internal Server Error")
                .body("Failed to save image.")
                .to_string();
        }
    }

    let result = match &message {
        Some(message) => sqlx::query!(
            "INSERT INTO messages (uuid, author, message, likes, has_image) VALUES ($1, $2, $3, $4, $5)",
            &uuid,
            &author,
            &message,
            &likes,
            imageUpdate
        ),
        None => {
            sqlx::query!(
                "INSERT INTO messages (uuid, author, likes, has_image) VALUES ($1, $2, $3, $4)",
                &uuid,
                &author,
                &likes,
                imageUpdate
            )
        },
    }
    .execute(state.pool.as_ref())
    .await;

    match result {
        Ok(_) => {
            state.mutations.lock().await.add_post(CompleteMessage {
                uuid,
                author,
                message,
                likes,
                image,
            });
            response.set_status_line("HTTP/1.1 201 OK");
        }
        Err(_) => {
            response.set_status_line("HTTP/1.1 409 CONFLICT");
        }
    }

    response.to_string()
}
