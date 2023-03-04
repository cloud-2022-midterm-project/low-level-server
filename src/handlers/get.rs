use std::sync::Arc;

use serde::Serialize;
use serde_json::json;

use crate::{
    app_state::{AppState, PutUpdate},
    maybe::Maybe,
    models::Message,
    request::Request,
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

pub(crate) async fn handle_get(request: Request, state: Arc<AppState>) -> String {
    let request_id = match request.uri().trim_start_matches("/").parse() {
        Ok(id) => id,
        Err(e) => {
            let body = format!(
                "Failed to parse request id, use GET like this GET /<request_id>\n{}",
                e
            );
            return Response::new()
                .status_line("HTTP/1.1 400 BAD REQUEST")
                .body(body.as_str())
                .to_string();
        }
    };
    let response = Response::new().append_header("Content-Type: application/json");

    // if there are cached mutation updates, return them
    {
        let mut mutations = state.mutations.lock().await;
        if !mutations.is_empty().await {
            let paginated_mutations = mutations.get_paginated(request_id).await;
            let body = serde_json::to_string(&paginated_mutations).unwrap();
            return response
                .status_line("HTTP/1.1 200 OK")
                .append_header(&format!("Content-Length: {}", body.len()))
                .body(body.as_str())
                .to_string();
            // constructing posts, while draining the updates_post map
            // let posts = state
            //     .mutations
            //     .updates_post_mut()
            //     .drain()
            //     .map(|(_, m)| {
            //         let image = {
            //             match m.has_image {
            //                 true => state.image_store.get(&m.uuid),
            //                 false => None,
            //             }
            //         };
            //         CompleteMessage::new(m, image)
            //     })
            //     .collect::<Vec<_>>();

            // // constructing puts, while draining the updates_put map
            // let puts = state
            //     .mutations
            //     .updates_put_mut()
            //     .drain()
            //     .map(|(_, update)| {
            //         let image = {
            //             match update.fields.imageUpdate {
            //                 Maybe::Value(true) => state.image_store.get(&update.uuid),
            //                 _ => None,
            //             }
            //         };
            //         CompletePutUpdate::new(update, image)
            //     })
            //     .collect::<Vec<_>>();

            // let body = json!({
            //     "posts": posts,
            //     "puts": puts,
            //     "deletes": state.mutations.updates_delete(),
            // })
            // .to_string();

            // state.mutations.clear();

            // return response
            //     .append_header(&format!("Content-Length: {}", body.len()))
            //     .body(&body)
            //     .to_string();
        }
    } // end of mutations lock

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
