use std::sync::Arc;

use crate::{app_state::AppState, image, maybe::Maybe, response::Response};

use serde::{Deserialize, Serialize};

pub enum BindValue {
    Author(String),
    Message(String),
    Likes(i32),
    HasImage(bool),
    ImageUpdate(bool),
    Image(String),
}

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct PutMessage {
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub author: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub message: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub likes: Maybe<i32>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub imageUpdate: Maybe<bool>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    pub image: Maybe<String>,
}

pub async fn handle_put(uuid: &str, body: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    let payload: PutMessage = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return response
                .status_line("HTTP/1.1 400 BAD REQUEST")
                .body(&format!("{}", e))
                .to_string();
        }
    };

    let mut command = "UPDATE messages SET ".to_string();
    let mut index = 1;

    let mut params = Vec::with_capacity(5);

    if let Maybe::Value(author) = payload.author {
        command.push_str(&format!("author = ${index}, "));
        index += 1;
        params.push(BindValue::Author(author));
    }
    if let Maybe::Value(message) = payload.message {
        command.push_str(&format!("message = ${index}, "));
        index += 1;
        params.push(BindValue::Message(message));
    }
    if let Maybe::Value(likes) = payload.likes {
        command.push_str(&format!("likes = ${index}, "));
        index += 1;
        params.push(BindValue::Likes(likes));
    }

    if let Maybe::Value(true) = payload.imageUpdate {
        params.push(BindValue::ImageUpdate(true));
        command.push_str(&format!("has_image = ${index}, "));
        index += 1;
        if let Maybe::Value(image) = payload.image {
            // update image
            if image::save(&state.image_base_path, &image, uuid).is_err() {
                return response
                    .status_line("HTTP/1.1 500 Internal Server Error")
                    .body("Failed to save image.")
                    .to_string();
            }
            params.push(BindValue::HasImage(true));
            params.push(BindValue::Image(image));
        } else {
            // remove image
            image::remove(&state.image_base_path, uuid).ok();
            params.push(BindValue::HasImage(false));
        }
    }

    if index == 1 {
        return response
            .status_line("HTTP/1.1 400 Bad Request")
            .body("No fields were provided to update.")
            .to_string();
    }

    // remove the last comma
    command.truncate(command.len() - 2);
    command.push_str(&format!(" WHERE uuid = ${index}"));

    let mut q = sqlx::query(&command);

    for param in params.iter() {
        match param {
            BindValue::Author(v) => q = q.bind(v),
            BindValue::Message(v) => q = q.bind(v),
            BindValue::Likes(v) => q = q.bind(v),
            BindValue::HasImage(v) => q = q.bind(v),
            _ => (),
        };
    }

    let result = q.bind(uuid).execute(state.pool.as_ref()).await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 Not Found");
            } else {
                state.mutations.lock().await.add_put(uuid, params);
                response.set_status_line("HTTP/1.1 204 No Content");
            }
        }
        Err(_) => {
            response.set_status_line("HTTP/1.1 500 Internal Server Error");
        }
    }

    response.to_string()
}
