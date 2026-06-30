use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use rib_db::models::{HandHistoryRecord, PlayerStatsRecord};
use rib_worker::ParseJob;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UploadReq {
    /// Raw hand history text, pasted or read client-side from the uploaded
    /// file. Site is auto-detected server-side; `site_hint` is accepted but
    /// currently only used for the stored filename/label, since detection
    /// from content is reliable enough not to need it.
    pub raw_text: String,
    pub filename: Option<String>,
    pub site_hint: Option<String>,
}

pub async fn upload(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
    Json(req): Json<UploadReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.raw_text.trim().is_empty() {
        return Err(ApiError::bad_request("uploaded file was empty"));
    }
    let site = req.site_hint.unwrap_or_else(|| "unknown".to_string());
    let record = rib_db::hand_history::insert_hand_history(&state.db, user_id, &site, req.filename.as_deref(), &req.raw_text)
        .await?;
    state.workers.submit_parse(ParseJob { hand_history_id: record.id, user_id });
    Ok(Json(serde_json::json!({ "hand_history_id": record.id, "status": "queued_for_parsing" })))
}

pub async fn list(AuthUser(user_id): AuthUser, State(state): State<AppState>) -> Result<Json<Vec<HandHistoryRecord>>, ApiError> {
    let list = rib_db::hand_history::list_hand_histories_for_user(&state.db, user_id).await?;
    Ok(Json(list))
}

pub async fn parsed_hands(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<rib_db::models::ParsedHandRecord>>, ApiError> {
    let hands = rib_db::hand_history::list_parsed_hands_for_user(&state.db, user_id, 500).await?;
    Ok(Json(hands))
}

pub async fn stats_overview(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Option<PlayerStatsRecord>>, ApiError> {
    let stats = rib_db::stats::get_player_stats(&state.db, user_id, "nlhe").await?;
    Ok(Json(stats))
}
