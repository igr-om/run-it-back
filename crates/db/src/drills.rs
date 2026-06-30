use serde_json::Value;
use uuid::Uuid;

use crate::models::{DrillAttemptRecord, DrillRecord};
use crate::pool::Pool;

#[allow(clippy::too_many_arguments)]
pub async fn create_drill(
    pool: &Pool,
    user_id: Uuid,
    game_type: &str,
    category: &str,
    spot_key: Option<&str>,
    spot_snapshot: Value,
    dealt_hand: &[String],
    correct_strategy: Value,
    correct_ev_bb: f64,
) -> sqlx::Result<DrillRecord> {
    sqlx::query_as::<_, DrillRecord>(
        r#"INSERT INTO drills
            (user_id, game_type, category, spot_key, spot_snapshot, dealt_hand, correct_strategy, correct_ev_bb)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
           RETURNING id, user_id, game_type, category, spot_key, spot_snapshot, dealt_hand,
                     correct_strategy, correct_ev_bb, created_at"#,
    )
    .bind(user_id)
    .bind(game_type)
    .bind(category)
    .bind(spot_key)
    .bind(spot_snapshot)
    .bind(dealt_hand)
    .bind(correct_strategy)
    .bind(correct_ev_bb)
    .fetch_one(pool)
    .await
}

pub async fn get_drill(pool: &Pool, id: Uuid) -> sqlx::Result<Option<DrillRecord>> {
    sqlx::query_as::<_, DrillRecord>(
        r#"SELECT id, user_id, game_type, category, spot_key, spot_snapshot, dealt_hand,
                  correct_strategy, correct_ev_bb, created_at
           FROM drills WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn record_attempt(
    pool: &Pool,
    drill_id: Uuid,
    user_id: Uuid,
    chosen_action: &str,
    ev_loss_bb: f64,
    is_correct: bool,
    explanation: &str,
) -> sqlx::Result<DrillAttemptRecord> {
    sqlx::query_as::<_, DrillAttemptRecord>(
        r#"INSERT INTO drill_attempts (drill_id, user_id, chosen_action, ev_loss_bb, is_correct, explanation)
           VALUES ($1,$2,$3,$4,$5,$6)
           RETURNING id, drill_id, user_id, chosen_action, ev_loss_bb, is_correct, explanation, answered_at"#,
    )
    .bind(drill_id)
    .bind(user_id)
    .bind(chosen_action)
    .bind(ev_loss_bb)
    .bind(is_correct)
    .bind(explanation)
    .fetch_one(pool)
    .await
}

pub async fn list_recent_attempts(pool: &Pool, user_id: Uuid, limit: i64) -> sqlx::Result<Vec<DrillAttemptRecord>> {
    sqlx::query_as::<_, DrillAttemptRecord>(
        r#"SELECT id, drill_id, user_id, chosen_action, ev_loss_bb, is_correct, explanation, answered_at
           FROM drill_attempts WHERE user_id = $1 ORDER BY answered_at DESC LIMIT $2"#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub struct AccuracyRow {
    pub total: i64,
    pub correct: i64,
}

pub async fn overall_accuracy(pool: &Pool, user_id: Uuid) -> sqlx::Result<AccuracyRow> {
    let row: (i64, i64) = sqlx::query_as(
        r#"SELECT COUNT(*), COALESCE(SUM(CASE WHEN is_correct THEN 1 ELSE 0 END), 0)
           FROM drill_attempts WHERE user_id = $1"#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    Ok(AccuracyRow { total: row.0, correct: row.1 })
}
