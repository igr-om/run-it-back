use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use rib_core::{GameType, Position, PotType};
use rib_solver::{SolveRequest, SpotKey};
use rib_worker::SolveJob;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn enqueue(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
    Json(req): Json<SolveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let request_json = serde_json::to_value(&req).map_err(|e| ApiError::internal(e))?;
    let job = rib_db::solve_jobs::enqueue_solve_job(&state.db, Some(user_id), request_json).await?;
    state.workers.submit_solve(SolveJob { job_id: job.id, request: req });
    Ok(Json(serde_json::json!({ "job_id": job.id, "status": "queued" })))
}

pub async fn job_status(
    AuthUser(_user_id): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<rib_db::models::SolveJobRecord>, ApiError> {
    let job = rib_db::solve_jobs::get_solve_job(&state.db, id).await?.ok_or_else(|| ApiError::not_found("solve job not found"))?;
    Ok(Json(job))
}

#[derive(Deserialize)]
pub struct PreflopQuery {
    pub hero: String,
    pub villain: String,
    pub stack_bb: u32,
    pub pot_type: String, // "srp" | "three_bet" | "four_bet"
}

fn parse_position(s: &str) -> Result<Position, ApiError> {
    for p in Position::for_table_size(9) {
        if p.label().eq_ignore_ascii_case(s) {
            return Ok(p);
        }
    }
    Err(ApiError::bad_request(format!("unrecognized position '{s}'")))
}

fn parse_pot_type(s: &str) -> Result<PotType, ApiError> {
    match s.to_ascii_lowercase().as_str() {
        "srp" | "single_raised" => Ok(PotType::Srp),
        "three_bet" | "3bet" => Ok(PotType::ThreeBet),
        "four_bet" | "4bet" => Ok(PotType::FourBet),
        "five_bet_plus" | "5bet" => Ok(PotType::FiveBetPlus),
        "limped" => Ok(PotType::LimpedPot),
        other => Err(ApiError::bad_request(format!("unrecognized pot type '{other}'"))),
    }
}

/// Instant lookup against the precomputed preflop library -- no queueing,
/// no waiting, since these were already warmed at startup by
/// `rib-worker::seed`. Returns 404 if this exact combination wasn't in the
/// curated seed list (uncommon stack depth, 9-max-only position, etc); the
/// frontend's fallback is to call `POST /api/solve` for a live solve of the
/// same spot instead.
pub async fn preflop_library(
    AuthUser(_user_id): AuthUser,
    State(state): State<AppState>,
    Query(q): Query<PreflopQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = SpotKey {
        game: GameType::Nlhe,
        pot_type: parse_pot_type(&q.pot_type)?,
        stack_bb: q.stack_bb,
        hero_position: parse_position(&q.hero)?,
        villain_position: parse_position(&q.villain)?,
        board: vec![],
    };
    let row = rib_db::solved_spots::get_solved_spot(&state.db, &key.cache_key())
        .await?
        .ok_or_else(|| ApiError::not_found("not in the precomputed library yet -- try a live solve for this exact spot"))?;
    Ok(Json(row.response))
}
