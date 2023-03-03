use serde::Serialize;
use serde_json::json;

use crate::{
    app_state::{AppState, PutUpdate},
    maybe::Maybe,
    models::Message,
    response::Response,
};

#[derive(Serialize)]
struct CompleteMessage {
    uuid: String,
    author: String,
    message: String,
    likes: i32,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    base64Image: Maybe<String>,
}

impl CompleteMessage {
    pub fn new(message: Message, image: Option<String>) -> Self {
        CompleteMessage {
            uuid: message.uuid,
            author: message.author,
            base64Image: match image {
                Some(image) => Maybe::Value(image),
                None => Maybe::Absent,
            },
            likes: message.likes,
            message: message.message,
        }
    }
}

#[derive(Serialize)]
struct CompletePutUpdate {
    uuid: String,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    author: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    message: Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    likes: Maybe<i32>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    base64Image: Maybe<String>,
}

impl CompletePutUpdate {
    fn new(update: PutUpdate, image: Option<String>) -> Self {
        CompletePutUpdate {
            uuid: update.uuid,
            author: update.fields.author,
            message: update.fields.message,
            likes: update.fields.likes,
            base64Image: match image {
                Some(image) => Maybe::Value(image),
                None => Maybe::Absent,
            },
        }
    }
}

pub(crate) async fn handle_get(state: &mut AppState) -> String {
    let response = Response::new().append_header("Content-Type: application/json");

    // if there are cached mutation updates, return them
    let AppState {
        updates_post,
        updates_put,
        updates_delete,
        ..
    } = state;
    if updates_post.len() > 0 || updates_put.len() > 0 || !updates_delete.is_empty() {
        // constructing posts, while draining the updates_post map
        let posts = updates_post
            .drain()
            .map(|(_, m)| {
                let image = {
                    match m.has_image {
                        true => state.image_store.get(&m.uuid),
                        false => None,
                    }
                };
                CompleteMessage::new(m, image)
            })
            .collect::<Vec<_>>();

        // constructing puts, while draining the updates_put map
        let puts = updates_put
            .drain()
            .map(|(_, update)| {
                let image = {
                    match update.fields.imageUpdate {
                        Maybe::Value(true) => state.image_store.get(&update.uuid),
                        _ => None,
                    }
                };
                CompletePutUpdate::new(update, image)
            })
            .collect::<Vec<_>>();

        let body = json!({
            "posts": posts,
            "puts": puts,
            "deletes": updates_delete,
        })
        .to_string();

        // clear the deletes
        updates_delete.clear();

        return response
            .append_header(&format!("Content-Length: {}", body.len()))
            .body(&body)
            .to_string();
    }

    let messages = match sqlx::query_as!(
        Message,
        "
        SELECT *
        FROM messages
        "
    )
    .fetch_all(state.pool.as_ref())
    .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error while fetching messages: {}", e);
            return response
                .status_line("HTTP/1.1 500 Internal Server Error")
                .body("Internal Server Error")
                .to_string();
        }
    };

    let messages: Vec<CompleteMessage> = messages
        .into_iter()
        .map(|m| {
            let image = {
                match m.has_image {
                    true => state.image_store.get(&m.uuid),
                    false => None,
                }
            };
            CompleteMessage::new(m, image)
        })
        .collect();

    let res_body = json!(messages).to_string();

    response
        .append_header(&format!("Content-Length: {}", res_body.len()))
        .body(&res_body)
        .to_string()
}
