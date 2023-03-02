use ahash::AHashMap;
use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;

use crate::{handlers::PutMessage, image_store::ImageStore, models::Message};

#[derive(Serialize)]
pub struct PutUpdate {
    pub(crate) fields: PutMessage,
    pub(crate) uuid: String,
}

pub struct AppState {
    pub pool: Arc<PgPool>,
    pub image_store: ImageStore,
    pub updates_post: AHashMap<String, Message>,
    pub updates_put: AHashMap<String, PutUpdate>,
    pub updates_delete: Vec<String>,
}
