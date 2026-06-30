use rib_core::{Card, Combo, OmahaRangeEntry};
use rib_evaluator::specific_combos;

/// One concrete hand in a player's solving universe -- 2 cards for NLHE,
/// 4 for Omaha. `class_label` is whatever the UI groups this combo under
/// (a 169-grid label like "AKs" for NLHE, or a rank-shorthand like "AAKK"
/// for Omaha) and `class_weight` is the prior weight inherited from that
/// class. The rest of the solver (CFR, payoff precomputation) only ever
/// touches `cards` generically (overlap checks, equity lookups), so it
/// doesn't care which game a given `HandIndex` came from.
#[derive(Debug, Clone)]
pub struct HandIndex {
    pub cards: Vec<Card>,
    pub class_label: String,
    pub class_weight: f32,
}

/// Build the set of specific hands a weighted 169-class NLHE range reduces
/// to once cards already visible (the board) are removed.
pub fn build_universe(range: &[(Combo, f32)], board: &[Card]) -> Vec<HandIndex> {
    let mut out = Vec::new();
    for (combo, weight) in range {
        if *weight <= 0.0 {
            continue;
        }
        for cards in specific_combos(*combo, board) {
            out.push(HandIndex { cards: cards.to_vec(), class_label: combo.label(), class_weight: *weight });
        }
    }
    out
}

/// Build the set of specific hands a weighted Omaha range reduces to once
/// board cards are removed. Unlike NLHE's 169-class range, an Omaha range
/// is already a list of specific 4-card combos (see
/// `rib_core::parse_omaha_range`) -- there's no further class-to-combo
/// expansion needed here, just board-conflict filtering.
pub fn build_omaha_universe(range: &[OmahaRangeEntry], board: &[Card]) -> Vec<HandIndex> {
    range
        .iter()
        .filter(|e| e.weight > 0.0 && !e.hole.cards().iter().any(|c| board.contains(c)))
        .map(|e| HandIndex { cards: e.hole.cards().to_vec(), class_label: e.hole.rank_shorthand(), class_weight: e.weight })
        .collect()
}

pub fn conflicts(a: &[Card], b: &[Card]) -> bool {
    a.iter().any(|c| b.contains(c))
}

/// Caps a combo universe at `max` hands via weighted random sampling
/// (without replacement). A "100%" NLHE range has 1,326 specific combos
/// (an unrestricted Omaha range would have far more); the payoff
/// precomputation that has to run *before a single CFR iteration* is
/// O(hero_combos x villain_combos), each pair needing its own Monte Carlo
/// equity calculation -- that product is computationally infeasible for
/// an interactive solve no matter how few iterations you ask for
/// afterward.
///
/// Sampling first collapses to *one representative combo per class*
/// before sampling, rather than sampling directly over every specific
/// combo -- otherwise a small `max` can (and in testing, did) end up with
/// zero representation for an entire class like "AA" purely by random
/// chance, leaving gaps in the displayed range grid that have nothing to
/// do with the actual solved strategy. With class-deduplication first,
/// whichever classes do make it into a capped sample are each properly
/// represented by a real, valid combo.
pub fn subsample(hands: &[HandIndex], max: usize, rng: &mut impl rand::Rng) -> Vec<HandIndex> {
    use rand::seq::SliceRandom;
    use std::collections::HashMap;

    let mut by_class: HashMap<String, HandIndex> = HashMap::new();
    for h in hands {
        by_class.entry(h.class_label.clone()).or_insert_with(|| h.clone());
    }
    let deduped: Vec<HandIndex> = by_class.into_values().collect();

    if deduped.len() <= max {
        return deduped;
    }
    deduped
        .choose_multiple_weighted(rng, max, |h| h.class_weight.max(1e-6) as f64)
        .expect("class weights are always positive and finite")
        .cloned()
        .collect()
}
