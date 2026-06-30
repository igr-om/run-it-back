use serde_json::Value;

use crate::models::SolvedSpotRecord;
use crate::pool::Pool;

pub async fn get_solved_spot(pool: &Pool, cache_key: &str) -> sqlx::Result<Option<SolvedSpotRecord>> {
    sqlx::query_as::<_, SolvedSpotRecord>(
        r#"SELECT cache_key, game_type, pot_type, stack_bb, hero_position, villain_position,
                  board, response, iterations, solved_at
           FROM solved_spots WHERE cache_key = $1"#,
    )
    .bind(cache_key)
    .fetch_optional(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_solved_spot(
    pool: &Pool,
    cache_key: &str,
    game_type: &str,
    pot_type: &str,
    stack_bb: i32,
    hero_position: &str,
    villain_position: &str,
    board: &[String],
    response: Value,
    iterations: i32,
) -> sqlx::Result<()> {
    sqlx::query(
        r#"INSERT INTO solved_spots
            (cache_key, game_type, pot_type, stack_bb, hero_position, villain_position, board, response, iterations)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
           ON CONFLICT (cache_key) DO UPDATE SET
             response = EXCLUDED.response, iterations = EXCLUDED.iterations, solved_at = now()"#,
    )
    .bind(cache_key)
    .bind(game_type)
    .bind(pot_type)
    .bind(stack_bb)
    .bind(hero_position)
    .bind(villain_position)
    .bind(board)
    .bind(response)
    .bind(iterations)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_solved_spots(pool: &Pool) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM solved_spots").fetch_one(pool).await?;
    Ok(row.0)
}
