//! Omaha hand evaluation: best 5-card hand using *exactly* 2 of the 4 hole
//! cards and *exactly* 3 of the board cards, maximized over every valid
//! choice. This is the one rule that makes Omaha evaluation different from
//! hold'em -- you can't just hand all your cards plus the board to a
//! 7-card evaluator and take the best 5, because that would let a hand use
//! 3, 4, or even all 4 hole cards, which isn't legal in Omaha.
//!
//! The actual hand-ranking math (straights, flushes, kickers, ...) is
//! identical to hold'em once you've picked which exact 5 cards to
//! evaluate, so this reuses `adapter::strength` (and therefore robopoker's
//! real evaluator) for that part -- nothing about *ranking* a 5-card hand
//! is Omaha-specific, only *which 5 cards you're allowed to use* is.

use std::cmp::Ordering;

use rand::seq::SliceRandom;
use rayon::prelude::*;

use rib_core::{Card, OmahaHole, OmahaRangeEntry};

use crate::adapter::strength;
use crate::equity::{remaining_deck, result_from_ordering, weighted_pick, EquityResult};
use crate::Strength;

/// Every way to choose exactly `k` items from `items`, order-independent.
/// `items` is always tiny here (4 hole cards or up to 5 board cards), so a
/// plain recursive enumeration is simpler and plenty fast -- no need for a
/// general-purpose combinatorics crate for numbers this small.
fn combinations<T: Copy>(items: &[T], k: usize) -> Vec<Vec<T>> {
    fn helper<T: Copy>(items: &[T], k: usize, start: usize, current: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if current.len() == k {
            out.push(current.clone());
            return;
        }
        if start >= items.len() {
            return;
        }
        for i in start..items.len() {
            current.push(items[i]);
            helper(items, k, i + 1, current, out);
            current.pop();
        }
    }
    if k == 0 {
        return vec![vec![]];
    }
    if items.len() < k {
        return vec![];
    }
    let mut out = Vec::new();
    helper(items, k, 0, &mut Vec::new(), &mut out);
    out
}

/// The best `Strength` achievable with `hole`'s 4 cards on `board` (which
/// must have at least 3 cards -- there's no "evaluating" an Omaha hand
/// before the flop, same as hold'em).
pub fn omaha_strength(hole: OmahaHole, board: &[Card]) -> Strength {
    debug_assert!(board.len() >= 3, "Omaha hands need at least a flop to evaluate");
    let mut best: Option<Strength> = None;
    for (a, b) in hole.two_card_combos() {
        for board3 in combinations(board, 3) {
            let s = strength([a, b], &board3);
            best = Some(match best {
                Some(cur) if cur >= s => cur,
                _ => s,
            });
        }
    }
    best.expect("non-empty board and hole combos always produce at least one candidate hand")
}

/// Compare two Omaha hands on the same board. `Ordering` is from `a`'s
/// perspective (`Greater` => `a` has the better hand).
pub fn omaha_compare(a: OmahaHole, b: OmahaHole, board: &[Card]) -> Ordering {
    omaha_strength(a, board).cmp(&omaha_strength(b, board))
}

/// Winning hand index/indices (ties split the pot) among a multi-way
/// showdown.
pub fn omaha_showdown(hands: &[OmahaHole], board: &[Card]) -> Vec<usize> {
    let strengths: Vec<_> = hands.iter().map(|h| omaha_strength(*h, board)).collect();
    let best = strengths.iter().max().unwrap();
    strengths.iter().enumerate().filter(|(_, s)| *s == best).map(|(i, _)| i).collect()
}

/// Heads-up Monte Carlo equity for a specific Omaha matchup, completing
/// the board at random for however many cards remain. Exact (not sampled)
/// once the board is complete, since there's nothing left to run out.
pub fn omaha_equity_heads_up(hero: OmahaHole, villain: OmahaHole, board: &[Card], iterations: u32) -> EquityResult {
    if board.len() == 5 {
        let ord = omaha_compare(hero, villain, board);
        return result_from_ordering(ord, 1);
    }
    let used: Vec<Card> = hero.cards().iter().chain(villain.cards().iter()).chain(board.iter()).copied().collect();
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
            match omaha_compare(hero, villain, &full_board) {
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

/// Monte Carlo equity of one specific Omaha hand against a weighted range
/// of opponent holdings (see `rib_core::parse_omaha_range`), sampling an
/// opponent combo each iteration in proportion to its weight, then running
/// out the board.
pub fn omaha_equity_vs_range(hero: OmahaHole, range: &[OmahaRangeEntry], board: &[Card], iterations: u32) -> EquityResult {
    let blocked: Vec<Card> = hero.cards().iter().chain(board.iter()).copied().collect();
    let pool: Vec<(OmahaHole, f32)> = range
        .iter()
        .filter(|e| e.weight > 0.0 && !e.hole.cards().iter().any(|c| blocked.contains(c)))
        .map(|e| (e.hole, e.weight))
        .collect();
    if pool.is_empty() {
        return EquityResult::default();
    }
    let total_weight: f32 = pool.iter().map(|(_, w)| w).sum();

    let (win, tie, lose) = (0..iterations)
        .into_par_iter()
        .map(|_| {
            let mut rng = rand::thread_rng();
            let villain = weighted_pick(&pool, total_weight, &mut rng).0;
            let used: Vec<Card> =
                hero.cards().iter().chain(villain.cards().iter()).chain(board.iter()).copied().collect();
            let need = 5 - board.len();
            let mut full_board = board.to_vec();
            if need > 0 {
                let mut deck = remaining_deck(&used);
                deck.shuffle(&mut rng);
                full_board.extend_from_slice(&deck[0..need]);
            }
            match omaha_compare(hero, villain, &full_board) {
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

/// Range vs range equity. Averages `omaha_equity_vs_range` over every combo
/// in `range_a` (weighted by that combo's own weight), capping how many
/// hero combos actually get sampled when the range is huge -- a single PLO
/// range can have thousands of specific combos, and running a full Monte
/// Carlo for every single one is unnecessary for a UI-facing estimate.
pub fn omaha_range_vs_range(
    range_a: &[OmahaRangeEntry],
    range_b: &[OmahaRangeEntry],
    board: &[Card],
    iterations_per_combo: u32,
    max_hero_combos: usize,
) -> EquityResult {
    let mut rng = rand::thread_rng();
    let mut sample: Vec<&OmahaRangeEntry> = range_a.iter().filter(|e| e.weight > 0.0).collect();
    if sample.len() > max_hero_combos {
        sample.partial_shuffle(&mut rng, max_hero_combos);
        sample.truncate(max_hero_combos);
    }

    let mut acc = (0f64, 0f64, 0f64);
    let mut weight_sum = 0f64;
    for entry in sample {
        let r = omaha_equity_vs_range(entry.hole, range_b, board, iterations_per_combo);
        let weight = entry.weight as f64;
        acc.0 += r.win as f64 * weight;
        acc.1 += r.tie as f64 * weight;
        acc.2 += r.lose as f64 * weight;
        weight_sum += weight;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn cards(s: &str) -> Vec<Card> {
        rib_core::parse_cards(s).unwrap()
    }

    #[test]
    fn higher_flush_beats_lower_flush_on_same_board() {
        // Both hands can complete a spade flush here (board has 3
        // spades); the assertion is just that ace-king-high beats
        // queen-high among two legal flushes on the same board.
        let hole = OmahaHole::from_str("AsKs2c2d").unwrap();
        let board = cards("5s 9s Qs 2h 7c");
        let s = omaha_strength(hole, &board);
        let other = OmahaHole::from_str("2s4s7d8d").unwrap();
        let other_s = omaha_strength(other, &board);
        assert!(s > other_s);
    }

    #[test]
    fn must_use_exactly_two_hole_cards_for_a_flush() {
        // Board has 4 spades + 1 blank. Hero only has ONE spade among
        // their 4 hole cards -- every 2-card hole subset therefore
        // contains at most 1 spade, so no legal (2 hole + 3 board)
        // selection can reach 5 spades. A buggy evaluator that just took
        // "best 5 of all 9 hole+board cards" (ignoring the exactly-2-from-
        // hole rule) WOULD wrongly find a flush here (1 hole spade + the
        // board's 4 spades = 5). The correct answer is no flush at all.
        let board = cards("5s 9s Qs 2s 7h");
        let one_spade = OmahaHole::from_str("AsKh2d3c").unwrap();
        let s = omaha_strength(one_spade, &board);

        // The worst possible flush (any suit) still ranks above every
        // non-flush hand, so this threshold cleanly answers "is `s` a
        // flush at all" regardless of kickers.
        let worst_possible_flush =
            strength([Card::from_str("2s").unwrap(), Card::from_str("3s").unwrap()], &cards("4s 5s 7s"));
        assert!(s < worst_possible_flush, "hero only holds 1 spade -- a flush here would mean the exactly-2-hole-card rule was violated");

        // Sanity check the rest of the harness: a hand that legitimately
        // holds 2 spades on the same board SHOULD make the flush.
        let two_spades = OmahaHole::from_str("AsKs2d3c").unwrap();
        let s2 = omaha_strength(two_spades, &board);
        assert!(s2 >= worst_possible_flush, "2 hole spades + 3 board spades should legally complete a flush");
        assert!(s2 > s);
    }

    #[test]
    fn quads_possible_with_pocket_pair_plus_board_pair() {
        let hole = OmahaHole::from_str("AsAh2c3d").unwrap();
        let board = cards("Ad Ac 7h 9s 2h");
        let s = omaha_strength(hole, &board);
        let weaker = OmahaHole::from_str("KsKh2c3d").unwrap();
        let weaker_s = omaha_strength(weaker, &board);
        assert!(s > weaker_s);
    }

    #[test]
    fn equity_heads_up_runs_and_sums_to_one() {
        let hero = OmahaHole::from_str("AsAhKdKc").unwrap();
        let villain = OmahaHole::from_str("2s2h3d3c").unwrap();
        let board = cards("Ad 7h 2c");
        let r = omaha_equity_heads_up(hero, villain, &board, 200);
        let total = r.win + r.tie + r.lose;
        assert!((total - 1.0).abs() < 0.01);
        assert!(r.win > r.lose); // hero flopped top set vs villain's bottom set
    }
}
