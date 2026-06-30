//! Card primitives shared across the whole backend.
//!
//! Used everywhere -- API payloads, the parser, the solver's game tree,
//! the evaluator, the frontend's JSON -- since it's trivially
//! `Serialize`/`Deserialize` and has ergonomic string parsing built in.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::RibError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn char(&self) -> char {
        match self {
            Suit::Clubs => 'c',
            Suit::Diamonds => 'd',
            Suit::Hearts => 'h',
            Suit::Spades => 's',
        }
    }

    /// Unicode suit glyph, used by the UI for a card's accent color.
    pub fn glyph(&self) -> char {
        match self {
            Suit::Clubs => '♣',
            Suit::Diamonds => '♦',
            Suit::Hearts => '♥',
            Suit::Spades => '♠',
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.char())
    }
}

impl FromStr for Suit {
    type Err = RibError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next().map(|c| c.to_ascii_lowercase()) {
            Some('c') => Ok(Suit::Clubs),
            Some('d') => Ok(Suit::Diamonds),
            Some('h') => Ok(Suit::Hearts),
            Some('s') => Ok(Suit::Spades),
            _ => Err(RibError::Parse(format!("invalid suit: {s}"))),
        }
    }
}

/// Card rank, ordered Two..Ace so `Rank::Ace > Rank::King` etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Rank {
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
    Ten = 10,
    Jack = 11,
    Queen = 12,
    King = 13,
    Ace = 14,
}

impl Rank {
    pub const ALL: [Rank; 13] = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];

    pub fn char(&self) -> char {
        match self {
            Rank::Two => '2',
            Rank::Three => '3',
            Rank::Four => '4',
            Rank::Five => '5',
            Rank::Six => '6',
            Rank::Seven => '7',
            Rank::Eight => '8',
            Rank::Nine => '9',
            Rank::Ten => 'T',
            Rank::Jack => 'J',
            Rank::Queen => 'Q',
            Rank::King => 'K',
            Rank::Ace => 'A',
        }
    }

    /// 0-indexed (Two=0..Ace=12), handy for array/grid indexing.
    pub fn idx(&self) -> usize {
        (*self as u8 - 2) as usize
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.char())
    }
}

impl FromStr for Rank {
    type Err = RibError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().next().map(|c| c.to_ascii_uppercase()) {
            Some('2') => Ok(Rank::Two),
            Some('3') => Ok(Rank::Three),
            Some('4') => Ok(Rank::Four),
            Some('5') => Ok(Rank::Five),
            Some('6') => Ok(Rank::Six),
            Some('7') => Ok(Rank::Seven),
            Some('8') => Ok(Rank::Eight),
            Some('9') => Ok(Rank::Nine),
            Some('T') => Ok(Rank::Ten),
            Some('J') => Ok(Rank::Jack),
            Some('Q') => Ok(Rank::Queen),
            Some('K') => Ok(Rank::King),
            Some('A') => Ok(Rank::Ace),
            _ => Err(RibError::Parse(format!("invalid rank: {s}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }

    /// All 52 cards, in a stable order (rank-major).
    pub fn deck() -> Vec<Card> {
        let mut deck = Vec::with_capacity(52);
        for &rank in Rank::ALL.iter() {
            for &suit in Suit::ALL.iter() {
                deck.push(Card::new(rank, suit));
            }
        }
        deck
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank.char(), self.suit.char())
    }
}

impl FromStr for Card {
    type Err = RibError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.len() != 2 {
            return Err(RibError::Parse(format!("invalid card: {s}")));
        }
        let rank = Rank::from_str(&s[0..1])?;
        let suit = Suit::from_str(&s[1..2])?;
        Ok(Card::new(rank, suit))
    }
}

/// Parse a whitespace/comma separated card string, e.g. "Ah Kd 7s" or "Ah,Kd,7s".
pub fn parse_cards(s: &str) -> Result<Vec<Card>, RibError> {
    s.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|t| !t.is_empty())
        .map(Card::from_str)
        .collect()
}
