use std::time::Instant;
use rib_core::Card;
use rib_evaluator::equity_heads_up;

fn main() {
    let hero: [Card; 2] = ["As".parse().unwrap(), "Kd".parse().unwrap()];
    let villain: [Card; 2] = ["7h".parse().unwrap(), "2c".parse().unwrap()];
    let board: Vec<Card> = vec![]; // preflop -- needs all 5 dealt via Monte Carlo

    let start = Instant::now();
    let r = equity_heads_up(hero, villain, &board, 120);
    let one_call = start.elapsed();
    println!("single equity_heads_up(board=[], 120 samples) call: {:?} -- win={:.3}", one_call, r.win);

    let n = 200u32;
    let start = Instant::now();
    for _ in 0..n {
        let _ = equity_heads_up(hero, villain, &board, 120);
    }
    let elapsed = start.elapsed();
    println!("{n} sequential calls in {:?} ({:?} per call avg)", elapsed, elapsed / n);
}
