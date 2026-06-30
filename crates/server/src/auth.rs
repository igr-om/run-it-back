use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ApiError::internal(e))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else { return false };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // user id
    exp: i64,
}

pub fn issue_jwt(user_id: Uuid, secret: &str) -> Result<String, ApiError> {
    let claims = Claims { sub: user_id.to_string(), exp: (Utc::now() + Duration::days(30)).timestamp() };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(|e| ApiError::internal(e))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Uuid, ApiError> {
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &Validation::default())
        .map_err(|_| ApiError::unauthorized("invalid or expired session"))?;
    Uuid::parse_str(&data.claims.sub).map_err(|_| ApiError::unauthorized("invalid session subject"))
}

/// An Axum extractor: add `current_user: AuthUser` to any handler's
/// arguments and Axum will reject the request with 401 before the handler
/// body even runs if the `Authorization: Bearer <token>` header is
/// missing/invalid -- every protected route uses this instead of checking
/// auth manually.
pub struct AuthUser(pub Uuid);

#[axum::async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ApiError::unauthorized("missing Authorization header"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::unauthorized("Authorization header must be 'Bearer <token>'"))?;
        let user_id = verify_jwt(token, &state.jwt_secret)?;
        Ok(AuthUser(user_id))
    }
}
