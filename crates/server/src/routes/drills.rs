use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use rib_core::Action;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
pub struct DrillPublicView {
    pub id: Uuid,
    pub category: String,
    pub spot_snapshot: Value,
    pub dealt_hand: Vec<String>,
    pub available_actions: Vec<Action>,
}

pub async fn generate(AuthUser(user_id): AuthUser, State(state): State<AppState>) -> Result<Json<DrillPublicView>, ApiError> {
    let generated = rib_drills::generate_and_persist(&state.db, user_id, "nlhe").await?;
    Ok(Json(DrillPublicView {
        id: generated.record.id,
        category: generated.record.category,
        spot_snapshot: generated.record.spot_snapshot,
        dealt_hand: generated.record.dealt_hand,
        available_actions: generated.response.hero_strategy.actions,
    }))
}

#[derive(Deserialize)]
pub struct AnswerReq {
    pub chosen_action_index: usize,
}

pub async fn answer(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AnswerReq>,
) -> Result<Json<rib_drills::GradeResult>, ApiError> {
    let drill = rib_db::drills::get_drill(&state.db, id).await?.ok_or_else(|| ApiError::not_found("drill not found"))?;
    if drill.user_id != user_id {
        return Err(ApiError::not_found("drill not found"));
    }

    let actions: Vec<Action> = serde_json::from_value(
        drill.correct_strategy.get("actions").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| ApiError::internal(e))?;
    let action_ev_bb: Vec<f32> = serde_json::from_value(
        drill.correct_strategy.get("action_ev_bb").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| ApiError::internal(e))?;
    let frequencies: Vec<f32> = serde_json::from_value(
        drill.correct_strategy.get("frequencies").cloned().unwrap_or(Value::Array(vec![])),
    )
    .map_err(|e| ApiError::internal(e))?;

    let hero_cards: Vec<rib_core::Card> = drill
        .dealt_hand
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();
    if hero_cards.len() != 2 {
        return Err(ApiError::internal("drill's dealt hand was malformed"));
    }
    let board: Vec<rib_core::Card> = drill
        .spot_snapshot
        .get("board")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|c| c.as_str().and_then(|s| s.parse().ok())).collect())
        .unwrap_or_default();

    let result = rib_drills::grade(
        &actions,
        &action_ev_bb,
        &frequencies,
        req.chosen_action_index,
        &drill.category,
        (hero_cards[0], hero_cards[1]),
        &board,
        &drill.spot_snapshot,
    )?;

    rib_db::drills::record_attempt(
        &state.db,
        drill.id,
        user_id,
        &result.chosen_action,
        result.ev_loss_bb as f64,
        result.is_correct,
        &result.explanation,
    )
    .await?;
    rib_db::weakness::record_result(&state.db, user_id, "nlhe", &drill.category, result.is_correct, result.ev_loss_bb as f64)
        .await?;

    Ok(Json(result))
}

pub async fn recent_attempts(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<rib_db::models::DrillAttemptRecord>>, ApiError> {
    let attempts = rib_db::drills::list_recent_attempts(&state.db, user_id, 50).await?;
    Ok(Json(attempts))
}

pub async fn weakness_profile(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<rib_db::models::WeaknessProfileRecord>>, ApiError> {
    let profile = rib_db::weakness::list_for_user(&state.db, user_id, "nlhe").await?;
    Ok(Json(profile))
}
