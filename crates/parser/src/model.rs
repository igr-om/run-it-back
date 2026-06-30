//! Canonical, site-independent hand representation. Every site parser's job
//! is to turn its own raw text format into a `Vec<ParsedHand>`; everything
//! downstream (DB storage, stats, drill generation from real play) only ever
//! sees this shape.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use rib_core::{Card, Position, Street};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    PostSmallBlind,
    PostBigBlind,
    PostAnte,
    Fold,
    Check,
    Call,
    Bet,
    Raise,
    AllIn,
    Show,
    Muck,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAction {
    pub seat: u8,
    pub player_name: String,
    pub is_hero: bool,
    pub street: Street,
    pub kind: ActionKind,
    /// Total chips this action puts in (not the delta), in big blinds. Folds
    /// and the like leave this `None`.
    pub amount_bb: Option<f64>,
    pub position: Option<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedHand {
    pub site: String,
    pub site_hand_id: Option<String>,
    pub game_type: String, // "nlhe" (PLO hands are detected and skipped -- see detect.rs)
    pub table_size: u8,
    pub big_blind_amount: f64, // in the site's currency/chip unit, used to normalize every other amount to bb
    pub hero_seat: Option<u8>,
    pub hero_position: Option<Position>,
    pub hero_cards: Vec<Card>,
    pub board: Vec<Card>,
    pub played_at: Option<DateTime<Utc>>,
    pub actions: Vec<ParsedAction>,
    /// Hero's net result this hand, in big blinds (positive = won).
    pub result_bb: f64,
    pub went_to_showdown: bool,
    pub won_hand: bool,
    /// Derived boolean tags used for stat aggregation -- see `stats.rs`.
    pub tags: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseHandHistoryError {
    #[error("could not detect which poker site this hand history is from")]
    UnknownSite,
    #[error("file appears to be a PLO hand history; PLO parsing isn't implemented in v1")]
    PloUnsupported,
    #[error("no hands could be parsed from this file (0 of {attempted} hand blocks parsed successfully)")]
    NoHandsParsed { attempted: usize },
    #[error("parser error: {0}")]
    Malformed(String),
}
