use std::sync::Arc;

use crate::{app_state::AppState, response::Response};

pub(crate) async fn handle_delete(uuid: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    let result = sqlx::query!(
        "
        DELETE FROM messages
        WHERE uuid = $1
        ",
        uuid
    )
    .execute(state.pool.as_ref())
    .await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 NOT FOUND");
            } else {
                // remove from image store if it exists
                state.image_store.remove(uuid).ok();
                state.mutations.lock().await.add_delete(uuid);
                response.set_status_line("HTTP/1.1 204 NO CONTENT");
            }
        }
        Err(_) => response.set_status_line("HTTP/1.1 500 INTERNAL SERVER ERROR"),
    }

    response.to_string()
}
