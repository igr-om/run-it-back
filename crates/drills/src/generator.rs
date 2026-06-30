//! Picks a category to drill (weighted toward whatever the user's
//! `weakness_profiles` say they're worst at), builds a concrete solvable
//! spot for it, solves it (using the precomputed library when the spot is
//! preflop and matches a curated key, otherwise live), deals the user one
//! specific hand from the relevant range, and persists the drill so
//! `grading.rs` has something to grade the answer against later.

use std::collections::HashMap;

use rand::seq::SliceRandom;
use rand::Rng;
use serde_json::json;
use uuid::Uuid;

use rib_core::{Card, Position};
use rib_db::models::DrillRecord;
use rib_db::{drills as db_drills, solved_spots as db_spots, weakness as db_weakness, Pool as DbPool};
use rib_solver::{solve, NodeContext, SolveRequest, SolveResponse, SpotKey};

pub const CATEGORIES: [&str; 9] = [
    "open_raise",
    "vs_open_defend",
    "vs_three_bet",
    "cbet_flop",
    "vs_cbet_flop",
    "turn_barrel",
    "vs_turn_barrel",
    "river_bet",
    "vs_river_bet",
];

fn category_label(c: &str) -> &'static str {
    match c {
        "open_raise" => "Preflop Open",
        "vs_open_defend" => "Defending vs. an Open",
        "vs_three_bet" => "Facing a 3-Bet",
        "cbet_flop" => "Flop C-Bet",
        "vs_cbet_flop" => "Facing a Flop C-Bet",
        "turn_barrel" => "Turn Barrel",
        "vs_turn_barrel" => "Facing a Turn Barrel",
        "river_bet" => "River Bet",
        "vs_river_bet" => "Facing a River Bet",
        _ => "Mixed",
    }
}

/// Weighted-random category pick: weak categories (low accuracy, high
/// average EV-loss) get drilled more often, with a floor so every category
/// stays reachable and a new user with no history yet sees a uniform mix.
pub async fn pick_category(db: &DbPool, user_id: Uuid, game_type: &str) -> String {
    let profiles = db_weakness::list_for_user(db, user_id, game_type).await.unwrap_or_default();
    let mut weights: HashMap<&str, f64> = CATEGORIES.iter().map(|c| (*c, 1.0)).collect();
    for p in &profiles {
        if let Some(w) = weights.get_mut(p.category.as_str()) {
            let acc = if p.attempts > 0 { p.correct as f64 / p.attempts as f64 } else { 0.5 };
            let ev_loss: f64 = p.avg_ev_loss_bb;
            *w = ((1.5 - acc) + ev_loss.min(3.0) * 0.5).max(0.15);
        }
    }
    let total: f64 = weights.values().sum();
    if total <= 0.0 {
        return CATEGORIES[0].to_string();
    }
    let mut r = rand::thread_rng().gen_range(0.0..total);
    for cat in CATEGORIES {
        let w = weights[cat];
        if r < w {
            return cat.to_string();
        }
        r -= w;
    }
    CATEGORIES[CATEGORIES.len() - 1].to_string()
}

pub struct GeneratedDrill {
    pub record: DrillRecord,
    pub response: SolveResponse,
}

pub async fn generate_and_persist(db: &DbPool, user_id: Uuid, game_type: &str) -> anyhow::Result<GeneratedDrill> {
    let category = pick_category(db, user_id, game_type).await;
    let (spot_key, request, snapshot) = build_spot(&category);
    let response = solve_with_cache(db, spot_key.clone(), &request).await?;

    let label = {
        let mut rng = rand::thread_rng();
        let labels: Vec<&String> = response.hero_strategy.frequencies.keys().collect();
        (*labels.choose(&mut rng).ok_or_else(|| anyhow::anyhow!("solved spot has no combos to deal from"))?).clone()
    };
    let combo = rib_core::Combo::all_169()
        .into_iter()
        .find(|c| c.label() == label)
        .ok_or_else(|| anyhow::anyhow!("solved combo label didn't match a known 169-class"))?;
    let board_cards = parse_board_cards(&request.board);
    let specific = combo
        .expand_specific()
        .into_iter()
        .find(|(a, b)| !board_cards.contains(a) && !board_cards.contains(b))
        .ok_or_else(|| anyhow::anyhow!("every specific combo for this class is blocked by the board"))?;
    let dealt_hand = vec![specific.0.to_string(), specific.1.to_string()];

    let correct_strategy = json!({
        "actions": response.hero_strategy.actions,
        "frequencies": response.hero_strategy.frequencies.get(&label).cloned().unwrap_or_default(),
        "action_ev_bb": response.action_ev_bb.get(&label).cloned().unwrap_or_default(),
    });
    let correct_ev_bb = response
        .hero_strategy
        .frequencies
        .get(&label)
        .zip(response.action_ev_bb.get(&label))
        .map(|(freqs, evs)| freqs.iter().zip(evs).map(|(f, e)| f * e).sum::<f32>())
        .unwrap_or(0.0);

    let record = db_drills::create_drill(
        db,
        user_id,
        game_type,
        &category,
        spot_key.as_ref().map(|k| k.cache_key()).as_deref(),
        snapshot,
        &dealt_hand,
        correct_strategy,
        correct_ev_bb as f64,
    )
    .await?;

    Ok(GeneratedDrill { record, response })
}

fn parse_board_cards(board: &[String]) -> Vec<Card> {
    use std::str::FromStr;
    board.iter().filter_map(|s| Card::from_str(s).ok()).collect()
}

/// Builds the (optional cache key, request, UI-facing snapshot) triple for
/// a category. Preflop categories use "100%" ranges on both sides (the
/// textbook-correct way to let the solver discover the opening/defending
/// range itself, see `rib-worker::seed`); postflop categories use
/// simplified representative ranges for "the range that realistically gets
/// to this spot", since by the flop nobody holds literally any two cards
/// anymore. Those defaults are a deliberate v1 simplification -- the range
/// explorer page lets a user override them with exact ranges for a live,
/// fully custom solve.
fn build_spot(category: &str) -> (Option<SpotKey>, SolveRequest, serde_json::Value) {
    let mut rng = rand::thread_rng();
    let stacks = [20u32, 40, 60, 100];
    let stack_bb = *stacks.choose(&mut rng).unwrap();
    let positions = Position::for_table_size(6);

    match category {
        "open_raise" => {
            let openers: Vec<Position> = positions.iter().copied().filter(|p| *p != Position::Bb).collect();
            let hero = *openers.choose(&mut rng).unwrap();
            // Any live player still behind hero could be the "villain" the
            // solver checks hero's strategy against; BB is the most common
            // and skill-relevant defender, so use it as the representative
            // closing action.
            let villain = Position::Bb;
            let key = SpotKey {
                game: rib_core::GameType::Nlhe,
                pot_type: rib_core::PotType::Srp,
                stack_bb,
                hero_position: hero,
                villain_position: villain,
                board: vec![],
            };
            let req = SolveRequest {
                game: rib_core::GameType::Nlhe,
                board: vec![],
                effective_stack_bb: stack_bb as f32,
                starting_pot_bb: 1.5,
                hero_invested_bb: if hero == Position::Sb { 0.5 } else { 0.0 },
                villain_invested_bb: 1.0,
                hero_range: "100%".into(),
                villain_range: "100%".into(),
                hero_is_in_position: hero.is_late(),
                context: NodeContext::FirstToAct,
                sizings: None,
                streets_to_extend: 0,
                iterations: 800,
            };
            let snap = json!({
                "category_label": category_label(category),
                "hero_position": hero.label(),
                "stack_bb": stack_bb,
                "pot_type": "Single Raised Pot",
                "description": format!("{} folds to you, action on you {}-handed, {} bb effective. First in.", "Everyone", 6, stack_bb),
            });
            (Some(key), req, snap)
        }
        "vs_open_defend" => {
            let non_openers: Vec<Position> = vec![Position::Sb, Position::Bb];
            let hero = *non_openers.choose(&mut rng).unwrap();
            let openers: Vec<Position> = positions.iter().copied().filter(|p| !p.is_blind() && *p != hero).collect();
            let villain = *openers.choose(&mut rng).unwrap_or(&Position::Btn);
            let open_size = 2.2f32;
            let hero_already_posted = if hero == Position::Sb { 0.5 } else { 1.0 };
            let call_amount = open_size - hero_already_posted;
            let pot_before_call = 1.5 + open_size;
            let key = SpotKey {
                game: rib_core::GameType::Nlhe,
                pot_type: rib_core::PotType::Srp,
                stack_bb,
                hero_position: hero,
                villain_position: villain,
                board: vec![],
            };
            let req = SolveRequest {
                game: rib_core::GameType::Nlhe,
                board: vec![],
                effective_stack_bb: stack_bb as f32,
                starting_pot_bb: open_size + 1.0,
                hero_invested_bb: hero_already_posted,
                villain_invested_bb: open_size,
                hero_range: "100%".into(),
                villain_range: "100%".into(),
                hero_is_in_position: false,
                context: NodeContext::FacingBet { size_bb: open_size },
                sizings: None,
                streets_to_extend: 0,
                iterations: 800,
            };
            let snap = json!({
                "category_label": category_label(category),
                "hero_position": hero.label(),
                "villain_position": villain.label(),
                "stack_bb": stack_bb,
                "facing_bb": call_amount,
                "pot_bb": pot_before_call,
                "hero_in_position": false,
                "description": format!("{} opens to {:.1}bb, action on you in the {}.", villain.label(), open_size, hero.label()),
            });
            (Some(key), req, snap)
        }
        "vs_three_bet" => {
            let openers: Vec<Position> = positions.iter().copied().filter(|p| *p != Position::Bb).collect();
            let hero = *openers.choose(&mut rng).unwrap();
            let threebettors: Vec<Position> = vec![Position::Btn, Position::Sb, Position::Bb]
                .into_iter()
                .filter(|p| *p != hero)
                .collect();
            let villain = *threebettors.choose(&mut rng).unwrap_or(&Position::Bb);
            let threebet_size = 8.5f32;
            let call_amount = threebet_size - 2.0;
            let pot_before_call = threebet_size + 2.0;
            let hero_ip = hero.is_late() && !villain.is_late();
            let key = SpotKey {
                game: rib_core::GameType::Nlhe,
                pot_type: rib_core::PotType::ThreeBet,
                stack_bb,
                hero_position: hero,
                villain_position: villain,
                board: vec![],
            };
            let req = SolveRequest {
                game: rib_core::GameType::Nlhe,
                board: vec![],
                effective_stack_bb: stack_bb as f32,
                starting_pot_bb: pot_before_call,
                hero_invested_bb: 2.0,
                villain_invested_bb: threebet_size,
                hero_range: "100%".into(),
                villain_range: "100%".into(),
                hero_is_in_position: hero_ip,
                context: NodeContext::FacingBet { size_bb: threebet_size },
                sizings: None,
                streets_to_extend: 0,
                iterations: 800,
            };
            let snap = json!({
                "category_label": category_label(category),
                "hero_position": hero.label(),
                "villain_position": villain.label(),
                "stack_bb": stack_bb,
                "facing_bb": call_amount,
                "pot_bb": pot_before_call,
                "hero_in_position": hero_ip,
                "description": format!("You open, {} 3-bets to {:.1}bb, action back on you.", villain.label(), threebet_size),
            });
            (Some(key), req, snap)
        }
        "cbet_flop" | "vs_cbet_flop" => postflop_spot(category, PostflopStreet::Flop, stack_bb, &mut rng),
        "turn_barrel" | "vs_turn_barrel" => postflop_spot(category, PostflopStreet::Turn, stack_bb, &mut rng),
        "river_bet" | "vs_river_bet" => postflop_spot(category, PostflopStreet::River, stack_bb, &mut rng),
        _ => build_spot("open_raise"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PostflopStreet {
    Flop,
    Turn,
    River,
}

impl PostflopStreet {
    fn board_size(self) -> usize {
        match self {
            PostflopStreet::Flop => 3,
            PostflopStreet::Turn => 4,
            PostflopStreet::River => 5,
        }
    }

    fn name(self) -> &'static str {
        match self {
            PostflopStreet::Flop => "flop",
            PostflopStreet::Turn => "turn",
            PostflopStreet::River => "river",
        }
    }
}

/// Shared builder for every street-specific postflop category
/// (`cbet_flop`/`vs_cbet_flop`, `turn_barrel`/`vs_turn_barrel`,
/// `river_bet`/`vs_river_bet`). The pattern is identical at every street:
/// deal a random board of the right length, narrow both players'
/// representative ranges a bit further than the previous street (since by
/// the turn/river, players who'd have folded already are gone from both
/// ranges), and either let hero act first (the "_bet"/"cbet_flop"
/// variants, where hero is the continuing preflop/flop aggressor) or have
/// hero face a bet (the "vs_*" variants).
///
/// River boards resolve to an *exact* showdown comparison (no Monte Carlo
/// runout needed, since all 5 cards are already known) -- the same tree
/// builder that approximates flop/turn spots with sampled runouts handles
/// this automatically based on board length, so nothing here needs to
/// special-case it.
fn postflop_spot(
    category: &str,
    street: PostflopStreet,
    stack_bb: u32,
    rng: &mut impl rand::Rng,
) -> (Option<SpotKey>, SolveRequest, serde_json::Value) {
    let mut deck = Card::deck();
    deck.shuffle(rng);
    let board: Vec<Card> = deck[..street.board_size()].to_vec();

    // Ranges get progressively narrower at later streets, modeling that
    // players who'd have given up already aren't still in either range by
    // the turn or river. These are still representative/simplified ranges
    // (see the module-level doc comment), not a chained solve of what
    // actually survives from the prior street's exact solved strategy --
    // a reasonable v1 approximation, refinable later.
    let (aggressor_range, caller_range, pot, sizing_fraction): (&str, &str, f32, f32) = match street {
        PostflopStreet::Flop => ("22+,A2+,K2+,Q4+,J7+,T7+,98s,87s,76s,65s,54s", "22+,A5+,K8+,Q9+,J9+,T8+,98s,87s,76s", 5.0, 0.66),
        PostflopStreet::Turn => ("33+,A5+,K8+,Q9+,J9s+,T9s,98s,87s,76s", "33+,A8+,K9+,QTs+,JTs,T9s,98s", 12.0, 0.70),
        PostflopStreet::River => ("66+,A9+,KJ+,QJs,JTs,T9s,98s", "66+,ATs+,AJo+,KQs,KQo,QJs", 26.0, 0.75),
    };

    let hero_is_aggressor = category == "cbet_flop" || category == "turn_barrel" || category == "river_bet";
    let (hero_range, villain_range, hero_ip) = if hero_is_aggressor {
        (aggressor_range.to_string(), caller_range.to_string(), true)
    } else {
        (caller_range.to_string(), aggressor_range.to_string(), false)
    };

    let facing_bb = pot * sizing_fraction;
    let req = SolveRequest {
        game: rib_core::GameType::Nlhe,
        board: board.iter().map(|c| c.to_string()).collect(),
        effective_stack_bb: stack_bb as f32,
        starting_pot_bb: pot,
        hero_invested_bb: pot / 2.0,
        villain_invested_bb: pot / 2.0,
        hero_range,
        villain_range,
        hero_is_in_position: hero_ip,
        context: if hero_is_aggressor { NodeContext::FirstToAct } else { NodeContext::FacingBet { size_bb: facing_bb } },
        sizings: None,
        streets_to_extend: 0,
        iterations: if street == PostflopStreet::River { 800 } else { 600 },
    };

    let description = if hero_is_aggressor {
        format!(
            "You've been the aggressor and it's checked to you on the {}.",
            street.name()
        )
    } else {
        format!("Villain bets into you on the {}.", street.name())
    };

    let mut snap = json!({
        "category_label": category_label(category),
        "board": board.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        "stack_bb": stack_bb,
        "pot_bb": pot,
        "hero_in_position": hero_ip,
        "description": description,
    });
    if !hero_is_aggressor {
        snap["facing_bb"] = json!(facing_bb);
    }
    // Postflop spots use a random board every time, so they aren't
    // cacheable against the curated (preflop-only) library.
    (None, req, snap)
}

async fn solve_with_cache(db: &DbPool, key: Option<SpotKey>, request: &SolveRequest) -> anyhow::Result<SolveResponse> {
    if let Some(k) = &key {
        if let Ok(Some(row)) = db_spots::get_solved_spot(db, &k.cache_key()).await {
            if let Ok(resp) = serde_json::from_value::<SolveResponse>(row.response) {
                return Ok(resp);
            }
        }
    }
    let response = solve(request)?;
    if let Some(k) = &key {
        let value = serde_json::to_value(&response).unwrap_or_else(|_| json!({}));
        let _ = db_spots::upsert_solved_spot(
            db,
            &k.cache_key(),
            "nlhe",
            &format!("{:?}", k.pot_type),
            k.stack_bb as i32,
            k.hero_position.label(),
            k.villain_position.label(),
            &k.board,
            value,
            response.iterations_run as i32,
        )
        .await;
    }
    Ok(response)
}
