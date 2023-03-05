use std::sync::Arc;

use serde::Serialize;
use serde_json::json;

use crate::{
    app_state::{AppState, PutUpdate},
    maybe::Maybe,
    models::Message,
    response::Response,
};

#[derive(Serialize, Debug)]
pub struct CompleteMessage {
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

#[derive(Serialize, Debug)]
pub struct CompletePutUpdate {
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
    pub fn new(update: PutUpdate, image: Option<String>) -> Self {
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

#[derive(Serialize)]
pub struct PaginationMetadata {
    total_pages: usize,
}

impl PaginationMetadata {
    pub fn new(count_all: usize, page_size: usize) -> Self {
        PaginationMetadata {
            total_pages: (count_all / page_size) + 1,
        }
    }
}

pub(crate) async fn handle_get(state: Arc<AppState>) -> String {
    let mut triggered_pagination = state.triggered_pagination.lock().await;
    if !*triggered_pagination {
        return Response::new()
            .status_line("HTTP/1.1 403 Forbidden")
            .body("Pagination not triggered yet. Request GET /trigger-pagination first and then, concurrently, GET / for a number of times equal to the total_pages value in the response of the previous request.")
            .to_string();
    }

    let response = Response::new().append_header("Content-Type: application/json");

    {
        let mut mutations = state.mutations.lock().await;
        if !mutations.is_pagination_empty() {
            let result = mutations.get(triggered_pagination);
            drop(mutations);
            let body = serde_json::to_string(&result).unwrap();
            return response
                .append_header(&format!("Content-Length: {}", body.len()))
                .body(&body)
                .to_string();
        }
    } // end of mutations lock

    // pagination in postgres
    let mut offset = state.db_pagination_offset.lock().await;
    // get a page of messages
    let messages = match sqlx::query_as!(
        Message,
        "
        SELECT *
        FROM messages
        ORDER BY uuid
        LIMIT $1
        OFFSET $2
        ",
        state.pagination_page_size as i64,
        *offset as i64
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

    // increase or reset the offset
    if messages.len() == state.pagination_page_size {
        *offset += state.pagination_page_size;
    } else {
        // pagination is done, reset the offset and the flag
        *offset = 0;
        *triggered_pagination = false;
    }

    // drop the lock so that other threads can access the offset
    drop(offset);

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

pub(crate) async fn get_pagination_meta(state: Arc<AppState>) -> String {
    // trigger pagination
    *state.triggered_pagination.lock().await = true;

    let response = Response::new().append_header("Content-Type: application/json");

    // if there are cached mutation updates, return them
    {
        let mut mutations = state.mutations.lock().await;
        if !mutations.is_empty_for_pagination() {
            let meta = mutations.get_pagination_meta();
            drop(mutations);
            let body = serde_json::to_string(&meta).unwrap();
            return response
                .status_line("HTTP/1.1 200 OK")
                .append_header(&format!("Content-Length: {}", body.len()))
                .body(body.as_str())
                .to_string();
        }
    } // end of mutations lock

    // count all rows in the database
    let count = match sqlx::query!(
        "
        SELECT COUNT(*) as count
        FROM messages
        "
    )
    .fetch_one(state.pool.as_ref())
    .await
    {
        Ok(v) => v.count.unwrap() as usize,
        Err(e) => {
            eprintln!("Error while counting messages: {}", e);
            return response
                .status_line("HTTP/1.1 500 Internal Server Error")
                .body("Internal Server Error")
                .to_string();
        }
    };

    let meta = PaginationMetadata::new(count, state.pagination_page_size);
    let body = serde_json::to_string(&meta).unwrap();
    response
        .status_line("HTTP/1.1 200 OK")
        .append_header(&format!("Content-Length: {}", body.len()))
        .body(body.as_str())
        .to_string()
}
