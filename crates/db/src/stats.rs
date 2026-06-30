use uuid::Uuid;

use crate::models::PlayerStatsRecord;
use crate::pool::Pool;

#[derive(Debug, Clone, Default)]
pub struct StatsInput {
    pub sample_size: i32,
    pub vpip: f64,
    pub pfr: f64,
    pub three_bet: f64,
    pub fold_to_three_bet: f64,
    pub cbet_flop: f64,
    pub fold_to_cbet_flop: f64,
    pub cbet_turn: f64,
    pub wtsd: f64,
    pub won_at_showdown: f64,
    pub aggression_factor: f64,
    pub net_bb_per_100: f64,
}

pub async fn upsert_player_stats(pool: &Pool, user_id: Uuid, game_type: &str, s: &StatsInput) -> sqlx::Result<()> {
    sqlx::query(
        r#"INSERT INTO player_stats
            (user_id, game_type, sample_size, vpip, pfr, three_bet, fold_to_three_bet,
             cbet_flop, fold_to_cbet_flop, cbet_turn, wtsd, won_at_showdown, aggression_factor,
             net_bb_per_100, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14, now())
           ON CONFLICT (user_id, game_type) DO UPDATE SET
             sample_size = EXCLUDED.sample_size,
             vpip = EXCLUDED.vpip,
             pfr = EXCLUDED.pfr,
             three_bet = EXCLUDED.three_bet,
             fold_to_three_bet = EXCLUDED.fold_to_three_bet,
             cbet_flop = EXCLUDED.cbet_flop,
             fold_to_cbet_flop = EXCLUDED.fold_to_cbet_flop,
             cbet_turn = EXCLUDED.cbet_turn,
             wtsd = EXCLUDED.wtsd,
             won_at_showdown = EXCLUDED.won_at_showdown,
             aggression_factor = EXCLUDED.aggression_factor,
             net_bb_per_100 = EXCLUDED.net_bb_per_100,
             updated_at = now()"#,
    )
    .bind(user_id)
    .bind(game_type)
    .bind(s.sample_size)
    .bind(s.vpip)
    .bind(s.pfr)
    .bind(s.three_bet)
    .bind(s.fold_to_three_bet)
    .bind(s.cbet_flop)
    .bind(s.fold_to_cbet_flop)
    .bind(s.cbet_turn)
    .bind(s.wtsd)
    .bind(s.won_at_showdown)
    .bind(s.aggression_factor)
    .bind(s.net_bb_per_100)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_player_stats(pool: &Pool, user_id: Uuid, game_type: &str) -> sqlx::Result<Option<PlayerStatsRecord>> {
    sqlx::query_as::<_, PlayerStatsRecord>(
        r#"SELECT id, user_id, game_type, sample_size, vpip, pfr, three_bet, fold_to_three_bet,
                  cbet_flop, fold_to_cbet_flop, cbet_turn, wtsd, won_at_showdown, aggression_factor,
                  net_bb_per_100, updated_at
           FROM player_stats WHERE user_id = $1 AND game_type = $2"#,
    )
    .bind(user_id)
    .bind(game_type)
    .fetch_optional(pool)
    .await
}
