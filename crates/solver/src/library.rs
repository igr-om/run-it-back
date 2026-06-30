//! Cache-key shape and the curated list of spots worth pre-solving so the
//! preflop trainer/range-explorer feels instant. Persistence is the `rib-db`
//! crate's job (a `solved_spots` table keyed by `SpotKey::cache_key()`); this
//! module only knows what a key looks like and which keys matter.

use serde::{Deserialize, Serialize};

use rib_core::{GameType, PotType, Position};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpotKey {
    pub game: GameType,
    pub pot_type: PotType,
    pub stack_bb: u32,
    pub hero_position: Position,
    pub villain_position: Position,
    /// Empty = preflop. 3/4/5 cards = flop/turn/river postflop spot.
    pub board: Vec<String>,
}

impl SpotKey {
    pub fn cache_key(&self) -> String {
        format!(
            "{}|{:?}|{}|{:?}|{:?}|{}",
            self.game, self.pot_type, self.stack_bb, self.hero_position, self.villain_position, self.board.join("")
        )
    }

    pub fn is_preflop(&self) -> bool {
        self.board.is_empty()
    }
}

/// A reasonably comprehensive (but not exhaustive -- exhaustive is what
/// `library_warm` background jobs are for, see `rib-worker`) set of preflop
/// spots: every position pair, every common pot type, the most common stack
/// depths. This intentionally mirrors the axes GTOWizard's own preflop
/// trainer lets you filter by.
pub fn curated_preflop_seed_list() -> Vec<SpotKey> {
    let positions = Position::for_table_size(6);
    let mut out = Vec::new();
    for &stack in [20u32, 40, 100].iter() {
        for &hero in &positions {
            for &villain in &positions {
                if hero == villain {
                    continue;
                }
                out.push(SpotKey {
                    game: GameType::Nlhe,
                    pot_type: PotType::Srp,
                    stack_bb: stack,
                    hero_position: hero,
                    villain_position: villain,
                    board: vec![],
                });
            }
        }
        // 3-bet and 4-bet pots are most commonly studied for BTN/CO/SB/BB.
        for &hero in &[rib_core::Position::Btn, rib_core::Position::Co, rib_core::Position::Sb, rib_core::Position::Bb] {
            for &villain in &positions {
                if hero == villain {
                    continue;
                }
                out.push(SpotKey {
                    game: GameType::Nlhe,
                    pot_type: PotType::ThreeBet,
                    stack_bb: stack,
                    hero_position: hero,
                    villain_position: villain,
                    board: vec![],
                });
                out.push(SpotKey {
                    game: GameType::Nlhe,
                    pot_type: PotType::FourBet,
                    stack_bb: stack,
                    hero_position: hero,
                    villain_position: villain,
                    board: vec![],
                });
            }
        }
    }
    out
}
