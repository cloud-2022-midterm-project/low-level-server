use crate::{app_state::AppState, app_state::PutUpdate, maybe::Maybe, response::Response};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
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
    pub base64Image: Maybe<String>,
}

pub async fn handle_put(uuid: &str, body: &str, state: &mut AppState) -> String {
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

    enum BindValue<'a> {
        Author(&'a str),
        Message(&'a str),
        Likes(i32),
        HasImage(bool),
    }

    let mut params = Vec::with_capacity(5);

    if let Maybe::Value(author) = &payload.author {
        command.push_str(&format!("author = ${index}, "));
        index += 1;
        params.push(BindValue::Author(author));
    }
    if let Maybe::Value(message) = &payload.message {
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
        command.push_str(&format!("has_image = ${index}, "));
        index += 1;
        if let Maybe::Value(image) = &payload.base64Image {
            // update image
            state.image_store.save(image, uuid);
            params.push(BindValue::HasImage(true));
        } else {
            // remove image
            state.image_store.remove(uuid).ok();
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
        q = match param {
            BindValue::Author(v) => q.bind(v),
            BindValue::Message(v) => q.bind(v),
            BindValue::Likes(v) => q.bind(v),
            BindValue::HasImage(v) => q.bind(v),
        };
    }

    let result = q.bind(uuid).execute(state.pool.as_ref()).await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 Not Found");
            } else {
                // if there's a post update of this uuid, modify it rather than adding to updates_put
                if let Some(m) = state.updates_post.get_mut(uuid) {
                    for param in params {
                        match param {
                            BindValue::Author(v) => m.author = v.to_string(),
                            BindValue::Message(v) => m.message = v.to_string(),
                            BindValue::Likes(v) => m.likes = v,
                            BindValue::HasImage(v) => m.has_image = v,
                        }
                    }
                } else {
                    // merge with existing update if it exists
                    state
                        .updates_put
                        .entry(uuid.to_string())
                        .and_modify(|m| {
                            for param in params {
                                match param {
                                    BindValue::Author(v) => {
                                        m.fields.author = Maybe::Value(v.to_string())
                                    }
                                    BindValue::Message(v) => {
                                        m.fields.message = Maybe::Value(v.to_string())
                                    }
                                    BindValue::Likes(v) => m.fields.likes = Maybe::Value(v),
                                    BindValue::HasImage(v) => {
                                        m.fields.imageUpdate = Maybe::Value(v)
                                    }
                                }
                            }
                        })
                        .or_insert_with(|| PutUpdate {
                            fields: payload,
                            uuid: uuid.to_string(),
                        });
                }
                response.set_status_line("HTTP/1.1 204 No Content");
            }
        }
        Err(_) => {
            response.set_status_line("HTTP/1.1 500 Internal Server Error");
        }
    }

    response.to_string()
}
