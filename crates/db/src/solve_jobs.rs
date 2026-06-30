use serde_json::Value;
use uuid::Uuid;

use crate::models::SolveJobRecord;
use crate::pool::Pool;

pub async fn enqueue_solve_job(pool: &Pool, user_id: Option<Uuid>, request: Value) -> sqlx::Result<SolveJobRecord> {
    sqlx::query_as::<_, SolveJobRecord>(
        r#"INSERT INTO solve_jobs (user_id, request, status)
           VALUES ($1, $2, 'queued')
           RETURNING id, user_id, request, status, progress, result, error, created_at, completed_at"#,
    )
    .bind(user_id)
    .bind(request)
    .fetch_one(pool)
    .await
}

pub async fn mark_running(pool: &Pool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE solve_jobs SET status = 'running' WHERE id = $1").bind(id).execute(pool).await?;
    Ok(())
}

pub async fn update_progress(pool: &Pool, id: Uuid, progress: f32) -> sqlx::Result<()> {
    sqlx::query("UPDATE solve_jobs SET progress = $2 WHERE id = $1")
        .bind(id)
        .bind(progress)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn complete_solve_job(pool: &Pool, id: Uuid, result: Value) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE solve_jobs SET status = 'done', progress = 1.0, result = $2, completed_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(result)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_solve_job(pool: &Pool, id: Uuid, error: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE solve_jobs SET status = 'failed', error = $2, completed_at = now() WHERE id = $1")
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_solve_job(pool: &Pool, id: Uuid) -> sqlx::Result<Option<SolveJobRecord>> {
    sqlx::query_as::<_, SolveJobRecord>(
        r#"SELECT id, user_id, request, status, progress, result, error, created_at, completed_at
           FROM solve_jobs WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}
