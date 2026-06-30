use serde::{Deserialize, Serialize};
use std::fmt;

/// Game variant. PLO is modeled here for forward-compatibility (the schema,
/// API and UI all branch on this cleanly) but the solver/evaluator only
/// implement `Nlhe` in v1 — see README "Roadmap". Requesting Plo from the
/// solver returns `RibError::Unsupported`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameType {
    Nlhe,
    Plo,
}

impl fmt::Display for GameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameType::Nlhe => write!(f, "NLHE"),
            GameType::Plo => write!(f, "PLO"),
        }
    }
}

/// How many big blinds effective stack at the start of the hand. Solved-spot
/// library entries are keyed on a small set of common depths; live solves
/// accept any value and round to the nearest bucket for caching purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StackDepth(pub u32);

impl StackDepth {
    pub const COMMON: [u32; 6] = [20, 40, 60, 100, 150, 200];

    pub fn nearest_common(&self) -> u32 {
        *Self::COMMON
            .iter()
            .min_by_key(|d| (**d as i64 - self.0 as i64).abs())
            .unwrap()
    }
}

/// Preflop "pot type" — how many raises have happened before the spot in
/// question. This is the primary axis GTOWizard-style trainers filter by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PotType {
    /// Single raised pot (one open, everyone else folds or calls).
    Srp,
    /// 3-bet pot.
    ThreeBet,
    /// 4-bet pot.
    FourBet,
    /// 5-bet+ (typically stacks are short / jammed).
    FiveBetPlus,
    /// Nobody raised preflop (everyone limped or checked around in BB).
    LimpedPot,
}

impl fmt::Display for PotType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PotType::Srp => "Single Raised Pot",
            PotType::ThreeBet => "3-Bet Pot",
            PotType::FourBet => "4-Bet Pot",
            PotType::FiveBetPlus => "5-Bet+ Pot",
            PotType::LimpedPot => "Limped Pot",
        };
        write!(f, "{s}")
    }
}

/// Semantic categorization of a postflop bet, independent of its exact
/// sizing. This is what the UI badges drills/spots with ("c-bet", "3-bet",
/// "probe", "overbet" etc.) and what the stats engine aggregates over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetCategory {
    /// First bet on a street, made by the player who was last aggressor.
    ContinuationBet,
    /// First bet on a street, made by someone other than last aggressor.
    Donk,
    /// Bet into the aggressor after checking earlier in the hand.
    Probe,
    /// Preflop open raise.
    Open,
    /// Preflop 3-bet.
    ThreeBet,
    /// Preflop 4-bet.
    FourBet,
    /// Preflop 5-bet or more.
    FiveBetPlus,
    /// Raise of a postflop bet.
    Raise,
    /// Bet/raise sized at ~100% pot, called out separately because it's a
    /// meaningfully distinct strategic class in solver output.
    PotSized,
    /// Bet/raise sized > 100% pot.
    Overbet,
    /// All remaining chips.
    AllIn,
}

impl fmt::Display for BetCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BetCategory::ContinuationBet => "C-Bet",
            BetCategory::Donk => "Donk Bet",
            BetCategory::Probe => "Probe Bet",
            BetCategory::Open => "Open",
            BetCategory::ThreeBet => "3-Bet",
            BetCategory::FourBet => "4-Bet",
            BetCategory::FiveBetPlus => "5-Bet+",
            BetCategory::Raise => "Raise",
            BetCategory::PotSized => "Pot-Sized Bet",
            BetCategory::Overbet => "Overbet",
            BetCategory::AllIn => "All-In",
        };
        write!(f, "{s}")
    }
}
