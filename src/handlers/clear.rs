use crate::{app_state::AppState, image};
use std::sync::Arc;

pub(crate) async fn clear(state: Arc<AppState>) -> String {
    let mut response = crate::response::Response::new();

    let result = sqlx::query!("DELETE FROM messages")
        .execute(state.pool.as_ref())
        .await;

    match result {
        Ok(_) => {
            image::clear(&state.image_base_path).ok();
            state.mutations.lock().await.clear();
            state.all_uuids.lock().await.clear();
            response.set_status_line("HTTP/1.1 204 NO CONTENT");
        }
        Err(_) => response.set_status_line("HTTP/1.1 500 INTERNAL SERVER ERROR"),
    }

    response.to_string()
}
