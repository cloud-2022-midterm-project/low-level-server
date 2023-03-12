use crate::mutation_manager::MutationManager;
use ahash::AHashSet;
use sqlx::PgPool;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

pub struct AppState {
    pub pool: Arc<PgPool>,
    pub mutations: Mutex<MutationManager>,
    pub pagination_page_size: usize,
    pub db_pagination_offset: Mutex<usize>,
    pub triggered_pagination: Mutex<bool>,
    pub image_base_path: PathBuf,
    pub all_uuids: Mutex<AHashSet<String>>,
}
