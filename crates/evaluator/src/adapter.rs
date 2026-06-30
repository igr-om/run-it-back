//! Thin wrapper tying `rib_core::Card` straight into `handrank::evaluate`.
//!
//! This module used to be the boundary that converted our `Card` type into
//! the `robopoker` crate's internal representation. Since the evaluator is
//! now self-contained (see `handrank.rs`), there's no conversion left to
//! do -- this module just exists so the rest of the codebase's `use
//! crate::adapter::strength` keeps working unchanged.

use rib_core::Card;

pub use crate::handrank::Strength;

/// Strength of the best 5-card hand made from `hole` (2 cards) + `board`
/// (3, 4, or 5 cards). Higher `Strength` always wins; ties are exact
/// equality (split pot).
pub fn strength(hole: [Card; 2], board: &[Card]) -> Strength {
    let mut all = Vec::with_capacity(7);
    all.extend_from_slice(&hole);
    all.extend_from_slice(board);
    crate::handrank::evaluate(&all)
}
