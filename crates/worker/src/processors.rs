use tracing::{error, info, warn};

use rib_db::models::PlayerStatsRecord;
use rib_db::{hand_history, solve_jobs, solved_spots, stats as db_stats, Pool as DbPool};
use rib_parser::{parse_and_tag, stats::AggregateStats};

use crate::job::{ParseJob, SolveJob, WarmJob};

pub async fn process_parse(db: &DbPool, job: ParseJob) {
    let record = match hand_history::list_hand_histories_for_user(db, job.user_id).await {
        Ok(list) => list.into_iter().find(|h| h.id == job.hand_history_id),
        Err(e) => {
            error!(error = %e, "failed to load hand history row for parse job");
            return;
        }
    };
    let Some(record) = record else {
        warn!(id = %job.hand_history_id, "hand history row vanished before its parse job ran");
        return;
    };

    if let Err(e) = hand_history::mark_parsing(db, record.id).await {
        error!(error = %e, "failed to mark hand history as parsing");
    }

    let outcome = match parse_and_tag(&record.raw_text) {
        Ok(o) => o,
        Err(e) => {
            let _ = hand_history::mark_failed(db, record.id, &e.to_string()).await;
            return;
        }
    };

    let mut inserted = 0i32;
    for hand in &outcome.hands {
        let actions_json = serde_json::to_value(&hand.actions).unwrap_or_else(|_| serde_json::json!([]));
        let board: Vec<String> = hand.board.iter().map(|c| c.to_string()).collect();
        let hero_cards: Vec<String> = hand.hero_cards.iter().map(|c| c.to_string()).collect();
        let hero_position = hand.hero_position.map(|p| p.label().to_string());
        let res = hand_history::insert_parsed_hand(
            db,
            record.id,
            job.user_id,
            &hand.site,
            hand.site_hand_id.as_deref(),
            &hand.game_type,
            hand.table_size as i32,
            hero_position.as_deref(),
            Some(hand.big_blind_amount),
            &board,
            &hero_cards,
            actions_json,
            hand.result_bb,
            hand.went_to_showdown,
            hand.won_hand,
            &hand.tags,
        )
        .await;
        match res {
            Ok(Some(_)) => inserted += 1,
            Ok(None) => {} // already imported -- partial unique index made it a no-op
            Err(e) => warn!(error = %e, "failed to insert one parsed hand, continuing with the rest of the file"),
        }
    }

    if let Err(e) = hand_history::mark_parsed(db, record.id, inserted).await {
        error!(error = %e, "failed to mark hand history parsed");
    }

    let batch_stats = rib_parser::stats::aggregate(&outcome.hands);
    let existing = db_stats::get_player_stats(db, job.user_id, "nlhe").await.ok().flatten();
    let merged = merge_stats(existing, &batch_stats);
    if let Err(e) = db_stats::upsert_player_stats(db, job.user_id, "nlhe", &merged).await {
        error!(error = %e, "failed to upsert player stats");
    }

    info!(
        hand_history_id = %record.id,
        inserted,
        total_blocks = outcome.total_blocks,
        failed_blocks = outcome.failed_blocks,
        "parsed hand history file"
    );
}

/// Folds a new batch of hands into whatever stats were already stored,
/// weighted by sample size, so re-uploading more history is cheap (no need
/// to re-derive from the user's entire lifetime hand count every time).
fn merge_stats(existing: Option<PlayerStatsRecord>, batch: &AggregateStats) -> db_stats::StatsInput {
    let batch_n = batch.sample_size as f64;
    let Some(old) = existing else {
        return db_stats::StatsInput {
            sample_size: batch.sample_size,
            vpip: batch.vpip,
            pfr: batch.pfr,
            three_bet: batch.three_bet,
            fold_to_three_bet: batch.fold_to_three_bet,
            cbet_flop: batch.cbet_flop,
            fold_to_cbet_flop: batch.fold_to_cbet_flop,
            cbet_turn: batch.cbet_turn,
            wtsd: batch.wtsd,
            won_at_showdown: batch.won_at_showdown,
            aggression_factor: batch.aggression_factor,
            net_bb_per_100: batch.net_bb_per_100,
        };
    };
    let old_n = old.sample_size as f64;
    let total_n = (old_n + batch_n).max(1.0);
    let w = |old_v: Option<f64>, new_v: f64| (old_v.unwrap_or(0.0) * old_n + new_v * batch_n) / total_n;

    db_stats::StatsInput {
        sample_size: old.sample_size + batch.sample_size,
        vpip: w(old.vpip, batch.vpip),
        pfr: w(old.pfr, batch.pfr),
        three_bet: w(old.three_bet, batch.three_bet),
        fold_to_three_bet: w(old.fold_to_three_bet, batch.fold_to_three_bet),
        cbet_flop: w(old.cbet_flop, batch.cbet_flop),
        fold_to_cbet_flop: w(old.fold_to_cbet_flop, batch.fold_to_cbet_flop),
        cbet_turn: w(old.cbet_turn, batch.cbet_turn),
        wtsd: w(old.wtsd, batch.wtsd),
        won_at_showdown: w(old.won_at_showdown, batch.won_at_showdown),
        // Aggression factor is technically a ratio of two raw counts rather
        // than a per-hand rate, so weighting it by hand count is a slight
        // approximation rather than an exact merge -- fine for a dashboard
        // trend indicator, not used anywhere correctness-critical.
        aggression_factor: w(old.aggression_factor, batch.aggression_factor),
        net_bb_per_100: w(old.net_bb_per_100, batch.net_bb_per_100),
    }
}

pub async fn process_solve(db: &DbPool, job: SolveJob) {
    if let Err(e) = solve_jobs::mark_running(db, job.job_id).await {
        error!(error = %e, "failed to mark solve job running");
    }
    match rib_solver::solve(&job.request) {
        Ok(response) => {
            let value = serde_json::to_value(&response).unwrap_or_else(|_| serde_json::json!({}));
            if let Err(e) = solve_jobs::complete_solve_job(db, job.job_id, value).await {
                error!(error = %e, "failed to persist solve result");
            }
        }
        Err(e) => {
            if let Err(e2) = solve_jobs::fail_solve_job(db, job.job_id, &e.to_string()).await {
                error!(error = %e2, "failed to persist solve job failure");
            }
        }
    }
}

pub async fn process_warm(db: &DbPool, job: WarmJob) {
    let cache_key = job.key.cache_key();
    if matches!(solved_spots::get_solved_spot(db, &cache_key).await, Ok(Some(_))) {
        return; // already warm
    }
    match rib_solver::solve(&job.request) {
        Ok(response) => {
            let value = serde_json::to_value(&response).unwrap_or_else(|_| serde_json::json!({}));
            let board = job.key.board.clone();
            if let Err(e) = solved_spots::upsert_solved_spot(
                db,
                &cache_key,
                "nlhe",
                &format!("{:?}", job.key.pot_type),
                job.key.stack_bb as i32,
                job.key.hero_position.label(),
                job.key.villain_position.label(),
                &board,
                value,
                response.iterations_run as i32,
            )
            .await
            {
                error!(error = %e, cache_key, "failed to persist warmed solved spot");
            }
        }
        Err(e) => warn!(cache_key, error = %e, "failed to warm solved-spot cache entry"),
    }
}
