use uuid::Uuid;

use crate::models::RangeRecord;
use crate::pool::Pool;

pub async fn create_range(
    pool: &Pool,
    user_id: Option<Uuid>,
    name: &str,
    game_type: &str,
    range_string: &str,
) -> sqlx::Result<RangeRecord> {
    sqlx::query_as::<_, RangeRecord>(
        r#"INSERT INTO ranges (user_id, name, game_type, range_string)
           VALUES ($1, $2, $3, $4)
           RETURNING id, user_id, name, game_type, range_string, created_at"#,
    )
    .bind(user_id)
    .bind(name)
    .bind(game_type)
    .bind(range_string)
    .fetch_one(pool)
    .await
}

/// Ranges visible to a user: their own saved ranges plus every system
/// preset (`user_id IS NULL`).
pub async fn list_ranges_for_user(pool: &Pool, user_id: Uuid) -> sqlx::Result<Vec<RangeRecord>> {
    sqlx::query_as::<_, RangeRecord>(
        r#"SELECT id, user_id, name, game_type, range_string, created_at
           FROM ranges WHERE user_id = $1 OR user_id IS NULL
           ORDER BY user_id IS NULL DESC, created_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_range(pool: &Pool, id: Uuid, user_id: Uuid) -> sqlx::Result<u64> {
    let res = sqlx::query("DELETE FROM ranges WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}
