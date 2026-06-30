use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use rib_db::models::RangeRecord;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateRangeReq {
    pub name: String,
    pub game_type: String,
    pub range_string: String,
}

pub async fn list(AuthUser(user_id): AuthUser, State(state): State<AppState>) -> Result<Json<Vec<RangeRecord>>, ApiError> {
    let ranges = rib_db::ranges::list_ranges_for_user(&state.db, user_id).await?;
    Ok(Json(ranges))
}

pub async fn create(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CreateRangeReq>,
) -> Result<Json<RangeRecord>, ApiError> {
    // Validate it parses before saving, so a typo doesn't silently save a
    // broken range string.
    rib_core::parse_range_string(&req.range_string)?;
    let range =
        rib_db::ranges::create_range(&state.db, Some(user_id), req.name.trim(), &req.game_type, &req.range_string).await?;
    Ok(Json(range))
}

pub async fn delete(
    AuthUser(user_id): AuthUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let affected = rib_db::ranges::delete_range(&state.db, id, user_id).await?;
    if affected == 0 {
        return Err(ApiError::not_found("range not found (or it isn't yours to delete)"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}
