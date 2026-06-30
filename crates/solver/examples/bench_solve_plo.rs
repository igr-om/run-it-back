use std::time::Instant;
use rib_core::GameType;
use rib_solver::{NodeContext, SolveRequest};

fn main() {
    // A real PLO spot: hero has a strong double-suited hand, villain has a
    // reasonably wide range, first to act on a flop.
    let req = SolveRequest {
        game: GameType::Plo,
        board: vec!["7h".into(), "8d".into(), "2c".into()],
        effective_stack_bb: 100.0,
        starting_pot_bb: 8.0,
        hero_invested_bb: 4.0,
        villain_invested_bb: 4.0,
        hero_range: "AAKK,AAQQ,AAJJ,KKQQ,AKQJds,AKQTds,AKJTds".to_string(),
        villain_range: "TT99,9988,8877,T987ss,9876ss,JT98ds,KQJTds".to_string(),
        hero_is_in_position: true,
        context: NodeContext::FirstToAct,
        sizings: None,
        streets_to_extend: 0,
        iterations: 500,
    };
    let start = Instant::now();
    let result = rib_solver::solve(&req);
    let elapsed = start.elapsed();
    match result {
        Ok(resp) => {
            println!("OK in {elapsed:?} -- {} hero combos x {} villain combos, hero_ev={:.3}bb", resp.n_hero_combos, resp.n_villain_combos, resp.hero_ev_bb);
            println!("actions: {:?}", resp.hero_strategy.actions.iter().map(|a| a.label()).collect::<Vec<_>>());
            for (label, freqs) in resp.hero_strategy.frequencies.iter().take(5) {
                println!("{label}: {freqs:?}");
            }
            for w in &resp.warnings {
                println!("warning: {w}");
            }
        }
        Err(e) => println!("ERROR after {elapsed:?}: {e}"),
    }
}
