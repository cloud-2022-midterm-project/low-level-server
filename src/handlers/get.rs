use crate::{app_state::AppState, image, models::Message, response::Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use ts_rs::TS;

#[derive(Serialize, Debug, Deserialize, TS)]
#[ts(export)]
pub struct CompleteMessage {
    pub uuid: String,
    pub author: String,
    pub message: String,
    pub likes: i32,
    pub image: String,
}

impl CompleteMessage {
    pub fn new(message: Message, image: String) -> Self {
        CompleteMessage {
            uuid: message.uuid,
            author: message.author,
            image,
            likes: message.likes,
            message: message.message,
        }
    }
}

#[derive(Serialize, Debug)]
pub enum PaginationType {
    Cache,
    Fresh,
}

#[derive(Serialize)]
pub struct PaginationMetadata {
    total_pages: usize,
    kind: PaginationType,
}

impl PaginationMetadata {
    pub fn new(count_all: usize, page_size: usize, kind: PaginationType) -> Self {
        PaginationMetadata {
            total_pages: (count_all as f64 / page_size as f64).ceil() as usize,
            kind,
        }
    }
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DbResults {
    pub page_number: usize,
    pub messages: Vec<CompleteMessage>,
}

pub(crate) async fn handle_get(state: Arc<AppState>) -> Vec<u8> {
    {
        let triggered_pagination = state.triggered_pagination.lock().await;
        if !*triggered_pagination {
            return Response::new()
                .status_line("HTTP/1.1 403 Forbidden")
                .body("Pagination not triggered yet.")
                .to_string()
                .into_bytes();
        }
    }

    let response = Response::new().append_header("Content-Type: application/json");

    {
        let mut mutations = state.mutations.lock().await;
        if !mutations.is_pagination_empty() {
            let mut page_number = state.pagination_page_number.lock().await;

            let result = mutations.get(*page_number, &state.image_base_path);
            drop(mutations);

            *page_number += 1;

            // if the pagination is done, reset the flag, and page_number
            let mut triggered_pagination = state.triggered_pagination.lock().await;
            if result.done {
                *triggered_pagination = false;
                *page_number = 0;
            }

            drop(page_number);
            drop(triggered_pagination);

            let body = bincode::serialize(&result).unwrap();
            let mut res = response
                .append_header(&format!("Content-Length: {}", body.len()))
                .to_string()
                .into_bytes();
            res.extend(body);
            return res;
        }
    }

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
    .map(|m| {
        let image = {
            match m.has_image {
                true => image::get(&state.image_base_path, &m.uuid).unwrap_or("".to_string()),
                false => "".to_string(),
            }
        };
        CompleteMessage::new(m, image)
    })
    .fetch_all(state.pool.as_ref())
    .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error while fetching messages: {}", e);
            return response
                .status_line("HTTP/1.1 500 Internal Server Error")
                .body("Internal Server Error")
                .to_string()
                .into_bytes();
        }
    };

    let mut page_number = state.pagination_page_number.lock().await;
    *page_number += 1;
    let mut triggered_pagination = state.triggered_pagination.lock().await;

    let result = DbResults {
        page_number: *page_number,
        messages,
    };

    if *page_number == *state.pages_count.lock().await {
        // pagination is done, reset the offset and the flag
        *offset = 0;
        *triggered_pagination = false;
        *page_number = 0;
    } else {
        *offset += state.pagination_page_size;
    }

    // drop the locks so that other threads can access the flag and offset immediately
    drop(triggered_pagination);
    drop(offset);

    let body = bincode::serialize(&result).unwrap();
    let mut res = response
        .append_header(&format!("Content-Length: {}", body.len()))
        .to_string()
        .into_bytes();
    res.extend(body);
    res
}

pub(crate) async fn get_pagination_meta(state: Arc<AppState>) -> Vec<u8> {
    // trigger pagination
    *state.triggered_pagination.lock().await = true;

    let response = Response::new().append_header("Content-Type: application/octet-stream");

    // if there are cached mutation updates, return them
    {
        let mut mutations = state.mutations.lock().await;
        if !mutations.is_empty_for_pagination() {
            let meta = mutations.get_pagination_meta();
            *state.pages_count.lock().await = meta.total_pages;
            drop(mutations);
            let body = bincode::serialize(&meta).unwrap();
            let res_without_body = response
                .status_line("HTTP/1.1 200 OK")
                .append_header(&format!("Content-Length: {}", body.len()))
                .to_string();
            let mut res = res_without_body.into_bytes();
            res.extend(body);
            return res;
        }
    }

    let count = state.all_uuids.lock().await.len();
    let meta = PaginationMetadata::new(count, state.pagination_page_size, PaginationType::Fresh);
    *state.pages_count.lock().await = meta.total_pages;
    let body = bincode::serialize(&meta).unwrap();
    let mut res = response
        .status_line("HTTP/1.1 200 OK")
        .append_header(&format!("Content-Length: {}", body.len()))
        .to_string()
        .into_bytes();
    res.extend(body);
    res
}
