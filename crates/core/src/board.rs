use serde::{Deserialize, Serialize};

use crate::card::{Card, Suit};
use crate::street::Street;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Board {
    pub cards: Vec<Card>,
}

impl Board {
    pub fn street(&self) -> Street {
        match self.cards.len() {
            0 => Street::Preflop,
            3 => Street::Flop,
            4 => Street::Turn,
            5 => Street::River,
            n => panic!("invalid board size: {n}"),
        }
    }

    pub fn is_paired(&self) -> bool {
        let mut ranks: Vec<_> = self.cards.iter().map(|c| c.rank).collect();
        ranks.sort();
        ranks.windows(2).any(|w| w[0] == w[1])
    }

    pub fn is_monotone(&self) -> bool {
        if self.cards.len() < 3 {
            return false;
        }
        let s0 = self.cards[0].suit;
        self.cards.iter().all(|c| c.suit == s0)
    }

    pub fn flush_draw_possible(&self) -> bool {
        let mut counts = [0u8; 4];
        for c in &self.cards {
            counts[suit_idx(c.suit)] += 1;
        }
        counts.iter().any(|&n| n >= 2)
    }

    /// Highest rank on board, used for high-card texture classification.
    pub fn high_card(&self) -> Option<crate::card::Rank> {
        self.cards.iter().map(|c| c.rank).max()
    }

    /// Coarse texture bucket used by the solver's postflop abstraction (a
    /// real GTO solver clusters boards into hundreds of strategically
    /// similar buckets via equity histograms; this is a much smaller, fast,
    /// hand-written approximation that's good enough to keep the abstracted
    /// tree tractable while still being strategically meaningful).
    pub fn texture(&self) -> BoardTexture {
        BoardTexture {
            paired: self.is_paired(),
            monotone: self.is_monotone(),
            flush_draw: self.flush_draw_possible(),
            high_card: self.high_card(),
            connected: self.is_connected(),
        }
    }

    fn is_connected(&self) -> bool {
        if self.cards.len() < 3 {
            return false;
        }
        let mut ranks: Vec<i32> = self.cards.iter().map(|c| c.rank as i32).collect();
        ranks.sort();
        ranks.dedup();
        ranks.windows(2).filter(|w| w[1] - w[0] <= 2).count() >= 1
    }
}

fn suit_idx(s: Suit) -> usize {
    match s {
        Suit::Clubs => 0,
        Suit::Diamonds => 1,
        Suit::Hearts => 2,
        Suit::Spades => 3,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardTexture {
    pub paired: bool,
    pub monotone: bool,
    pub flush_draw: bool,
    pub high_card: Option<crate::card::Rank>,
    pub connected: bool,
}
