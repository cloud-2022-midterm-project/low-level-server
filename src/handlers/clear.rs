use crate::app_state::AppState;
use std::sync::Arc;

pub(crate) async fn clear(state: Arc<AppState>) -> String {
    let mut response = crate::response::Response::new();

    let result = sqlx::query!("DELETE FROM messages")
        .execute(state.pool.as_ref())
        .await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 NOT FOUND");
            } else {
                // remove from image store if it exists
                state.image_store.clear();
                state.mutations.lock().await.clear();
                response.set_status_line("HTTP/1.1 204 NO CONTENT");
            }
        }
        Err(_) => response.set_status_line("HTTP/1.1 500 INTERNAL SERVER ERROR"),
    }

    response.to_string()
}
