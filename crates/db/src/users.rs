use chrono::Utc;
use uuid::Uuid;

use crate::models::User;
use crate::pool::Pool;

pub async fn create_user(pool: &Pool, username: &str, email: &str, password_hash: &str) -> sqlx::Result<User> {
    sqlx::query_as::<_, User>(
        r#"INSERT INTO users (username, email, password_hash)
           VALUES ($1, $2, $3)
           RETURNING id, username, email, password_hash, created_at, last_login_at"#,
    )
    .bind(username)
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

pub async fn get_user_by_username(pool: &Pool, username: &str) -> sqlx::Result<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"SELECT id, username, email, password_hash, created_at, last_login_at
           FROM users WHERE username = $1"#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_email(pool: &Pool, email: &str) -> sqlx::Result<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"SELECT id, username, email, password_hash, created_at, last_login_at
           FROM users WHERE email = $1"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_id(pool: &Pool, id: Uuid) -> sqlx::Result<Option<User>> {
    sqlx::query_as::<_, User>(
        r#"SELECT id, username, email, password_hash, created_at, last_login_at
           FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn touch_last_login(pool: &Pool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET last_login_at = $1 WHERE id = $2")
        .bind(Utc::now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
