use rib_solver::{curated_preflop_seed_list, NodeContext, SolveRequest, SpotKey};

use crate::job::WarmJob;
use crate::pool::WorkerPool;

/// Builds a `SolveRequest` for a preflop `SpotKey`. Both ranges are the
/// full 169-combo grid ("100%") -- this isn't a simplification so much as
/// the textbook-correct way to compute an opening/defending range from
/// scratch: real preflop solvers find out *which* hands should raise/call/
/// fold by solving with every hand live on both sides, not by being told
/// the answer in advance. (Postflop/custom spots, solved live from the
/// trainer or range explorer, let the user supply narrower ranges, since
/// those *should* reflect "what range realistically got to this spot".)
fn request_for_preflop_spot(key: &SpotKey) -> SolveRequest {
    let pot_type_initial_raise_bb = match key.pot_type {
        rib_core::PotType::Srp => 2.0,
        rib_core::PotType::ThreeBet => 3.0,
        rib_core::PotType::FourBet => 4.0,
        rib_core::PotType::FiveBetPlus => 5.0,
        rib_core::PotType::LimpedPot => 1.0,
    };
    SolveRequest {
        game: key.game,
        board: vec![],
        effective_stack_bb: key.stack_bb as f32,
        starting_pot_bb: pot_type_initial_raise_bb + 1.0, // blinds + the raises already in
        hero_invested_bb: 1.0,
        villain_invested_bb: 1.0,
        hero_range: "100%".to_string(),
        villain_range: "100%".to_string(),
        hero_is_in_position: false,
        context: NodeContext::FirstToAct,
        sizings: None,
        streets_to_extend: 0,
        iterations: 800,
    }
}

/// Enqueues every curated preflop spot as a (lowest-priority) warm job.
/// Safe to call on every server startup -- `process_warm` checks the cache
/// first and is a no-op for anything already solved.
pub fn enqueue_curated_warm_jobs(pool: &WorkerPool) -> usize {
    let keys = curated_preflop_seed_list();
    let n = keys.len();
    for key in keys {
        let request = request_for_preflop_spot(&key);
        pool.submit_warm(WarmJob { key, request });
    }
    n
}
