use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use rib_db::models::User;

use crate::auth::{hash_password, issue_jwt, verify_password, AuthUser};
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterReq {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResp {
    pub token: String,
    pub user: User,
}

pub async fn register(State(state): State<AppState>, Json(req): Json<RegisterReq>) -> Result<Json<AuthResp>, ApiError> {
    let username = req.username.trim();
    let email = req.email.trim();
    if username.len() < 3 {
        return Err(ApiError::bad_request("username must be at least 3 characters"));
    }
    if req.password.len() < 8 {
        return Err(ApiError::bad_request("password must be at least 8 characters"));
    }
    if !email.contains('@') {
        return Err(ApiError::bad_request("that doesn't look like a valid email address"));
    }

    let hash = hash_password(&req.password)?;
    let user = rib_db::users::create_user(&state.db, username, email, &hash).await.map_err(|e| {
        if e.to_string().to_lowercase().contains("unique") {
            ApiError::conflict("that username or email is already taken")
        } else {
            ApiError::from(e)
        }
    })?;
    let token = issue_jwt(user.id, &state.jwt_secret)?;
    Ok(Json(AuthResp { token, user }))
}

pub async fn login(State(state): State<AppState>, Json(req): Json<LoginReq>) -> Result<Json<AuthResp>, ApiError> {
    let user = rib_db::users::get_user_by_username(&state.db, req.username.trim())
        .await?
        .ok_or_else(|| ApiError::unauthorized("invalid username or password"))?;
    if !verify_password(&req.password, &user.password_hash) {
        return Err(ApiError::unauthorized("invalid username or password"));
    }
    let _ = rib_db::users::touch_last_login(&state.db, user.id).await;
    let token = issue_jwt(user.id, &state.jwt_secret)?;
    Ok(Json(AuthResp { token, user }))
}

pub async fn me(AuthUser(user_id): AuthUser, State(state): State<AppState>) -> Result<Json<User>, ApiError> {
    let user = rib_db::users::get_user_by_id(&state.db, user_id).await?.ok_or_else(|| ApiError::not_found("user not found"))?;
    Ok(Json(user))
}
