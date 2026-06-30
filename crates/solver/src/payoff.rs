use std::collections::HashMap;

use rayon::prelude::*;

use rib_core::{Card, OmahaHole};
use rib_evaluator::{equity_heads_up, omaha_equity_heads_up, EquityResult};

use crate::hand_index::{conflicts, HandIndex};
use crate::tree::{NodeId, NodeKind, TerminalKind, Tree};

/// How many runout samples to use when a Terminal's board is incomplete
/// (the street-extension limit was reached before the river). Kept modest
/// so live solves stay interactive; precomputed library entries can afford
/// to run this much higher (see `library::seed`).
pub const DEFAULT_RUNOUT_SAMPLES: u32 = 60;

/// node_id -> payoff[i][j] = hero's net bb result, hero hand `i` vs villain
/// hand `j`, for every Terminal::Showdown node in the tree. Entries where
/// the two hands share a card are left at 0.0 (impossible matchup, and
/// CFR's reach-weighting will assign them ~0 weight in practice anyway).
pub type PayoffTable = HashMap<NodeId, Vec<Vec<f32>>>;

/// Dispatches to the NLHE (2-card) or Omaha (4-card) equity calculator
/// based on how many cards each hand actually has. `HandIndex.cards` is a
/// plain `Vec<Card>` so the same solver/CFR code works for both games;
/// this is the one place that needs to know which concrete evaluator a
/// given pair of hands wants.
fn equity(hero: &[Card], villain: &[Card], board: &[Card], samples: u32) -> EquityResult {
    match (hero.len(), villain.len()) {
        (2, 2) => {
            let h = [hero[0], hero[1]];
            let v = [villain[0], villain[1]];
            equity_heads_up(h, v, board, samples)
        }
        (4, 4) => {
            let h = OmahaHole([hero[0], hero[1], hero[2], hero[3]]);
            let v = OmahaHole([villain[0], villain[1], villain[2], villain[3]]);
            omaha_equity_heads_up(h, v, board, samples)
        }
        (hn, vn) => unreachable!("mismatched or unsupported hand sizes in payoff computation: hero={hn} villain={vn}"),
    }
}

pub fn precompute_payoffs(
    tree: &Tree,
    hero_hands: &[HandIndex],
    villain_hands: &[HandIndex],
    runout_samples: u32,
) -> PayoffTable {
    let showdown_nodes: Vec<(NodeId, &crate::tree::TreeNode)> = tree
        .arena
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n.kind, NodeKind::Terminal(TerminalKind::Showdown { .. })))
        .collect();

    showdown_nodes
        .into_par_iter()
        .map(|(id, node)| {
            let board = match &node.kind {
                NodeKind::Terminal(TerminalKind::Showdown { board }) => board,
                _ => unreachable!(),
            };
            let pot = node.pot;
            let mut matrix = vec![vec![0f32; villain_hands.len()]; hero_hands.len()];
            for (i, h) in hero_hands.iter().enumerate() {
                for (j, v) in villain_hands.iter().enumerate() {
                    if conflicts(&h.cards, &v.cards) {
                        continue;
                    }
                    let eq = equity(&h.cards, &v.cards, &board.cards, runout_samples);
                    // win -> hero nets villain's contribution; lose -> hero
                    // loses their own contribution; tie -> nets 0 (pot split
                    // evenly returns each player's own stake).
                    matrix[i][j] = eq.win * pot.villain_invested - eq.lose * pot.hero_invested;
                }
            }
            (id, matrix)
        })
        .collect()
}
