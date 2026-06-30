use std::time::Instant;
use rib_core::GameType;
use rib_solver::{NodeContext, SolveRequest};

fn solve_and_report(label: &str, board: Vec<&str>, pot: f32, facing: Option<f32>) {
    let req = SolveRequest {
        game: GameType::Nlhe,
        board: board.iter().map(|s| s.to_string()).collect(),
        effective_stack_bb: 100.0,
        starting_pot_bb: pot,
        hero_invested_bb: pot / 2.0,
        villain_invested_bb: pot / 2.0,
        hero_range: "33+,A8+,K9+,QTs+,JTs,T9s,98s".to_string(),
        villain_range: "33+,A5+,K8+,Q9+,J9s+,T9s,98s,87s,76s".to_string(),
        hero_is_in_position: facing.is_none(),
        context: match facing {
            Some(f) => NodeContext::FacingBet { size_bb: f },
            None => NodeContext::FirstToAct,
        },
        sizings: None,
        streets_to_extend: 0,
        iterations: 600,
    };
    let start = Instant::now();
    match rib_solver::solve(&req) {
        Ok(resp) => println!(
            "{label}: OK in {:?} -- board len {} -- {} combos x {} combos -- hero_ev={:.2}bb",
            start.elapsed(), board.len(), resp.n_hero_combos, resp.n_villain_combos, resp.hero_ev_bb
        ),
        Err(e) => println!("{label}: ERROR after {:?}: {e}", start.elapsed()),
    }
}

fn main() {
    solve_and_report("turn_barrel (4-card board, first to act)", vec!["7h", "8d", "2c", "Kh"], 12.0, None);
    solve_and_report("vs_turn_barrel (4-card board, facing bet)", vec!["7h", "8d", "2c", "Kh"], 12.0, Some(8.4));
    solve_and_report("river_bet (5-card board, first to act)", vec!["7h", "8d", "2c", "Kh", "3s"], 26.0, None);
    solve_and_report("vs_river_bet (5-card board, facing bet)", vec!["7h", "8d", "2c", "Kh", "3s"], 26.0, Some(19.5));
}
