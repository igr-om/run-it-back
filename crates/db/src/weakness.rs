use uuid::Uuid;

use crate::models::WeaknessProfileRecord;
use crate::pool::Pool;

/// Incrementally folds one drill attempt's result into the user's running
/// per-category accuracy and average EV-loss. Using a running mean (rather
/// than recomputing from `drill_attempts` every time) keeps this O(1) no
/// matter how much drill history a user accumulates.
pub async fn record_result(
    pool: &Pool,
    user_id: Uuid,
    game_type: &str,
    category: &str,
    is_correct: bool,
    ev_loss_bb: f64,
) -> sqlx::Result<()> {
    sqlx::query(
        r#"INSERT INTO weakness_profiles (user_id, game_type, category, attempts, correct, avg_ev_loss_bb, last_seen_at)
           VALUES ($1, $2, $3, 1, $4, $5, now())
           ON CONFLICT (user_id, game_type, category) DO UPDATE SET
             attempts = weakness_profiles.attempts + 1,
             correct = weakness_profiles.correct + $4,
             avg_ev_loss_bb = (weakness_profiles.avg_ev_loss_bb * weakness_profiles.attempts + $5)
                              / (weakness_profiles.attempts + 1),
             last_seen_at = now()"#,
    )
    .bind(user_id)
    .bind(game_type)
    .bind(category)
    .bind(if is_correct { 1 } else { 0 })
    .bind(ev_loss_bb)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_for_user(pool: &Pool, user_id: Uuid, game_type: &str) -> sqlx::Result<Vec<WeaknessProfileRecord>> {
    sqlx::query_as::<_, WeaknessProfileRecord>(
        r#"SELECT id, user_id, game_type, category, attempts, correct, avg_ev_loss_bb, last_seen_at
           FROM weakness_profiles WHERE user_id = $1 AND game_type = $2"#,
    )
    .bind(user_id)
    .bind(game_type)
    .fetch_all(pool)
    .await
}
