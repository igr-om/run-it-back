use rib_core::{Action, Board, Card, SizingOption};

use crate::pot::PotState;
use crate::request::Player;

pub type NodeId = usize;

#[derive(Debug)]
pub enum NodeKind {
    Decision {
        actor: Player,
        actions: Vec<Action>,
        children: Vec<NodeId>,
    },
    Chance {
        branches: Vec<(Card, NodeId)>,
    },
    Terminal(TerminalKind),
}

#[derive(Debug)]
pub enum TerminalKind {
    /// `folder` gave up; the other player wins whatever is in the pot.
    Fold { folder: Player },
    /// Hands are compared on `board`. If `board` has fewer than 5 cards
    /// (the street extension limit was reached before the river), payoff is
    /// filled in via runout equity instead of an exact showdown -- see
    /// `payoff::precompute_payoffs`.
    Showdown { board: Board },
}

#[derive(Debug)]
pub struct TreeNode {
    pub kind: NodeKind,
    pub pot: PotState,
}

#[derive(Clone)]
pub struct TreeConfig {
    pub sizings: Vec<SizingOption>,
    pub reraise_sizings: Vec<SizingOption>,
    pub max_raises_per_street: u8,
    pub streets_to_extend: u8,
    pub hero_in_position: bool,
}

pub struct Tree {
    pub arena: Vec<TreeNode>,
    pub root: NodeId,
}

struct Builder<'a> {
    arena: Vec<TreeNode>,
    cfg: &'a TreeConfig,
}

impl<'a> Builder<'a> {
    fn push(&mut self, kind: NodeKind, pot: PotState) -> NodeId {
        self.arena.push(TreeNode { kind, pot });
        self.arena.len() - 1
    }

    /// `facing`: total amount the acting player must call up to, if any
    /// (None = first to act / nothing to face yet this street).
    fn build_decision(
        &mut self,
        actor: Player,
        facing: Option<f32>,
        pot: PotState,
        board: &Board,
        raises_so_far: u8,
        streets_left: u8,
    ) -> NodeId {
        let stack_left = match actor {
            Player::Hero => pot.hero_stack_left(),
            Player::Villain => pot.villain_stack_left(),
        };

        match facing {
            None => {
                let mut actions = vec![Action::Check];
                let mut children = vec![self.after_check(actor, pot, board, streets_left)];

                if stack_left > 0.01 {
                    for sizing in self.cfg.sizings.clone() {
                        let size = bet_size(pot.pot(), sizing.pot_fraction, stack_left);
                        if size <= 0.01 {
                            continue;
                        }
                        actions.push(if size >= stack_left - 0.01 { Action::AllIn(size) } else { Action::Bet(size) });
                        let new_pot = set_invested(pot, actor, size);
                        children.push(self.build_decision(actor.other(), Some(size), new_pot, board, 0, streets_left));
                    }
                }
                self.push(NodeKind::Decision { actor, actions, children }, pot)
            }
            Some(facing_size) => {
                let mut actions = vec![Action::Fold, Action::Call];
                let mut children = vec![
                    self.push(NodeKind::Terminal(TerminalKind::Fold { folder: actor }), pot),
                    {
                        let new_pot = set_invested(pot, actor, facing_size);
                        self.after_call(new_pot, board, streets_left)
                    },
                ];

                let can_raise = raises_so_far < self.cfg.max_raises_per_street && stack_left > facing_size + 0.01;
                if can_raise {
                    for sizing in self.cfg.reraise_sizings.clone() {
                        let raise_to = raise_size(pot.pot(), facing_size, sizing.pot_fraction, stack_left);
                        if raise_to <= facing_size + 0.01 {
                            continue;
                        }
                        actions.push(if raise_to >= stack_left - 0.01 { Action::AllIn(raise_to) } else { Action::Raise(raise_to) });
                        let new_pot = set_invested(pot, actor, raise_to);
                        children.push(self.build_decision(actor.other(), Some(raise_to), new_pot, board, raises_so_far + 1, streets_left));
                    }
                }
                self.push(NodeKind::Decision { actor, actions, children }, pot)
            }
        }
    }

    /// Builds what happens after `checker` checks: the other player gets to
    /// act, and *their* check ends the street (rather than bouncing back),
    /// which is why this isn't just a recursive call into `build_decision`.
    fn after_check(&mut self, checker: Player, pot: PotState, board: &Board, streets_left: u8) -> NodeId {
        let other = checker.other();
        let stack_left = match other {
            Player::Hero => pot.hero_stack_left(),
            Player::Villain => pot.villain_stack_left(),
        };
        let mut actions = vec![Action::Check];
        let mut children = vec![self.end_of_street(pot, board, streets_left)];

        if stack_left > 0.01 {
            for sizing in self.cfg.sizings.clone() {
                let size = bet_size(pot.pot(), sizing.pot_fraction, stack_left);
                if size <= 0.01 {
                    continue;
                }
                actions.push(if size >= stack_left - 0.01 { Action::AllIn(size) } else { Action::Bet(size) });
                let new_pot = set_invested(pot, other, size);
                children.push(self.build_decision(other.other(), Some(size), new_pot, board, 0, streets_left));
            }
        }
        self.push(NodeKind::Decision { actor: other, actions, children }, pot)
    }

    fn after_call(&mut self, pot: PotState, board: &Board, streets_left: u8) -> NodeId {
        self.end_of_street(pot, board, streets_left)
    }

    /// Street is over (checked through, or bet+call happened). Either deal
    /// the next card (chance node, if we're still extending) or resolve to
    /// a showdown terminal using runout equity for whatever streets remain
    /// unmodeled.
    fn end_of_street(&mut self, pot: PotState, board: &Board, streets_left: u8) -> NodeId {
        use rib_core::Street;
        if board.street() == Street::River || streets_left == 0 {
            return self.push(NodeKind::Terminal(TerminalKind::Showdown { board: board.clone() }), pot);
        }

        let used: Vec<Card> = board.cards.clone();
        let next_cards: Vec<Card> = Card::deck().into_iter().filter(|c| !used.contains(c)).collect();
        let mut branches = Vec::with_capacity(next_cards.len());
        for card in next_cards {
            let mut next_board = board.clone();
            next_board.cards.push(card);
            let first_actor = if self.cfg.hero_in_position { Player::Villain } else { Player::Hero };
            let child = self.build_decision(first_actor, None, pot, &next_board, 0, streets_left - 1);
            branches.push((card, child));
        }
        self.push(NodeKind::Chance { branches }, pot)
    }
}

fn set_invested(pot: PotState, actor: Player, total_this_street_plus_prior: f32) -> PotState {
    match actor {
        Player::Hero => pot.with_hero_to(total_this_street_plus_prior),
        Player::Villain => pot.with_villain_to(total_this_street_plus_prior),
    }
}

/// Bet size (total chips, not delta) for an opening bet: `fraction * pot`,
/// clamped to the available stack. An infinite fraction (the "jam" preset)
/// always means "all remaining chips".
fn bet_size(pot: f32, fraction: f32, stack_left: f32) -> f32 {
    if fraction.is_infinite() {
        return stack_left;
    }
    (pot * fraction).min(stack_left)
}

/// "Pot raise" sizing: the standard formula for a raise sized at `fraction`
/// of the pot *that would exist if the raiser's call were already in*:
/// raise_to = facing + fraction * (pot + 2*facing). Clamped to the stack.
fn raise_size(pot_before_facing: f32, facing: f32, fraction: f32, stack_left: f32) -> f32 {
    if fraction.is_infinite() {
        return stack_left;
    }
    let pot_after_call = pot_before_facing + facing;
    let raise_amount = fraction * (pot_after_call + facing);
    (facing + raise_amount).min(stack_left)
}

pub fn build_tree(first_actor: Player, facing: Option<f32>, pot: PotState, board: Board, cfg: TreeConfig) -> Tree {
    let mut builder = Builder { arena: Vec::new(), cfg: &cfg };
    let root = builder.build_decision(first_actor, facing, pot, &board, 0, cfg.streets_to_extend);
    Tree { arena: builder.arena, root }
}
