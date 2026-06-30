use serde::{Deserialize, Serialize};
use std::fmt;

/// Canonical position labels. Not every label is used at every table size;
/// `Position::for_table_size` returns the correctly ordered subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Position {
    Sb,
    Bb,
    Utg,
    Utg1,
    Utg2,
    Lj,
    Hj,
    Co,
    Btn,
}

impl Position {
    /// Returns positions in acting order (preflop first-to-act .. button) for
    /// a given number of seated players (2-9).
    pub fn for_table_size(n: usize) -> Vec<Position> {
        use Position::*;
        match n {
            2 => vec![Btn, Bb], // heads-up: BTN/SB is the same seat
            3 => vec![Btn, Sb, Bb],
            4 => vec![Co, Btn, Sb, Bb],
            5 => vec![Hj, Co, Btn, Sb, Bb],
            6 => vec![Utg, Hj, Co, Btn, Sb, Bb],
            7 => vec![Utg, Lj, Hj, Co, Btn, Sb, Bb],
            8 => vec![Utg, Utg1, Lj, Hj, Co, Btn, Sb, Bb],
            _ => vec![Utg, Utg1, Utg2, Lj, Hj, Co, Btn, Sb, Bb],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Position::Sb => "SB",
            Position::Bb => "BB",
            Position::Utg => "UTG",
            Position::Utg1 => "UTG+1",
            Position::Utg2 => "UTG+2",
            Position::Lj => "LJ",
            Position::Hj => "HJ",
            Position::Co => "CO",
            Position::Btn => "BTN",
        }
    }

    /// Rough categorization used by the drill generator / stats engine.
    pub fn is_blind(&self) -> bool {
        matches!(self, Position::Sb | Position::Bb)
    }

    pub fn is_late(&self) -> bool {
        matches!(self, Position::Co | Position::Btn)
    }

    pub fn is_early(&self) -> bool {
        matches!(self, Position::Utg | Position::Utg1 | Position::Utg2)
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}
