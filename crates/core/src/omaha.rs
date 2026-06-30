//! Pot-Limit Omaha hole-card representation and range parsing.
//!
//! NLHE's 169-class grid works because 2-card starting hands collapse
//! cleanly into "rank pair + suited/offsuit". Omaha's 4-card hands don't:
//! there are C(52,4) = 270,725 distinct starting combos, and the natural
//! abstraction axes (which ranks, how many pair up, how many are suited
//! together) don't fit a 13x13 grid the way NLHE does. So instead of a
//! grid, ranges here are described by a short rank-pattern + an optional
//! suit-pattern filter (mirroring how PLO tools like Flopzilla/GTO+ let you
//! filter ranges -- "double suited", "rainbow", etc.) and expanded directly
//! to specific weighted combos.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::card::{Card, Rank, Suit};
use crate::error::RibError;

/// Four specific hole cards, canonically sorted (rank desc, then suit) so
/// two holdings built from the same cards in a different order compare
/// equal and hash identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct OmahaHole(pub [Card; 4]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuitPattern {
    /// All 4 cards the same suit.
    Monotone,
    /// 3 cards one suit, 1 card a different suit.
    TripsSuited,
    /// Two pairs of matching suits (e.g. 2 spades + 2 hearts).
    DoubleSuited,
    /// Exactly one pair of matching suits, the other two cards different.
    SingleSuited,
    /// All 4 cards different suits.
    Rainbow,
}

impl fmt::Display for SuitPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SuitPattern::Monotone => "monotone",
            SuitPattern::TripsSuited => "trips-suited",
            SuitPattern::DoubleSuited => "double-suited",
            SuitPattern::SingleSuited => "single-suited",
            SuitPattern::Rainbow => "rainbow",
        };
        write!(f, "{s}")
    }
}

impl OmahaHole {
    pub fn new(mut cards: [Card; 4]) -> Result<Self, RibError> {
        for i in 0..4 {
            for j in (i + 1)..4 {
                if cards[i] == cards[j] {
                    return Err(RibError::Range(format!("duplicate card {} in an Omaha hand", cards[i])));
                }
            }
        }
        cards.sort_by(|a, b| b.rank.cmp(&a.rank).then(a.suit.cmp(&b.suit)));
        Ok(Self(cards))
    }

    /// All C(4,2) = 6 ways to choose 2 of the 4 hole cards. Omaha's best
    /// 5-card hand must use *exactly* 2 hole cards (never more, never
    /// fewer) -- evaluating a hand means trying all 6 of these against all
    /// 3-card board combinations and taking the best resulting 5-card hand.
    pub fn two_card_combos(&self) -> [(Card, Card); 6] {
        [
            (self.0[0], self.0[1]),
            (self.0[0], self.0[2]),
            (self.0[0], self.0[3]),
            (self.0[1], self.0[2]),
            (self.0[1], self.0[3]),
            (self.0[2], self.0[3]),
        ]
    }

    pub fn contains(&self, card: Card) -> bool {
        self.0.contains(&card)
    }

    pub fn cards(&self) -> [Card; 4] {
        self.0
    }

    pub fn suit_pattern(&self) -> SuitPattern {
        let mut counts = [0u8; 4];
        for c in &self.0 {
            counts[suit_idx(c.suit)] += 1;
        }
        let mut sorted = counts;
        sorted.sort_unstable_by(|a, b| b.cmp(a));
        match sorted {
            [4, 0, 0, 0] => SuitPattern::Monotone,
            [3, 1, 0, 0] => SuitPattern::TripsSuited,
            [2, 2, 0, 0] => SuitPattern::DoubleSuited,
            [2, 1, 1, 0] => SuitPattern::SingleSuited,
            _ => SuitPattern::Rainbow,
        }
    }

    /// 4-character rank-only shorthand ("AAKK", "AKQJ"), ranks sorted high
    /// to low -- the class label this hand's range entry would print as.
    pub fn rank_shorthand(&self) -> String {
        self.0.iter().map(|c| c.rank.char()).collect()
    }
}

fn suit_idx(s: Suit) -> usize {
    Suit::ALL.iter().position(|&x| x == s).unwrap_or(0)
}

impl fmt::Display for OmahaHole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in &self.0 {
            write!(f, "{c}")?;
        }
        Ok(())
    }
}

impl FromStr for OmahaHole {
    type Err = RibError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cards = parse_omaha_cards(s)?;
        OmahaHole::new(cards)
    }
}

impl From<OmahaHole> for String {
    fn from(h: OmahaHole) -> String {
        h.to_string()
    }
}

impl TryFrom<String> for OmahaHole {
    type Error = RibError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        OmahaHole::from_str(&s)
    }
}

/// Parses either space/comma separated ("Ah Kh Qd Jc") or bare concatenated
/// ("AhKhQdJc") 4-card notation.
fn parse_omaha_cards(s: &str) -> Result<[Card; 4], RibError> {
    let s = s.trim();
    let cards: Vec<Card> = if s.contains(' ') || s.contains(',') {
        crate::card::parse_cards(s)?
    } else {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 8 {
            return Err(RibError::Parse(format!(
                "expected 4 cards (8 characters) in concatenated Omaha notation, got '{s}'"
            )));
        }
        chars
            .chunks(2)
            .map(|c| c.iter().collect::<String>().parse::<Card>())
            .collect::<Result<Vec<_>, _>>()?
    };
    if cards.len() != 4 {
        return Err(RibError::Parse(format!("expected exactly 4 cards, got {} in '{s}'", cards.len())));
    }
    Ok([cards[0], cards[1], cards[2], cards[3]])
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OmahaRangeEntry {
    pub hole: OmahaHole,
    pub weight: f32,
}

/// Parses a comma-separated Omaha range string. Each entry is 4 rank
/// characters (repeats allowed: "AAKK", "AAAK", "AAAA"), optionally
/// followed by a suit-pattern filter (`ds`, `ss`, `mono`, `rainbow`/`r`)
/// and/or a `:weight` (defaults to 1.0), e.g.:
///
/// `"AAKKds:0.75, AKQJ, T987ss"`
///
/// Every entry expands to every specific 4-card combo matching that rank
/// pattern (and suit filter, if given) -- e.g. "AAKK" alone expands to all
/// C(4,2)*C(4,2) = 36 specific combos of two aces and two kings.
pub fn parse_omaha_range(s: &str) -> Result<Vec<OmahaRangeEntry>, RibError> {
    let mut by_hole: HashMap<OmahaHole, f32> = HashMap::new();
    for raw in s.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (body, weight) = match raw.split_once(':') {
            Some((b, w)) => (b, w.trim().parse::<f32>().map_err(|_| RibError::Range(format!("bad weight in '{raw}'")))?),
            None => (raw, 1.0),
        };
        if body.len() < 4 {
            return Err(RibError::Range(format!("'{raw}' needs at least 4 rank characters")));
        }
        let (rank_part, suit_part) = body.split_at(4);
        let ranks: Vec<Rank> = rank_part
            .chars()
            .map(|c| Rank::from_str(&c.to_string()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| RibError::Range(format!("bad rank in '{raw}'")))?;
        let filter = parse_suit_filter(suit_part.trim())?;

        for hole in expand_rank_pattern(&ranks) {
            if let Some(want) = filter {
                if hole.suit_pattern() != want {
                    continue;
                }
            }
            by_hole.insert(hole, weight);
        }
    }
    Ok(by_hole.into_iter().map(|(hole, weight)| OmahaRangeEntry { hole, weight }).collect())
}

fn parse_suit_filter(s: &str) -> Result<Option<SuitPattern>, RibError> {
    match s.to_ascii_lowercase().as_str() {
        "" => Ok(None),
        "ds" => Ok(Some(SuitPattern::DoubleSuited)),
        "ss" | "s" => Ok(Some(SuitPattern::SingleSuited)),
        "ts" => Ok(Some(SuitPattern::TripsSuited)),
        "mono" | "monotone" => Ok(Some(SuitPattern::Monotone)),
        "r" | "rainbow" => Ok(Some(SuitPattern::Rainbow)),
        other => Err(RibError::Range(format!("unrecognized suit filter '{other}' (expected ds, ss, ts, mono, or rainbow)"))),
    }
}

/// Every specific 4-card combo matching a 4-rank pattern (repeats allowed),
/// with no suit filtering yet -- that's applied by the caller afterward.
fn expand_rank_pattern(ranks: &[Rank]) -> Vec<OmahaHole> {
    let mut counts: HashMap<Rank, usize> = HashMap::new();
    for &r in ranks {
        *counts.entry(r).or_insert(0) += 1;
    }
    // Cartesian product, across each distinct rank, of "which k of the 4
    // suits does this rank use".
    let mut per_rank_choices: Vec<Vec<Vec<Suit>>> = Vec::new();
    let mut rank_for_choice: Vec<Rank> = Vec::new();
    for (&rank, &count) in counts.iter() {
        per_rank_choices.push(suit_combinations(count));
        rank_for_choice.push(rank);
    }

    let mut combos: Vec<Vec<Card>> = vec![vec![]];
    for (i, choices) in per_rank_choices.iter().enumerate() {
        let rank = rank_for_choice[i];
        let mut next = Vec::new();
        for partial in &combos {
            for suits in choices {
                let mut extended = partial.clone();
                for &s in suits {
                    extended.push(Card::new(rank, s));
                }
                next.push(extended);
            }
        }
        combos = next;
    }

    combos
        .into_iter()
        .filter_map(|cards| {
            if cards.len() != 4 {
                return None;
            }
            OmahaHole::new([cards[0], cards[1], cards[2], cards[3]]).ok()
        })
        .collect()
}

/// Every way to choose `k` of the 4 suits (order doesn't matter), via a
/// 16-subset bitmask scan -- simplest correct approach given there are
/// only 4 suits to ever consider.
fn suit_combinations(k: usize) -> Vec<Vec<Suit>> {
    if k == 0 || k > 4 {
        return vec![];
    }
    (0u8..16)
        .filter(|mask| mask.count_ones() as usize == k)
        .map(|mask| Suit::ALL.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, &s)| s).collect())
        .collect()
}

/// Total weight (post-expansion) for a parsed range -- useful for sanity
/// checks ("did my filter actually match anything?") in the API layer.
pub fn total_combos(entries: &[OmahaRangeEntry]) -> usize {
    entries.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_concatenated_and_spaced() {
        let a: OmahaHole = "AhKhQdJc".parse().unwrap();
        let b: OmahaHole = "Ah Kh Qd Jc".parse().unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_duplicate_card() {
        assert!("AhAhQdJc".parse::<OmahaHole>().is_err());
    }

    #[test]
    fn pair_pair_expands_to_36() {
        let entries = parse_omaha_range("AAKK").unwrap();
        assert_eq!(entries.len(), 36);
    }

    #[test]
    fn quads_expands_to_1() {
        let entries = parse_omaha_range("AAAA").unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn four_distinct_ranks_expands_to_256() {
        let entries = parse_omaha_range("AKQJ").unwrap();
        assert_eq!(entries.len(), 256);
    }

    #[test]
    fn double_suited_filter_narrows_it() {
        let all = parse_omaha_range("AKQJ").unwrap();
        let ds = parse_omaha_range("AKQJds").unwrap();
        assert!(ds.len() < all.len());
        for e in &ds {
            assert_eq!(e.hole.suit_pattern(), SuitPattern::DoubleSuited);
        }
    }
}
