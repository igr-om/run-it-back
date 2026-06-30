use crate::hand_index::HandIndex;
use crate::payoff::PayoffTable;
use crate::request::Player;
use crate::tree::{NodeId, NodeKind, TerminalKind, Tree};

/// Per-node regret and cumulative-strategy tables, shaped [node][hand][action].
/// Only `Decision` nodes have non-empty rows; other node kinds get an empty
/// `Vec` placeholder so indices line up with the tree's arena.
pub struct Tables {
    pub regrets: Vec<Vec<Vec<f32>>>,
    pub strategy_sum: Vec<Vec<Vec<f32>>>,
}

impl Tables {
    pub fn init(tree: &Tree, n_hero: usize, n_villain: usize) -> Self {
        let mut regrets = Vec::with_capacity(tree.arena.len());
        let mut strategy_sum = Vec::with_capacity(tree.arena.len());
        for node in &tree.arena {
            match &node.kind {
                NodeKind::Decision { actor, actions, .. } => {
                    let n_hands = match actor {
                        Player::Hero => n_hero,
                        Player::Villain => n_villain,
                    };
                    regrets.push(vec![vec![0f32; actions.len()]; n_hands]);
                    strategy_sum.push(vec![vec![0f32; actions.len()]; n_hands]);
                }
                _ => {
                    regrets.push(Vec::new());
                    strategy_sum.push(Vec::new());
                }
            }
        }
        Self { regrets, strategy_sum }
    }
}

/// Regret matching (CFR+ flavor: regrets are floored at zero immediately).
fn regret_matching(rows: &[Vec<f32>]) -> Vec<Vec<f32>> {
    rows.iter()
        .map(|row| {
            let positive_sum: f32 = row.iter().map(|x| x.max(0.0)).sum();
            if positive_sum > 1e-9 {
                row.iter().map(|x| x.max(0.0) / positive_sum).collect()
            } else {
                vec![1.0 / row.len() as f32; row.len()]
            }
        })
        .collect()
}

/// Average strategy from cumulative `strategy_sum` rows (what's actually
/// reported to the user, since CFR's *average* strategy is the one that
/// provably converges to a Nash equilibrium of the abstracted game, not the
/// last iteration's strategy).
fn average_strategy(rows: &[Vec<f32>]) -> Vec<Vec<f32>> {
    rows.iter()
        .map(|row| {
            let sum: f32 = row.iter().sum();
            if sum > 1e-9 {
                row.iter().map(|x| x / sum).collect()
            } else {
                vec![1.0 / row.len() as f32; row.len()]
            }
        })
        .collect()
}

struct Ctx<'a> {
    tree: &'a Tree,
    payoffs: &'a PayoffTable,
    hero_hands: &'a [HandIndex],
    villain_hands: &'a [HandIndex],
}

/// One full recursive walk of the tree. When `iter` is `Some(t)` this is a
/// *training* pass: it reads the live regret-matching strategy, recurses,
/// then updates regrets (CFR+) and accumulates `strategy_sum` weighted by
/// iteration number `t` (linear averaging, which converges faster than
/// uniform averaging -- this is the "+" in CFR+). When `iter` is `None`,
/// this is a final *evaluation* pass: it reads the converged average
/// strategy and performs no updates, used to report the EV the trained
/// strategy actually achieves.
fn walk(
    ctx: &Ctx,
    tables: &mut Tables,
    node_id: NodeId,
    hero_reach: &[f32],
    villain_reach: &[f32],
    iter: Option<u32>,
) -> (Vec<f32>, Vec<f32>) {
    let node = &ctx.tree.arena[node_id];
    match &node.kind {
        NodeKind::Terminal(TerminalKind::Fold { folder }) => {
            let pot = node.pot;
            let (hero_val, villain_val) = match folder {
                Player::Hero => (-pot.hero_invested, pot.hero_invested),
                Player::Villain => (pot.villain_invested, -pot.villain_invested),
            };
            (vec![hero_val; hero_reach.len()], vec![villain_val; villain_reach.len()])
        }
        NodeKind::Terminal(TerminalKind::Showdown { .. }) => {
            let matrix = &ctx.payoffs[&node_id];
            let n_h = hero_reach.len();
            let n_v = villain_reach.len();
            let mut util_h = vec![0f32; n_h];
            let mut util_v = vec![0f32; n_v];
            for i in 0..n_h {
                let mut s = 0f32;
                for j in 0..n_v {
                    s += villain_reach[j] * matrix[i][j];
                }
                util_h[i] = s;
            }
            for j in 0..n_v {
                let mut s = 0f32;
                for i in 0..n_h {
                    s += hero_reach[i] * (-matrix[i][j]);
                }
                util_v[j] = s;
            }
            (util_h, util_v)
        }
        NodeKind::Chance { branches } => {
            let n_h = hero_reach.len();
            let n_v = villain_reach.len();
            let mut util_h = vec![0f32; n_h];
            let mut util_v = vec![0f32; n_v];
            let n = branches.len().max(1) as f32;
            for (card, child) in branches {
                let hero_r2: Vec<f32> = hero_reach
                    .iter()
                    .zip(ctx.hero_hands)
                    .map(|(r, h)| if h.cards.contains(card) { 0.0 } else { *r })
                    .collect();
                let villain_r2: Vec<f32> = villain_reach
                    .iter()
                    .zip(ctx.villain_hands)
                    .map(|(r, h)| if h.cards.contains(card) { 0.0 } else { *r })
                    .collect();
                let (ch, cv) = walk(ctx, tables, *child, &hero_r2, &villain_r2, iter);
                for i in 0..n_h {
                    util_h[i] += ch[i] / n;
                }
                for j in 0..n_v {
                    util_v[j] += cv[j] / n;
                }
            }
            (util_h, util_v)
        }
        NodeKind::Decision { actor, actions, children } => {
            let n_actions = actions.len();
            let sigma = match iter {
                Some(_) => regret_matching(&tables.regrets[node_id]),
                None => average_strategy(&tables.strategy_sum[node_id]),
            };
            let acting_reach = match actor {
                Player::Hero => hero_reach,
                Player::Villain => villain_reach,
            };
            let n_acting = acting_reach.len();

            let mut action_utils_acting: Vec<Vec<f32>> = Vec::with_capacity(n_actions);
            let mut action_utils_other: Vec<Vec<f32>> = Vec::with_capacity(n_actions);

            for (a_idx, &child) in children.iter().enumerate() {
                let new_acting_reach: Vec<f32> = (0..n_acting).map(|i| acting_reach[i] * sigma[i][a_idx]).collect();
                let (uh, uv) = match actor {
                    Player::Hero => walk(ctx, tables, child, &new_acting_reach, villain_reach, iter),
                    Player::Villain => walk(ctx, tables, child, hero_reach, &new_acting_reach, iter),
                };
                match actor {
                    Player::Hero => {
                        action_utils_acting.push(uh);
                        action_utils_other.push(uv);
                    }
                    Player::Villain => {
                        action_utils_acting.push(uv);
                        action_utils_other.push(uh);
                    }
                }
            }

            let mut node_util_acting = vec![0f32; n_acting];
            for i in 0..n_acting {
                for a in 0..n_actions {
                    node_util_acting[i] += sigma[i][a] * action_utils_acting[a][i];
                }
            }
            let other_len = match actor {
                Player::Hero => villain_reach.len(),
                Player::Villain => hero_reach.len(),
            };
            let mut node_util_other = vec![0f32; other_len];
            for a in 0..n_actions {
                for j in 0..other_len {
                    node_util_other[j] += action_utils_other[a][j];
                }
            }

            if let Some(t) = iter {
                for i in 0..n_acting {
                    for a in 0..n_actions {
                        let regret = action_utils_acting[a][i] - node_util_acting[i];
                        let r = &mut tables.regrets[node_id][i][a];
                        *r = (*r + regret).max(0.0);
                        tables.strategy_sum[node_id][i][a] += (t as f32) * sigma[i][a];
                    }
                }
            }

            match actor {
                Player::Hero => (node_util_acting, node_util_other),
                Player::Villain => (node_util_other, node_util_acting),
            }
        }
    }
}

pub struct TrainResult {
    pub tables: Tables,
    pub root_hero_ev: Vec<f32>,
    pub root_villain_ev: Vec<f32>,
    /// Rough convergence diagnostic: total positive regret accumulated at
    /// the root decision node, divided by iteration count. Not a rigorous
    /// best-response exploitability calculation, but trends toward zero as
    /// the strategy converges and is cheap to compute from tables we
    /// already have.
    pub exploitability_estimate: f32,
}

pub fn train(
    tree: &Tree,
    payoffs: &PayoffTable,
    hero_hands: &[HandIndex],
    villain_hands: &[HandIndex],
    hero_initial_reach: &[f32],
    villain_initial_reach: &[f32],
    iterations: u32,
) -> TrainResult {
    let ctx = Ctx { tree, payoffs, hero_hands, villain_hands };
    let mut tables = Tables::init(tree, hero_hands.len(), villain_hands.len());

    for t in 1..=iterations.max(1) {
        walk(&ctx, &mut tables, tree.root, hero_initial_reach, villain_initial_reach, Some(t));
    }

    let (root_hero_ev, root_villain_ev) = walk(&ctx, &mut tables, tree.root, hero_initial_reach, villain_initial_reach, None);

    let exploitability_estimate = if let NodeKind::Decision { .. } = &tree.arena[tree.root].kind {
        let total_regret: f32 = tables.regrets[tree.root].iter().flatten().map(|x| x.max(0.0)).sum();
        total_regret / iterations.max(1) as f32
    } else {
        0.0
    };

    TrainResult { tables, root_hero_ev, root_villain_ev, exploitability_estimate }
}

/// Pull the converged average strategy out at any node (hand_index ->
/// per-action probabilities). Used to build the root response, and
/// available for callers (e.g. the drill grader) that want a deeper node.
pub fn strategy_at(tables: &Tables, node_id: NodeId) -> Vec<Vec<f32>> {
    average_strategy(&tables.strategy_sum[node_id])
}

pub fn uniform_reach(hands: &[HandIndex]) -> Vec<f32> {
    hands.iter().map(|h| h.class_weight).collect()
}

/// EV for hero, broken down *per action* at the root (rather than blended
/// by the converged mix), assuming hero forces that action with every hand
/// while everyone else continues to play their converged equilibrium
/// strategy in every subsequent node. This is what the drill grader uses
/// to explain a wrong answer: "you chose X, but Y was worth `n` bb more
/// with this exact hand" needs the EV *of* X and *of* Y individually, which
/// `train`'s blended root EV alone can't give you. Returns one
/// `Vec<f32>` (per hero hand index) per root action, in the same order as
/// the root's action list; `None` if the root isn't a hero decision (the
/// request had villain acting first, e.g. a "facing a bet" spot from
/// villain's seat -- grading isn't meaningful there since hero never had a
/// choice to grade).
pub fn root_action_ev_breakdown(
    tree: &Tree,
    payoffs: &PayoffTable,
    hero_hands: &[HandIndex],
    villain_hands: &[HandIndex],
    hero_reach: &[f32],
    villain_reach: &[f32],
    tables: &mut Tables,
) -> Option<Vec<Vec<f32>>> {
    let (actor, children): (Player, Vec<NodeId>) = match &tree.arena[tree.root].kind {
        NodeKind::Decision { actor, children, .. } => (*actor, children.clone()),
        _ => return None,
    };
    if actor != Player::Hero {
        return None;
    }
    let ctx = Ctx { tree, payoffs, hero_hands, villain_hands };
    let mut out = Vec::with_capacity(children.len());
    for &child in &children {
        let (uh, _uv) = walk(&ctx, tables, child, hero_reach, villain_reach, None);
        out.push(uh);
    }
    Some(out)
}
