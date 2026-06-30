use rand::seq::SliceRandom;
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use rib_core::{Card, Combo};

use crate::adapter::strength;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct EquityResult {
    pub win: f32,
    pub tie: f32,
    pub lose: f32,
    /// Equivalent equity counting ties as a fractional win, the number most
    /// UIs display ("62.4% equity").
    pub equity: f32,
    pub samples: u32,
}

pub(crate) fn remaining_deck(used: &[Card]) -> Vec<Card> {
    Card::deck().into_iter().filter(|c| !used.contains(c)).collect()
}

/// Heads-up Monte Carlo equity for a specific hole-card matchup, with the
/// board completed at random for however many cards remain. Exact (not
/// sampled) on the river, since there's nothing left to run out.
pub fn equity_heads_up(hero: [Card; 2], villain: [Card; 2], board: &[Card], iterations: u32) -> EquityResult {
    if board.len() == 5 {
        let ord = strength(hero, board).cmp(&strength(villain, board));
        return result_from_ordering(ord, 1);
    }
    let used: Vec<Card> = hero.iter().chain(villain.iter()).chain(board.iter()).copied().collect();
    let deck = remaining_deck(&used);
    let need = 5 - board.len();

    let (win, tie, lose) = (0..iterations)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::thread_rng();
            let mut pool = deck.clone();
            pool.shuffle(&mut rng);
            let mut full_board = board.to_vec();
            full_board.extend_from_slice(&pool[0..need]);
            match strength(hero, &full_board).cmp(&strength(villain, &full_board)) {
                Ordering::Greater => (1u64, 0u64, 0u64),
                Ordering::Equal => (0, 1, 0),
                Ordering::Less => (0, 0, 1),
            }
        })
        .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));

    let total = (win + tie + lose).max(1) as f32;
    EquityResult {
        win: win as f32 / total,
        tie: tie as f32 / total,
        lose: lose as f32 / total,
        equity: (win as f32 + 0.5 * tie as f32) / total,
        samples: iterations,
    }
}

pub(crate) fn result_from_ordering(ord: Ordering, samples: u32) -> EquityResult {
    match ord {
        Ordering::Greater => EquityResult { win: 1.0, tie: 0.0, lose: 0.0, equity: 1.0, samples },
        Ordering::Equal => EquityResult { win: 0.0, tie: 1.0, lose: 0.0, equity: 0.5, samples },
        Ordering::Less => EquityResult { win: 0.0, tie: 0.0, lose: 1.0, equity: 0.0, samples },
    }
}

/// Every literal 2-card combination belonging to a 169-grid `Combo`,
/// excluding any that share a card with `blocked`.
pub fn specific_combos(combo: Combo, blocked: &[Card]) -> Vec<[Card; 2]> {
    use rib_core::Suit;
    let mut out = Vec::new();
    let suits = Suit::ALL;
    if combo.hi == combo.lo {
        for i in 0..4 {
            for j in (i + 1)..4 {
                let pair = [Card::new(combo.hi, suits[i]), Card::new(combo.lo, suits[j])];
                if !pair.iter().any(|c| blocked.contains(c)) {
                    out.push(pair);
                }
            }
        }
    } else {
        for &s1 in suits.iter() {
            for &s2 in suits.iter() {
                let suited = s1 == s2;
                let wanted_suited = combo.kind == rib_core::Suitedness::Suited;
                if suited != wanted_suited {
                    continue;
                }
                let pair = [Card::new(combo.hi, s1), Card::new(combo.lo, s2)];
                if !pair.iter().any(|c| blocked.contains(c)) {
                    out.push(pair);
                }
            }
        }
    }
    out
}

/// A weighted preflop range: combo -> inclusion frequency in [0, 1].
pub type WeightedRange = Vec<(Combo, f32)>;

/// Monte Carlo equity of one specific hand against an entire weighted range
/// (e.g. "AA vs a CO opening range"), sampling an opponent combo each
/// iteration in proportion to its weight and number of specific card
/// combinations, then running out the board.
pub fn equity_vs_range(
    hero: [Card; 2],
    range: &WeightedRange,
    board: &[Card],
    iterations: u32,
) -> EquityResult {
    // Build a flat, weighted pool of (specific hand, weight) pairs once.
    let blocked: Vec<Card> = hero.iter().chain(board.iter()).copied().collect();
    let mut pool: Vec<([Card; 2], f32)> = Vec::new();
    for (combo, weight) in range {
        if *weight <= 0.0 {
            continue;
        }
        for specific in specific_combos(*combo, &blocked) {
            pool.push((specific, *weight));
        }
    }
    if pool.is_empty() {
        return EquityResult::default();
    }
    let total_weight: f32 = pool.iter().map(|(_, w)| w).sum();

    let (win, tie, lose) = (0..iterations)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::thread_rng();
            let villain = weighted_pick(&pool, total_weight, &mut rng).0;
            let used: Vec<Card> = hero
                .iter()
                .chain(villain.iter())
                .chain(board.iter())
                .copied()
                .collect();
            let need = 5 - board.len();
            let mut full_board = board.to_vec();
            if need > 0 {
                let mut deck = remaining_deck(&used);
                deck.shuffle(&mut rng);
                full_board.extend_from_slice(&deck[0..need]);
            }
            match strength(hero, &full_board).cmp(&strength(villain, &full_board)) {
                Ordering::Greater => (1u64, 0u64, 0u64),
                Ordering::Equal => (0, 1, 0),
                Ordering::Less => (0, 0, 1),
            }
        })
        .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));

    let total = (win + tie + lose).max(1) as f32;
    EquityResult {
        win: win as f32 / total,
        tie: tie as f32 / total,
        lose: lose as f32 / total,
        equity: (win as f32 + 0.5 * tie as f32) / total,
        samples: iterations,
    }
}

pub(crate) fn weighted_pick<'a, T>(pool: &'a [(T, f32)], total_weight: f32, rng: &mut impl Rng) -> &'a (T, f32) {
    let mut target = rng.gen::<f32>() * total_weight;
    for item in pool {
        target -= item.1;
        if target <= 0.0 {
            return item;
        }
    }
    &pool[pool.len() - 1]
}

/// Range vs range equity, used for the "equity" panel in the range explorer.
/// Averages `equity_vs_range` over every combo in `range_a`, weighted by that
/// combo's own inclusion weight and specific-combo count.
pub fn range_vs_range(range_a: &WeightedRange, range_b: &WeightedRange, board: &[Card], iterations_per_combo: u32) -> EquityResult {
    let mut acc = (0f64, 0f64, 0f64);
    let mut weight_sum = 0f64;
    for (combo, w) in range_a {
        if *w <= 0.0 {
            continue;
        }
        for hand in specific_combos(*combo, board) {
            let r = equity_vs_range(hand, range_b, board, iterations_per_combo);
            let weight = *w as f64;
            acc.0 += r.win as f64 * weight;
            acc.1 += r.tie as f64 * weight;
            acc.2 += r.lose as f64 * weight;
            weight_sum += weight;
        }
    }
    if weight_sum == 0.0 {
        return EquityResult::default();
    }
    EquityResult {
        win: (acc.0 / weight_sum) as f32,
        tie: (acc.1 / weight_sum) as f32,
        lose: (acc.2 / weight_sum) as f32,
        equity: ((acc.0 + 0.5 * acc.1) / weight_sum) as f32,
        samples: iterations_per_combo,
    }
}
