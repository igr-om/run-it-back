use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use rib_core::{parse_range_string, Action, Board, GameType, SizingOption, Strategy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Player {
    Hero,
    Villain,
}

impl Player {
    pub fn other(&self) -> Player {
        match self {
            Player::Hero => Player::Villain,
            Player::Villain => Player::Hero,
        }
    }
}

/// What hero is doing right now: opening the action (check/bet) or
/// responding to a villain bet of a given size (fold/call/raise).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeContext {
    /// Hero acts first this street (or both players have checked so far).
    FirstToAct,
    /// Hero faces a bet of `size_bb` (total amount put in this street).
    FacingBet { size_bb: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveRequest {
    pub game: GameType,
    /// Board cards known at the start of the node being solved. Empty board
    /// = preflop solve.
    pub board: Vec<String>,
    pub effective_stack_bb: f32,
    /// Total pot size (bb) already in the middle before this node's action.
    pub starting_pot_bb: f32,
    /// How much hero has already put in *this hand* (folded equity / future
    /// streets math needs this even though it isn't part of `starting_pot_bb`).
    pub hero_invested_bb: f32,
    pub villain_invested_bb: f32,
    /// Range strings, e.g. "22+,A2s+,KQo,AKo". Hero is the player to act
    /// first in this request (either genuinely first-to-act, or facing a bet).
    pub hero_range: String,
    pub villain_range: String,
    pub hero_is_in_position: bool,
    pub context: NodeContext,
    /// Bet sizings (as fraction of pot) available to whichever player is
    /// opening the betting. Defaults to `SizingOption::default_set()`.
    #[serde(default)]
    pub sizings: Option<Vec<f32>>,
    /// How many additional community cards to deal and keep solving for,
    /// beyond the current street (0 = resolve remaining streets via runout
    /// equity only, no further betting modeled; 1 = model betting on the
    /// next street too). Capped at 1 by the server regardless of what's
    /// requested, to keep live solves real-time.
    #[serde(default)]
    pub streets_to_extend: u8,
    #[serde(default = "default_iterations")]
    pub iterations: u32,
}

fn default_iterations() -> u32 {
    500
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolveResponse {
    pub hero_strategy: Strategy,
    /// combo label -> EV (bb) per action, aligned with `hero_strategy.actions`.
    /// This is what lets the drill grader say "checking was -0.6bb vs the
    /// solver's mix" instead of only "you were wrong" -- `Strategy::ev_bb`
    /// alone only carries the EV of the *blended* solved mix, not of each
    /// individual action.
    pub action_ev_bb: HashMap<String, Vec<f32>>,
    pub hero_ev_bb: f32,
    pub villain_ev_bb: f32,
    pub iterations_run: u32,
    pub exploitability_estimate: f32,
    pub n_hero_combos: usize,
    pub n_villain_combos: usize,
    pub warnings: Vec<String>,
}

pub fn parse_weighted_range(s: &str) -> Result<Vec<(rib_core::Combo, f32)>, rib_core::RibError> {
    // Supports an optional ":weight" suffix per token for mixed-frequency
    // ranges, e.g. "22+,A5s:0.5,KQo:0.3". Falls back to weight 1.0.
    let mut out: HashMap<rib_core::Combo, f32> = HashMap::new();
    for raw in s.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (token, weight) = match raw.split_once(':') {
            Some((t, w)) => (t, w.trim().parse::<f32>().unwrap_or(1.0)),
            None => (raw, 1.0),
        };
        for combo in parse_range_string(token)? {
            out.insert(combo, weight);
        }
    }
    Ok(out.into_iter().collect())
}

pub fn resolve_sizings(custom: &Option<Vec<f32>>) -> Vec<SizingOption> {
    match custom {
        None => SizingOption::default_set(),
        Some(fractions) => fractions
            .iter()
            .map(|f| SizingOption::new(format!("{:.0}%", f * 100.0), *f))
            .collect(),
    }
}

pub fn parse_board(strs: &[String]) -> Result<Board, rib_core::RibError> {
    use std::str::FromStr;
    let cards = strs
        .iter()
        .map(|s| rib_core::Card::from_str(s))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Board { cards })
}

/// Convenience for non-mix actions used as solver tree branches.
pub fn fold_check_call() -> Vec<Action> {
    vec![Action::Fold, Action::Call]
}
