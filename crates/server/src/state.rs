use std::sync::Arc;

use rib_db::Pool as DbPool;
use rib_worker::WorkerPool;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub workers: Arc<WorkerPool>,
    pub jwt_secret: Arc<String>,
}
