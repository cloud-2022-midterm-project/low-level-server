use crate::{app_state::AppState, image, mutation_manager::ServerPutUpdate, response::Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize, Serialize, Default, Debug)]
pub struct PutMessage {
    pub author: String,
    pub message: String,
    pub likes: i32,
    pub imageUpdate: bool,
    pub image: String,
}

pub async fn handle_put(uuid: &str, body: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    // check for conflicting uuid
    if !state.all_uuids.lock().await.contains(uuid) {
        return response.status_line("HTTP/1.1 404 NOT FOUND").to_string();
    }

    let payload: PutMessage = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            return response
                .status_line("HTTP/1.1 400 BAD REQUEST")
                .body(&format!("{}", e))
                .to_string();
        }
    };

    // There are 3 cases for `image_to_client`:
    // 1. No update to image, meaning the client will not get an image (null or absent in the response)
    // 2. Update image with new content, meaning the client will get the new image in the response
    // 3. Remove image, meaning the client will get an `empty` string in the response
    let mut image_to_client = None;

    let result = if payload.imageUpdate {
        if !payload.image.is_empty() {
            // update image
            if let Err(e) = image::save(&state.image_base_path, &payload.image, uuid) {
                eprintln!("Error saving image: {}", e);
                return response
                    .status_line("HTTP/1.1 500 Internal Server Error")
                    .body("Failed to save image.")
                    .to_string();
            }

            image_to_client = Some(payload.image);
            sqlx::query!(
                "UPDATE messages SET author = $1, message = $2, likes = $3, has_image = $4 WHERE uuid = $5",
                payload.author,
                payload.message,
                payload.likes,
                true,
                uuid
            )
        } else {
            // remove image
            image::remove(&state.image_base_path, uuid).ok();
            image_to_client = Some("".to_string());
            sqlx::query!(
                "UPDATE messages SET author = $1, message = $2, likes = $3, has_image = $4 WHERE uuid = $5",
                payload.author,
                payload.message,
                payload.likes,
                false,
                uuid
            )
        }
    } else {
        sqlx::query!(
            "UPDATE messages SET author = $1, message = $2, likes = $3 WHERE uuid = $4",
            payload.author,
            payload.message,
            payload.likes,
            uuid
        )
    }
    .execute(state.pool.as_ref())
    .await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 Not Found");
            } else {
                state.mutations.lock().await.add_put(
                    uuid,
                    ServerPutUpdate {
                        author: payload.author,
                        message: payload.message,
                        likes: payload.likes,
                        image: image_to_client,
                        image_updated: payload.imageUpdate,
                    },
                    &state.image_base_path,
                );
                response.set_status_line("HTTP/1.1 204 No Content");
            }
        }
        Err(_) => {
            response.set_status_line("HTTP/1.1 500 Internal Server Error");
        }
    }

    response.to_string()
}
