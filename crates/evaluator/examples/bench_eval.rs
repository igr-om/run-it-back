use std::time::Instant;
use rib_core::Card;
use rib_evaluator::handrank::evaluate;

fn main() {
    let cards: Vec<Card> = "As Kh Qd Jc 9s 7h 2c".split(' ').map(|s| s.parse().unwrap()).collect();
    let n = 200_000u32;
    let start = Instant::now();
    let mut acc = 0usize;
    for _ in 0..n {
        let s = evaluate(&cards);
        acc = acc.wrapping_add(format!("{:?}", s.ranking()).len());
    }
    let elapsed = start.elapsed();
    println!("{} 7-card evaluations in {:?} ({:.0}/sec) [{}]", n, elapsed, n as f64 / elapsed.as_secs_f64(), acc % 7);
}
