pub mod mutation_manager;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{handlers::PutMessage, image_store::ImageStore};

use self::mutation_manager::MutationManager;

#[derive(Serialize, Deserialize)]
pub struct PutUpdate {
    pub(crate) fields: PutMessage,
    pub(crate) uuid: String,
}

pub struct AppState {
    pub pool: Arc<PgPool>,
    pub image_store: ImageStore,
    pub mutations: Mutex<MutationManager>,
}
