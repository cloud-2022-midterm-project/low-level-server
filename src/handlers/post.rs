use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{app_state::AppState, image, response::Response};

use super::CompleteMessage;

#[derive(Deserialize, Serialize)]
pub struct PostMessage {
    uuid: String,
    author: String,
    message: String,
    likes: i32,
    imageUpdate: bool,
    image: String,
}

pub async fn handle_post(body: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    let PostMessage {
        uuid,
        author,
        message,
        likes,
        imageUpdate,
        mut image,
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

    // if let (true, "") = (imageUpdate, image) {
    // if let Err(e) = image::save(&state.image_base_path, image, &uuid) {
    //     eprintln!("Error saving image: {}", e);
    //     return response
    //         .status_line("HTTP/1.1 500 Internal Server Error")
    //         .body("Failed to save image.")
    //         .to_string();
    // }
    // }
    if imageUpdate {
        if !image.is_empty() {
            if let Err(e) = image::save(&state.image_base_path, &image, &uuid) {
                eprintln!("Error saving image: {}", e);
                return response
                    .status_line("HTTP/1.1 500 Internal Server Error")
                    .body("Failed to save image.")
                    .to_string();
            }
        } else {
            image = String::new();
        }
    }

    let result = sqlx::query!(
        "INSERT INTO messages (uuid, author, message, likes, has_image) VALUES ($1, $2, $3, $4, $5)",
        uuid,
        author,
        message,
        likes,
        imageUpdate
    )
    .execute(state.pool.as_ref())
    .await;

    match result {
        Ok(_) => {
            state.mutations.lock().await.add_post(
                CompleteMessage {
                    uuid,
                    author,
                    message,
                    likes,
                    image,
                },
                &state.image_base_path,
                imageUpdate,
            );
            response.set_status_line("HTTP/1.1 201 OK");
        }
        Err(_) => {
            response.set_status_line("HTTP/1.1 409 CONFLICT");
        }
    }

    response.to_string()
}
