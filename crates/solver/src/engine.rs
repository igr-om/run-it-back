use rib_core::{GameType, RibError, Strategy};

use crate::cfr::{strategy_at, train, uniform_reach};
use crate::hand_index::{build_omaha_universe, build_universe, HandIndex};
use crate::payoff::{precompute_payoffs, DEFAULT_RUNOUT_SAMPLES};
use crate::pot::PotState;
use crate::request::{parse_board, parse_weighted_range, resolve_sizings, NodeContext, Player, SolveRequest, SolveResponse};
use crate::tree::{build_tree, NodeKind, TreeConfig};

/// Iteration count is clamped server-side regardless of what the client
/// asks for, so a "live" solve always returns in roughly real time. The
/// precomputed library (see `library.rs`) is what makes common spots feel
/// instant rather than capped-but-still-a-few-seconds.
pub const MAX_LIVE_ITERATIONS: u32 = 1500;
pub const MIN_LIVE_ITERATIONS: u32 = 50;
const MAX_RAISES_PER_STREET: u8 = 2;
/// This is the single most important performance guard in the whole
/// solver. The payoff precomputation is O(hero_combos x villain_combos),
/// and every one of those pairs needs its own Monte Carlo equity
/// calculation -- *before a single CFR iteration runs*, so capping
/// iterations alone does nothing to bound this cost. A "100%" NLHE range
/// carries 1,326 specific combos (an unrestricted Omaha range carries far
/// more); left uncapped, that many pairs at the default sample count
/// would take on the order of minutes for a single spot. Subsampling down
/// to a few dozen representative combos per side (weighted so the subset
/// still reflects the original range's class proportions) keeps every
/// live solve interactive regardless of how wide the input ranges are.
const MAX_COMBOS_PER_SIDE: usize = 18;

pub fn solve(req: &SolveRequest) -> Result<SolveResponse, RibError> {
    let board = parse_board(&req.board)?;
    let mut warnings = Vec::new();

    let (mut hero_hands, mut villain_hands) = match req.game {
        GameType::Nlhe => {
            let hero_range = parse_weighted_range(&req.hero_range)?;
            let villain_range = parse_weighted_range(&req.villain_range)?;
            if hero_range.is_empty() {
                return Err(RibError::Range("hero_range resolved to zero combos".into()));
            }
            if villain_range.is_empty() {
                return Err(RibError::Range("villain_range resolved to zero combos".into()));
            }
            (build_universe(&hero_range, &board.cards), build_universe(&villain_range, &board.cards))
        }
        GameType::Plo => {
            let hero_range = rib_core::parse_omaha_range(&req.hero_range)?;
            let villain_range = rib_core::parse_omaha_range(&req.villain_range)?;
            if hero_range.is_empty() {
                return Err(RibError::Range("hero_range resolved to zero combos".into()));
            }
            if villain_range.is_empty() {
                return Err(RibError::Range("villain_range resolved to zero combos".into()));
            }
            (build_omaha_universe(&hero_range, &board.cards), build_omaha_universe(&villain_range, &board.cards))
        }
    };
    if hero_hands.is_empty() || villain_hands.is_empty() {
        return Err(RibError::Range("range fully blocked by the board -- no valid combos remain".into()));
    }

    let mut rng = rand::thread_rng();
    if hero_hands.len() > MAX_COMBOS_PER_SIDE {
        warnings.push(format!(
            "Hero range had {} specific combos; sampled down to {MAX_COMBOS_PER_SIDE} to keep this solve interactive.",
            hero_hands.len()
        ));
        hero_hands = crate::hand_index::subsample(&hero_hands, MAX_COMBOS_PER_SIDE, &mut rng);
    }
    if villain_hands.len() > MAX_COMBOS_PER_SIDE {
        warnings.push(format!(
            "Villain range had {} specific combos; sampled down to {MAX_COMBOS_PER_SIDE} to keep this solve interactive.",
            villain_hands.len()
        ));
        villain_hands = crate::hand_index::subsample(&villain_hands, MAX_COMBOS_PER_SIDE, &mut rng);
    }

    if req.game == GameType::Plo {
        warnings.push(
            "This is a genuine CFR+ equilibrium for the bet sizes actually offered, same as the NLHE solver -- both games abstract continuous bet sizing down to a fixed menu so an on-demand solve is tractable at all, which is the same trade every real-time solver makes, just at a coarser grain. \
             The one gap specific to PLO: a real pot-limit raise can never exceed the pot, but this solver's sizing menu (including the \"All-in\" option) doesn't compute or enforce that cap, so in deep-stacks-relative-to-pot spots it may offer (and find +EV for) a sizing too large to be legal at a real table. NLHE has no equivalent issue since any size up to the stack is always legal there."
                .to_string(),
        );
    }

    let pot = PotState {
        hero_invested: req.hero_invested_bb,
        villain_invested: req.villain_invested_bb,
        effective_stack: req.effective_stack_bb,
    };

    let sizings = resolve_sizings(&req.sizings);
    let streets_to_extend = req.streets_to_extend.min(1);
    let cfg = TreeConfig {
        sizings,
        reraise_sizings: rib_core::SizingOption::reraise_set(),
        max_raises_per_street: MAX_RAISES_PER_STREET,
        streets_to_extend,
        hero_in_position: req.hero_is_in_position,
    };

    let facing = match req.context {
        NodeContext::FirstToAct => None,
        NodeContext::FacingBet { size_bb } => Some(size_bb),
    };

    let tree = build_tree(Player::Hero, facing, pot, board, cfg);

    let runout_samples = if streets_to_extend == 0 { DEFAULT_RUNOUT_SAMPLES } else { DEFAULT_RUNOUT_SAMPLES / 2 };
    let payoffs = precompute_payoffs(&tree, &hero_hands, &villain_hands, runout_samples.max(40));

    let hero_reach = uniform_reach(&hero_hands);
    let villain_reach = uniform_reach(&villain_hands);

    let iterations = req.iterations.clamp(MIN_LIVE_ITERATIONS, MAX_LIVE_ITERATIONS);
    let mut result = train(&tree, &payoffs, &hero_hands, &villain_hands, &hero_reach, &villain_reach, iterations);

    let root_actions = match &tree.arena[tree.root].kind {
        NodeKind::Decision { actions, .. } => actions.clone(),
        _ => return Err(RibError::Solver("root node has no decision -- nothing to solve".into())),
    };

    let strategy_rows = strategy_at(&result.tables, tree.root);
    let strategy = roll_up_to_grid(&hero_hands, &root_actions, &strategy_rows);

    let action_ev_breakdown = crate::cfr::root_action_ev_breakdown(
        &tree, &payoffs, &hero_hands, &villain_hands, &hero_reach, &villain_reach, &mut result.tables,
    );
    let action_ev_bb = roll_up_action_ev(&hero_hands, action_ev_breakdown);

    let reach_sum: f32 = hero_reach.iter().sum::<f32>().max(1e-9);
    let hero_ev_bb = result
        .root_hero_ev
        .iter()
        .zip(hero_reach.iter())
        .map(|(ev, w)| ev * w)
        .sum::<f32>()
        / reach_sum;
    let v_reach_sum: f32 = villain_reach.iter().sum::<f32>().max(1e-9);
    let villain_ev_bb = result
        .root_villain_ev
        .iter()
        .zip(villain_reach.iter())
        .map(|(ev, w)| ev * w)
        .sum::<f32>()
        / v_reach_sum;

    Ok(SolveResponse {
        hero_strategy: strategy,
        action_ev_bb,
        hero_ev_bb,
        villain_ev_bb,
        iterations_run: iterations,
        exploitability_estimate: result.exploitability_estimate,
        n_hero_combos: hero_hands.len(),
        n_villain_combos: villain_hands.len(),
        warnings,
    })
}

/// Same averaging idea as `roll_up_to_grid`, but for the per-action EV
/// breakdown: class label -> EV per action, averaged over every specific
/// combo sharing that label.
fn roll_up_action_ev(hands: &[HandIndex], breakdown: Option<Vec<Vec<f32>>>) -> std::collections::HashMap<String, Vec<f32>> {
    use std::collections::HashMap;
    let Some(per_action) = breakdown else {
        return HashMap::new();
    };
    let n_actions = per_action.len();
    let mut sums: HashMap<String, Vec<f32>> = HashMap::new();
    let mut counts: HashMap<String, f32> = HashMap::new();

    for (hand_idx, hand) in hands.iter().enumerate() {
        let label = hand.class_label.clone();
        let entry = sums.entry(label.clone()).or_insert_with(|| vec![0f32; n_actions]);
        for a in 0..n_actions {
            entry[a] += per_action[a][hand_idx];
        }
        *counts.entry(label).or_insert(0.0) += 1.0;
    }

    sums.into_iter()
        .map(|(label, sum)| {
            let n = counts[&label];
            (label, sum.into_iter().map(|x| x / n).collect())
        })
        .collect()
}

/// Average the (already-solved) per-specific-combo strategy back up to the
/// class-level grid the UI renders, weighted by each combo's class weight.
/// This is what GTOWizard-style range grids show as "this cell is 70% bet /
/// 30% check" even though, strictly, the solver reasons over removal-aware
/// specific combos underneath.
fn roll_up_to_grid(hands: &[HandIndex], actions: &[rib_core::Action], strategy_rows: &[Vec<f32>]) -> Strategy {
    use std::collections::HashMap;
    let mut sums: HashMap<String, Vec<f32>> = HashMap::new();
    let mut counts: HashMap<String, f32> = HashMap::new();

    for (hand, row) in hands.iter().zip(strategy_rows.iter()) {
        let label = hand.class_label.clone();
        let entry = sums.entry(label.clone()).or_insert_with(|| vec![0f32; actions.len()]);
        for (i, p) in row.iter().enumerate() {
            entry[i] += p;
        }
        *counts.entry(label).or_insert(0.0) += 1.0;
    }

    let mut frequencies = HashMap::new();
    for (label, sum) in sums {
        let n = counts[&label];
        frequencies.insert(label, sum.into_iter().map(|x| x / n).collect());
    }

    Strategy { actions: actions.to_vec(), frequencies, ev_bb: HashMap::new() }
}
