use std::cmp::Ordering;

use rib_core::Card;

use crate::adapter::strength;

/// Compare two hole-card hands given a (3, 4, or 5 card) board. `Ordering`
/// is from the perspective of `a` (Greater = `a` wins).
pub fn compare(a: [Card; 2], b: [Card; 2], board: &[Card]) -> Ordering {
    strength(a, board).cmp(&strength(b, board))
}

/// Result of a multi-way showdown: indices of the winning hand(s) among the
/// input slice (ties split the pot between all returned indices).
pub fn showdown(hands: &[[Card; 2]], board: &[Card]) -> Vec<usize> {
    let strengths: Vec<_> = hands.iter().map(|h| strength(*h, board)).collect();
    let best = strengths.iter().max().unwrap();
    strengths
        .iter()
        .enumerate()
        .filter(|(_, s)| *s == best)
        .map(|(i, _)| i)
        .collect()
}
