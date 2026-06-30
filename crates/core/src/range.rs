//! Preflop "combo" grid (the 13x13 / 169-cell grid every GTO trainer shows)
//! plus parsing of standard shorthand range notation like `22+,A5s+,KQo`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::action::Action;
use crate::card::{Card, Rank, Suit};
use crate::error::RibError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suitedness {
    Paired,
    Suited,
    Offsuit,
}

/// One of the 169 distinct starting-hand classes (suit-isomorphic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Combo {
    /// Higher (or equal, for pairs) rank.
    pub hi: Rank,
    /// Lower rank.
    pub lo: Rank,
    pub kind: Suitedness,
}

impl Combo {
    pub fn pair(r: Rank) -> Self {
        Combo { hi: r, lo: r, kind: Suitedness::Paired }
    }

    pub fn new(a: Rank, b: Rank, suited: bool) -> Self {
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        if hi == lo {
            return Combo::pair(hi);
        }
        Combo { hi, lo, kind: if suited { Suitedness::Suited } else { Suitedness::Offsuit } }
    }

    /// All 169 combos, ordered hi-to-lo then pairs > suited > offsuit, the
    /// order GTOWizard-style grids display them in (used to lay out the UI
    /// grid deterministically).
    pub fn all_169() -> Vec<Combo> {
        let mut out = Vec::with_capacity(169);
        let ranks = Rank::ALL;
        for i in (0..13).rev() {
            for j in (0..13).rev() {
                let hi = ranks[i];
                let lo = ranks[j];
                if i == j {
                    out.push(Combo::pair(hi));
                } else if i > j {
                    out.push(Combo::new(hi, lo, true)); // suited, upper triangle
                } else {
                    out.push(Combo::new(lo, hi, false)); // offsuit, lower triangle (hi/lo normalized inside `new`)
                }
            }
        }
        out
    }

    /// How many of the 1326 specific 2-card combinations this class
    /// represents (used to weight EV/frequency aggregation correctly).
    pub fn n_specific_combos(&self) -> u32 {
        match self.kind {
            Suitedness::Paired => 6,
            Suitedness::Suited => 4,
            Suitedness::Offsuit => 12,
        }
    }

    pub fn label(&self) -> String {
        match self.kind {
            Suitedness::Paired => format!("{}{}", self.hi.char(), self.lo.char()),
            Suitedness::Suited => format!("{}{}s", self.hi.char(), self.lo.char()),
            Suitedness::Offsuit => format!("{}{}o", self.hi.char(), self.lo.char()),
        }
    }

    pub fn from_hole_cards(cards: [Card; 2]) -> Combo {
        let suited = cards[0].suit == cards[1].suit;
        Combo::new(cards[0].rank, cards[1].rank, suited)
    }

    /// Every specific 2-card combination this class represents, e.g. "AKs"
    /// -> 4 combos (one per suit), "AKo" -> 12, "AA" -> 6. Used by the drill
    /// generator to deal an actual hand (real suits) once it's picked which
    /// 169-class to drill, and by anything else that needs to go from "the
    /// solver's class-level strategy" back to "a concrete hand a player
    /// could be holding".
    pub fn expand_specific(&self) -> Vec<(Card, Card)> {
        const SUITS: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
        let mut out = Vec::new();
        match self.kind {
            Suitedness::Paired => {
                for i in 0..4 {
                    for j in (i + 1)..4 {
                        out.push((Card::new(self.hi, SUITS[i]), Card::new(self.hi, SUITS[j])));
                    }
                }
            }
            Suitedness::Suited => {
                for s in SUITS {
                    out.push((Card::new(self.hi, s), Card::new(self.lo, s)));
                }
            }
            Suitedness::Offsuit => {
                for s1 in SUITS {
                    for s2 in SUITS {
                        if s1 != s2 {
                            out.push((Card::new(self.hi, s1), Card::new(self.lo, s2)));
                        }
                    }
                }
            }
        }
        out
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Parse a single token like "AA", "AKs", "AKo", "QJ" (unsuited shorthand
/// meaning "both", expanded to two combos) optionally followed by `+` or as
/// part of a `-` interval, handled by `parse_range_string`.
fn parse_token_base(tok: &str) -> Result<Vec<Combo>, RibError> {
    let chars: Vec<char> = tok.chars().collect();
    if chars.len() < 2 {
        return Err(RibError::Range(format!("bad token: {tok}")));
    }
    let r1 = Rank::from_str_char(chars[0])?;
    let r2 = Rank::from_str_char(chars[1])?;
    if chars.len() == 2 {
        if r1 == r2 {
            return Ok(vec![Combo::pair(r1)]);
        }
        // bare "QJ" with no suffix => both suited and offsuit combos.
        return Ok(vec![Combo::new(r1, r2, true), Combo::new(r1, r2, false)]);
    }
    match chars[2].to_ascii_lowercase() {
        's' => Ok(vec![Combo::new(r1, r2, true)]),
        'o' => Ok(vec![Combo::new(r1, r2, false)]),
        _ => Err(RibError::Range(format!("bad token suffix: {tok}"))),
    }
}

impl Rank {
    fn from_str_char(c: char) -> Result<Rank, RibError> {
        use std::str::FromStr;
        Rank::from_str(&c.to_string())
    }
}

/// Expand a `+`-suffixed token, e.g. "55+" -> 55,66,..,AA; "A5s+" -> A5s..AQs;
/// "KTo+" -> KTo, KJo, KQo.
fn expand_plus(base: Combo) -> Vec<Combo> {
    let mut out = Vec::new();
    let ranks = Rank::ALL;
    let hi_idx = ranks.iter().position(|r| *r == base.hi).unwrap();
    let lo_idx = ranks.iter().position(|r| *r == base.lo).unwrap();
    match base.kind {
        Suitedness::Paired => {
            for i in hi_idx..13 {
                out.push(Combo::pair(ranks[i]));
            }
        }
        Suitedness::Suited | Suitedness::Offsuit => {
            // Keep `hi` fixed, walk `lo` up toward (but not including) `hi`.
            for i in lo_idx..hi_idx {
                out.push(Combo::new(base.hi, ranks[i], base.kind == Suitedness::Suited));
            }
        }
    }
    out
}

/// Expand a `-`-delimited interval, e.g. "ATs-A5s" -> ATs,A9s,A8s,A7s,A6s,A5s;
/// "TT-77" -> TT,99,88,77. Both ends must share the same "hi" rank (or both
/// be pairs); this matches standard range-string conventions.
fn expand_interval(from: Combo, to: Combo) -> Result<Vec<Combo>, RibError> {
    let ranks = Rank::ALL;
    match (from.kind, to.kind) {
        (Suitedness::Paired, Suitedness::Paired) => {
            let (lo_i, hi_i) = (
                ranks.iter().position(|r| *r == to.hi).unwrap(),
                ranks.iter().position(|r| *r == from.hi).unwrap(),
            );
            let (lo_i, hi_i) = (lo_i.min(hi_i), lo_i.max(hi_i));
            Ok((lo_i..=hi_i).map(|i| Combo::pair(ranks[i])).collect())
        }
        (a, b) if a == b && from.hi == to.hi => {
            let i1 = ranks.iter().position(|r| *r == from.lo).unwrap();
            let i2 = ranks.iter().position(|r| *r == to.lo).unwrap();
            let (lo_i, hi_i) = (i1.min(i2), i1.max(i2));
            Ok((lo_i..=hi_i)
                .map(|i| Combo::new(from.hi, ranks[i], a == Suitedness::Suited))
                .collect())
        }
        _ => Err(RibError::Range("interval endpoints must share a suffix/high card".into())),
    }
}

/// Parse a full range string such as `"22+,ATs+,KQo,A5s-A2s"` into the set of
/// 169-grid combos it refers to (duplicates collapsed).
pub fn parse_range_string(s: &str) -> Result<Vec<Combo>, RibError> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("100%") || trimmed.eq_ignore_ascii_case("all") || trimmed.eq_ignore_ascii_case("any2") {
        return Ok(Combo::all_169());
    }
    let mut seen = HashMap::new();
    for raw_tok in s.split(',') {
        let tok = raw_tok.trim();
        if tok.is_empty() {
            continue;
        }
        if let Some((from_s, to_s)) = tok.split_once('-') {
            let from = parse_token_base(from_s.trim())?;
            let to = parse_token_base(to_s.trim())?;
            for combo in expand_interval(from[0], to[0])? {
                seen.insert(combo, ());
            }
        } else if let Some(base_s) = tok.strip_suffix('+') {
            for base in parse_token_base(base_s.trim())? {
                for combo in expand_plus(base) {
                    seen.insert(combo, ());
                }
            }
        } else {
            for combo in parse_token_base(tok)? {
                seen.insert(combo, ());
            }
        }
    }
    Ok(seen.into_keys().collect())
}

/// A solved (or user) mixed strategy across the 169-combo grid: for each
/// combo, a probability distribution over a fixed action set. This is the
/// canonical shape returned by the solver and consumed by the range-explorer
/// UI and the drill grader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub actions: Vec<Action>,
    /// combo label ("AKs") -> frequencies aligned with `actions`, sums to 1.
    pub frequencies: HashMap<String, Vec<f32>>,
    /// combo label -> EV in big blinds for the *solved* mix at that combo
    /// (used to compute EV-loss when grading a drill answer).
    pub ev_bb: HashMap<String, f32>,
}

impl Strategy {
    pub fn empty(actions: Vec<Action>) -> Self {
        Self { actions, frequencies: HashMap::new(), ev_bb: HashMap::new() }
    }

    pub fn dominant_action(&self, combo_label: &str) -> Option<Action> {
        let freqs = self.frequencies.get(combo_label)?;
        let (idx, _) = freqs
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;
        self.actions.get(idx).copied()
    }
}
