pub mod mutation_manager;

use serde::Serialize;
use sqlx::PgPool;
use std::sync::Arc;

use crate::{handlers::PutMessage, image_store::ImageStore};

use self::mutation_manager::MutationManager;

#[derive(Serialize)]
pub struct PutUpdate {
    pub(crate) fields: PutMessage,
    pub(crate) uuid: String,
}

pub struct AppState {
    pub pool: Arc<PgPool>,
    pub image_store: ImageStore,
    pub mutations: MutationManager,
}
