use serde_json::Value;
use uuid::Uuid;

use crate::models::{HandHistoryRecord, ParsedHandRecord};
use crate::pool::Pool;

pub async fn insert_hand_history(
    pool: &Pool,
    user_id: Uuid,
    site: &str,
    original_filename: Option<&str>,
    raw_text: &str,
) -> sqlx::Result<HandHistoryRecord> {
    sqlx::query_as::<_, HandHistoryRecord>(
        r#"INSERT INTO hand_histories (user_id, site, original_filename, raw_text, status)
           VALUES ($1, $2, $3, $4, 'pending')
           RETURNING id, user_id, site, original_filename, raw_text, hand_count, status, error, uploaded_at, parsed_at"#,
    )
    .bind(user_id)
    .bind(site)
    .bind(original_filename)
    .bind(raw_text)
    .fetch_one(pool)
    .await
}

pub async fn mark_parsing(pool: &Pool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE hand_histories SET status = 'parsing' WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_parsed(pool: &Pool, id: Uuid, hand_count: i32) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE hand_histories SET status = 'parsed', hand_count = $2, parsed_at = now() WHERE id = $1",
    )
    .bind(id)
    .bind(hand_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(pool: &Pool, id: Uuid, error: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE hand_histories SET status = 'failed', error = $2 WHERE id = $1")
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_hand_histories_for_user(pool: &Pool, user_id: Uuid) -> sqlx::Result<Vec<HandHistoryRecord>> {
    sqlx::query_as::<_, HandHistoryRecord>(
        r#"SELECT id, user_id, site, original_filename, raw_text, hand_count, status, error, uploaded_at, parsed_at
           FROM hand_histories WHERE user_id = $1 ORDER BY uploaded_at DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_parsed_hand(
    pool: &Pool,
    hand_history_id: Uuid,
    user_id: Uuid,
    site: &str,
    site_hand_id: Option<&str>,
    game_type: &str,
    table_size: i32,
    hero_position: Option<&str>,
    big_blind: Option<f64>,
    board: &[String],
    hero_cards: &[String],
    actions: Value,
    result_bb: f64,
    went_to_showdown: bool,
    won_hand: bool,
    tags: &[String],
) -> sqlx::Result<Option<Uuid>> {
    // Returns `None` when this hand was already imported before (same user
    // + site + site_hand_id) -- the partial unique index makes that a no-op
    // insert rather than an error, so re-uploading the same history file is
    // always safe.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"INSERT INTO parsed_hands
            (hand_history_id, user_id, site, site_hand_id, game_type, table_size, hero_position,
             big_blind, board, hero_cards, actions, result_bb, went_to_showdown, won_hand, tags)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
           ON CONFLICT (user_id, site, site_hand_id) WHERE site_hand_id IS NOT NULL DO NOTHING
           RETURNING id"#,
    )
    .bind(hand_history_id)
    .bind(user_id)
    .bind(site)
    .bind(site_hand_id)
    .bind(game_type)
    .bind(table_size)
    .bind(hero_position)
    .bind(big_blind)
    .bind(board)
    .bind(hero_cards)
    .bind(actions)
    .bind(result_bb)
    .bind(went_to_showdown)
    .bind(won_hand)
    .bind(tags)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn list_parsed_hands_for_user(pool: &Pool, user_id: Uuid, limit: i64) -> sqlx::Result<Vec<ParsedHandRecord>> {
    sqlx::query_as::<_, ParsedHandRecord>(
        r#"SELECT id, hand_history_id, user_id, site, site_hand_id, game_type, table_size, hero_position,
                  big_blind, played_at, board, hero_cards, actions, result_bb, went_to_showdown, won_hand,
                  tags, created_at
           FROM parsed_hands WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2"#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn count_parsed_hands_for_user(pool: &Pool, user_id: Uuid) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM parsed_hands WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}
