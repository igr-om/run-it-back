use serde::{Deserialize, Serialize};
use std::fmt;

use crate::game::BetCategory;

/// A single action a player can take. Bet/Raise sizes are stored as the
/// *total chips put in this street* (not the delta), in big blinds, which is
/// the convention used end-to-end (tree building, drill grading, UI).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(f32),
    Raise(f32),
    AllIn(f32),
}

impl Action {
    pub fn label(&self) -> String {
        match self {
            Action::Fold => "Fold".into(),
            Action::Check => "Check".into(),
            Action::Call => "Call".into(),
            Action::Bet(x) => format!("Bet {x:.1}bb"),
            Action::Raise(x) => format!("Raise to {x:.1}bb"),
            Action::AllIn(x) => format!("All-in {x:.1}bb"),
        }
    }

    pub fn is_aggressive(&self) -> bool {
        matches!(self, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_))
    }

    pub fn is_voluntary_continue(&self) -> bool {
        !matches!(self, Action::Fold)
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A named bet sizing expressed as a fraction of the current pot, used both
/// to label solver tree branches ("33%", "75%", "Pot", "2.5x", ...) and to
/// drive the bet-category classifier in `classify_bet`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SizingOption {
    pub label: String,
    pub pot_fraction: f32,
}

impl SizingOption {
    pub fn new(label: impl Into<String>, pot_fraction: f32) -> Self {
        Self { label: label.into(), pot_fraction }
    }

    pub fn quarter() -> Self { Self::new("25%", 0.25) }
    pub fn third() -> Self { Self::new("33%", 0.33) }
    pub fn half() -> Self { Self::new("50%", 0.50) }
    pub fn two_thirds() -> Self { Self::new("66%", 0.66) }
    pub fn three_quarters() -> Self { Self::new("75%", 0.75) }
    pub fn pot() -> Self { Self::new("Pot", 1.0) }
    pub fn overbet() -> Self { Self::new("150%", 1.5) }
    pub fn jam() -> Self { Self::new("All-in", f32::INFINITY) }

    /// A compact, GTOWizard-style default sizing tree: small/medium/large
    /// c-bet sizes plus pot and an all-in option. Used when the caller
    /// doesn't specify a custom sizing set for a solve request, for the
    /// *opening* bet/raise on a street.
    pub fn default_set() -> Vec<SizingOption> {
        vec![Self::third(), Self::two_thirds(), Self::pot(), Self::jam()]
    }

    /// A deliberately small sizing set for *re*-raises (3-bet and beyond).
    /// Applying the full 4-option `default_set` at every raise level makes
    /// the bet tree's node count grow as (sizings)^(raise depth) -- with 4
    /// sizings and 3 raise levels that's already hundreds of distinct
    /// showdown nodes, each needing its own full hero-x-villain equity
    /// matrix. Real strategy simplification points the same way in
    /// practice anyway: 3-bet/4-bet sizing menus are usually much simpler
    /// than opening-bet menus (often just "pot" and "jam").
    pub fn reraise_set() -> Vec<SizingOption> {
        vec![Self::pot(), Self::jam()]
    }

    /// A wider tree for spots that benefit from more granularity (e.g. flop
    /// c-bet sizing study).
    pub fn wide_set() -> Vec<SizingOption> {
        vec![
            Self::quarter(),
            Self::third(),
            Self::half(),
            Self::two_thirds(),
            Self::three_quarters(),
            Self::pot(),
            Self::overbet(),
            Self::jam(),
        ]
    }
}

/// Preflop raise sizing, expressed as a multiple of the previous bet (so an
/// "open" multiple is in BBs, a "3-bet" multiple is x the open, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RaiseMultiple {
    pub label: &'static str,
    pub multiple: f32,
}

impl RaiseMultiple {
    pub fn standard_opens() -> Vec<RaiseMultiple> {
        vec![
            RaiseMultiple { label: "2.0x", multiple: 2.0 },
            RaiseMultiple { label: "2.3x", multiple: 2.3 },
            RaiseMultiple { label: "2.5x", multiple: 2.5 },
            RaiseMultiple { label: "3.0x", multiple: 3.0 },
        ]
    }

    pub fn standard_3bets() -> Vec<RaiseMultiple> {
        vec![
            RaiseMultiple { label: "3x", multiple: 3.0 },
            RaiseMultiple { label: "3.5x", multiple: 3.5 },
            RaiseMultiple { label: "4x", multiple: 4.0 },
        ]
    }

    pub fn standard_4bets() -> Vec<RaiseMultiple> {
        vec![
            RaiseMultiple { label: "2.2x", multiple: 2.2 },
            RaiseMultiple { label: "2.5x", multiple: 2.5 },
            RaiseMultiple { label: "Jam", multiple: f32::INFINITY },
        ]
    }
}

/// Classify a postflop bet into a `BetCategory` given context about the
/// hand so far. This is used by both the live hand-history stats engine and
/// the drill UI's badges.
pub fn classify_bet(
    pot_fraction: f32,
    is_first_bet_this_street: bool,
    bettor_was_last_aggressor: bool,
    bettor_checked_earlier_this_hand: bool,
    is_all_in: bool,
) -> BetCategory {
    if is_all_in {
        return BetCategory::AllIn;
    }
    if is_first_bet_this_street {
        if bettor_was_last_aggressor {
            return BetCategory::ContinuationBet;
        }
        if bettor_checked_earlier_this_hand {
            return BetCategory::Probe;
        }
        return BetCategory::Donk;
    }
    if pot_fraction >= 1.0 + f32::EPSILON {
        return BetCategory::Overbet;
    }
    if (pot_fraction - 1.0).abs() < 0.05 {
        return BetCategory::PotSized;
    }
    BetCategory::Raise
}
