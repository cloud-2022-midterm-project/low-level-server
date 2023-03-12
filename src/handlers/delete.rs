use std::sync::Arc;

use crate::{app_state::AppState, image, response::Response};

pub(crate) async fn handle_delete(uuid: &str, state: Arc<AppState>) -> String {
    let mut response = Response::new();

    // check for conflicting uuid
    if !state.all_uuids.lock().await.remove(uuid) {
        return response.status_line("HTTP/1.1 404 NOT FOUND").to_string();
    }

    let result = sqlx::query!("DELETE FROM messages WHERE uuid = $1", uuid)
        .execute(state.pool.as_ref())
        .await;

    match result {
        Ok(result) => {
            if result.rows_affected() == 0 {
                response.set_status_line("HTTP/1.1 404 NOT FOUND");
            } else {
                // remove from image store if it exists
                image::remove(&state.image_base_path, uuid).ok();
                state.mutations.lock().await.add_delete(uuid);
                response.set_status_line("HTTP/1.1 204 NO CONTENT");
            }
        }
        Err(_) => response.set_status_line("HTTP/1.1 500 INTERNAL SERVER ERROR"),
    }

    response.to_string()
}
