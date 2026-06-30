use std::time::Instant;
use rib_core::GameType;
use rib_solver::{NodeContext, SolveRequest};

fn main() {
    // The worst-case real scenario: a preflop open-raise drill with 100%
    // ranges on both sides -- exactly what was hanging before the fix.
    let req = SolveRequest {
        game: GameType::Nlhe,
        board: vec![],
        effective_stack_bb: 100.0,
        starting_pot_bb: 1.5,
        hero_invested_bb: 0.0,
        villain_invested_bb: 1.0,
        hero_range: "100%".to_string(),
        villain_range: "100%".to_string(),
        hero_is_in_position: true,
        context: NodeContext::FirstToAct,
        sizings: None,
        streets_to_extend: 0,
        iterations: 800,
    };
    let start = Instant::now();
    let result = rib_solver::solve(&req);
    let elapsed = start.elapsed();
    match result {
        Ok(resp) => {
            println!("OK in {elapsed:?} -- {} hero combos x {} villain combos, hero_ev={:.3}bb", resp.n_hero_combos, resp.n_villain_combos, resp.hero_ev_bb);
            for w in &resp.warnings {
                println!("warning: {w}");
            }
            println!("actions: {:?}", resp.hero_strategy.actions.iter().map(|a| a.label()).collect::<Vec<_>>());
            for label in ["AA", "AKs", "72o", "27o", "32o"] {
                if let Some(freqs) = resp.hero_strategy.frequencies.get(label) {
                    println!("{label}: {freqs:?}");
                }
            }
        }
        Err(e) => println!("ERROR after {elapsed:?}: {e}"),
    }
}
