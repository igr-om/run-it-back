use uuid::Uuid;

use rib_solver::SolveRequest;

#[derive(Debug, Clone)]
pub struct ParseJob {
    pub hand_history_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct SolveJob {
    pub job_id: Uuid,
    pub request: SolveRequest,
}

#[derive(Debug, Clone)]
pub struct WarmJob {
    pub key: rib_solver::SpotKey,
    pub request: SolveRequest,
}
