use serde::Serialize;
use serde_json::json;

use crate::{
    app_state::{AppState, PutUpdate},
    maybe::Maybe,
    models::Message,
    response::Response,
};

#[derive(Serialize)]
struct CompleteMessage<'a> {
    uuid: &'a str,
    author: &'a str,
    message: &'a str,
    likes: i32,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    base64Image: Maybe<&'a str>,
}

impl<'a> CompleteMessage<'a> {
    pub fn new(message: &'a Message, image: Option<&'a str>) -> Self {
        CompleteMessage {
            uuid: &message.uuid,
            author: &message.author,
            base64Image: match image {
                Some(image) => Maybe::Value(image),
                None => Maybe::Absent,
            },
            likes: message.likes,
            message: &message.message,
        }
    }
}

#[derive(Serialize)]
struct CompletePutUpdate<'a> {
    uuid: &'a str,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    author: &'a Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    message: &'a Maybe<String>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    likes: &'a Maybe<i32>,
    #[serde(default, skip_serializing_if = "Maybe::is_absent")]
    base64Image: Maybe<&'a str>,
}

impl<'a> CompletePutUpdate<'a> {
    fn new(update: &'a PutUpdate, image: Option<&'a str>) -> Self {
        CompletePutUpdate {
            uuid: &update.uuid,
            author: &update.fields.author,
            message: &update.fields.message,
            likes: &update.fields.likes,
            base64Image: match image {
                Some(image) => Maybe::Value(image),
                None => Maybe::Absent,
            },
        }
    }
}

/// ## TODO
/// Should we improve the cached response construction logic?
pub(crate) async fn handle_get(state: &mut AppState) -> String {
    let response = Response::new().append_header("Content-Type: application/json");

    // If there are mutation updates, return them
    let AppState {
        updates_post,
        updates_put,
        updates_delete,
        ..
    } = state;
    if updates_post.len() > 0 || updates_put.len() > 0 || !updates_delete.is_empty() {
        // constructing posts for response
        let updates_post_vec = updates_post.values().collect::<Vec<_>>();
        let images_post = state.image_store.get_many(
            &updates_post_vec
                .iter()
                .map(|m| match m.has_image {
                    true => Some(m.uuid.as_ref()),
                    false => None,
                })
                .collect::<Vec<_>>(),
        );
        let posts = updates_post_vec
            .iter()
            .zip(images_post.iter())
            .map(|(m, image)| CompleteMessage::new(m, image.as_deref()))
            .collect::<Vec<_>>();

        // constructing puts for response
        let updates_put_vec = updates_put.values().collect::<Vec<_>>();
        let images_put = state.image_store.get_many(
            &updates_put_vec
                .iter()
                .map(|update| match update.fields.imageUpdate {
                    Maybe::Value(true) => Some(update.uuid.as_ref()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
        let puts = updates_put_vec
            .iter()
            .zip(images_put.iter())
            .map(|(update, image)| CompletePutUpdate::new(update, image.as_deref()))
            .collect::<Vec<_>>();

        let body = json!({
            "posts": posts,
            "puts": puts,
            "deletes": *updates_delete,
        })
        .to_string();

        // clear updates
        updates_post.clear();
        updates_put.clear();
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

    let images = state.image_store.get_many(
        &messages
            .iter()
            .map(|m| match m.has_image {
                true => Some(m.uuid.as_ref()),
                false => None,
            })
            .collect::<Vec<_>>(),
    );
    let complete_messages: Vec<CompleteMessage> = messages
        .iter()
        .zip(images.iter())
        .map(|(m, image)| CompleteMessage {
            base64Image: match image {
                Some(image) => Maybe::Value(image),
                None => Maybe::Absent,
            },
            author: &m.author,
            likes: m.likes,
            message: &m.message,
            uuid: &m.uuid,
        })
        .collect();

    let res_body = json!(complete_messages).to_string();

    response
        .append_header(&format!("Content-Length: {}", res_body.len()))
        .body(&res_body)
        .to_string()
}
