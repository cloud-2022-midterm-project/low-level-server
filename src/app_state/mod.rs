use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

use crate::{handlers::PutMessage, mutation_manager::MutationManager};

#[derive(Serialize, Deserialize)]
pub struct PutUpdate {
    pub(crate) fields: PutMessage,
    pub(crate) uuid: String,
}

pub struct AppState {
    pub pool: Arc<PgPool>,
    pub mutations: Mutex<MutationManager>,
    pub pagination_page_size: usize,
    pub db_pagination_offset: Mutex<usize>,
    pub triggered_pagination: Mutex<bool>,
    pub image_base_path: PathBuf,
}
