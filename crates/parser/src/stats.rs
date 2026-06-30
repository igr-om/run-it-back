//! Turns a `Vec<ParsedHand>` into (a) per-hand tags, stored alongside each
//! hand so the UI can filter "show me every hand where I faced a 3-bet",
//! and (b) aggregate percentages (VPIP, PFR, 3-bet%, c-bet%, ...) for the
//! stats dashboard. The same tag vocabulary is reused by `rib-drills` as
//! weakness categories, so a leak found in someone's real hand history and
//! a leak found in the drill trainer point at the same underlying skill.

use std::collections::HashMap;

use rib_core::Street;

use crate::model::{ActionKind, ParsedHand};

pub fn derive_tags(hand: &ParsedHand) -> Vec<String> {
    let mut tags = Vec::new();
    let hero_actions = || hand.actions.iter().filter(|a| a.is_hero);

    let preflop_raises_before_hero = |hero_idx: usize| {
        hand.actions[..hero_idx]
            .iter()
            .filter(|a| a.street == Street::Preflop && a.kind == ActionKind::Raise)
            .count()
    };

    let vpip = hero_actions().any(|a| {
        a.street == Street::Preflop && matches!(a.kind, ActionKind::Call | ActionKind::Bet | ActionKind::Raise)
    });
    if vpip {
        tags.push("vpip".to_string());
    }

    let mut hero_pf_raise_idx: Option<usize> = None;
    for (i, a) in hand.actions.iter().enumerate() {
        if a.is_hero && a.street == Street::Preflop && a.kind == ActionKind::Raise {
            hero_pf_raise_idx = Some(i);
            break;
        }
    }
    if let Some(idx) = hero_pf_raise_idx {
        tags.push("pfr".to_string());
        match preflop_raises_before_hero(idx) {
            0 => tags.push("open_raise".to_string()),
            1 => tags.push("three_bet".to_string()),
            2 => tags.push("four_bet".to_string()),
            _ => tags.push("five_bet_plus".to_string()),
        }
    }

    // Did hero face exactly one preflop raise before acting, and fold? ->
    // fold_to_three_bet (hero had opened, someone 3-bet, hero folded).
    if let Some(fold_idx) = hand
        .actions
        .iter()
        .position(|a| a.is_hero && a.street == Street::Preflop && a.kind == ActionKind::Fold)
    {
        let raises_before = preflop_raises_before_hero(fold_idx);
        let hero_raised_earlier = hand.actions[..fold_idx]
            .iter()
            .any(|a| a.is_hero && a.street == Street::Preflop && a.kind == ActionKind::Raise);
        if raises_before == 2 && hero_raised_earlier {
            tags.push("fold_to_three_bet".to_string());
        } else if raises_before == 3 {
            tags.push("fold_to_four_bet".to_string());
        }
    }

    // C-bet: hero was the last preflop aggressor and is the first to bet on
    // the flop/turn.
    if let Some(last_pf_raise_idx) = hand
        .actions
        .iter()
        .enumerate()
        .filter(|(_, a)| a.street == Street::Preflop && a.kind == ActionKind::Raise)
        .last()
        .map(|(i, _)| i)
    {
        if hand.actions[last_pf_raise_idx].is_hero {
            if let Some(first_flop_bet) = hand.actions.iter().find(|a| a.street == Street::Flop && a.kind == ActionKind::Bet) {
                if first_flop_bet.is_hero {
                    tags.push("cbet_flop".to_string());
                } else {
                    tags.push("faced_cbet_flop".to_string());
                    let hero_folded_to_it = hand
                        .actions
                        .iter()
                        .skip_while(|a| !std::ptr::eq(*a, first_flop_bet))
                        .find(|a| a.is_hero && a.street == Street::Flop)
                        .map(|a| a.kind == ActionKind::Fold)
                        .unwrap_or(false);
                    if hero_folded_to_it {
                        tags.push("fold_to_cbet_flop".to_string());
                    }
                }
            }
        }
    }

    if hand.went_to_showdown {
        tags.push("wtsd".to_string());
    }
    if hand.won_hand {
        tags.push(if hand.went_to_showdown { "won_showdown".to_string() } else { "won_uncontested".to_string() });
    }

    tags
}

#[derive(Debug, Clone, Default)]
pub struct AggregateStats {
    pub sample_size: i32,
    pub vpip: f64,
    pub pfr: f64,
    pub three_bet: f64,
    pub fold_to_three_bet: f64,
    pub cbet_flop: f64,
    pub fold_to_cbet_flop: f64,
    pub cbet_turn: f64,
    pub wtsd: f64,
    pub won_at_showdown: f64,
    pub aggression_factor: f64,
    pub net_bb_per_100: f64,
}

pub fn aggregate(hands: &[ParsedHand]) -> AggregateStats {
    let n = hands.len().max(1) as f64;
    let pct = |count: usize| -> f64 { 100.0 * count as f64 / n };

    let has = |h: &ParsedHand, tag: &str| h.tags.iter().any(|t| t == tag);

    let vpip = hands.iter().filter(|h| has(h, "vpip")).count();
    let pfr = hands.iter().filter(|h| has(h, "pfr")).count();
    let three_bet = hands.iter().filter(|h| has(h, "three_bet")).count();
    let faced_three_bet = hands
        .iter()
        .filter(|h| has(h, "open_raise"))
        .filter(|h| h.actions.iter().any(|a| !a.is_hero && a.street == Street::Preflop && a.kind == ActionKind::Raise))
        .count();
    let fold_to_three_bet = hands.iter().filter(|h| has(h, "fold_to_three_bet")).count();
    let cbet_flop = hands.iter().filter(|h| has(h, "cbet_flop")).count();
    let faced_cbet_flop = hands.iter().filter(|h| has(h, "faced_cbet_flop")).count();
    let fold_to_cbet_flop = hands.iter().filter(|h| has(h, "fold_to_cbet_flop")).count();
    let wtsd = hands.iter().filter(|h| h.went_to_showdown).count();
    let won_at_showdown = hands.iter().filter(|h| h.went_to_showdown && h.won_hand).count();

    let total_aggressive: f64 = hands
        .iter()
        .flat_map(|h| h.actions.iter())
        .filter(|a| a.is_hero && matches!(a.kind, ActionKind::Bet | ActionKind::Raise))
        .count() as f64;
    let total_calls: f64 = hands
        .iter()
        .flat_map(|h| h.actions.iter())
        .filter(|a| a.is_hero && a.kind == ActionKind::Call)
        .count() as f64;
    let aggression_factor = if total_calls > 0.0 { total_aggressive / total_calls } else { total_aggressive };

    let net_bb: f64 = hands.iter().map(|h| h.result_bb).sum();

    AggregateStats {
        sample_size: hands.len() as i32,
        vpip: pct(vpip),
        pfr: pct(pfr),
        three_bet: if faced_three_bet > 0 { 100.0 * three_bet as f64 / faced_three_bet.max(1) as f64 } else { pct(three_bet) },
        fold_to_three_bet: if fold_to_three_bet > 0 { pct(fold_to_three_bet) } else { 0.0 },
        cbet_flop: if faced_cbet_flop + cbet_flop > 0 { 100.0 * cbet_flop as f64 / (cbet_flop as f64).max(1.0) } else { 0.0 },
        fold_to_cbet_flop: if faced_cbet_flop > 0 { 100.0 * fold_to_cbet_flop as f64 / faced_cbet_flop as f64 } else { 0.0 },
        cbet_turn: 0.0, // left for a future pass once turn-cbet sequencing mirrors the flop logic above
        wtsd: pct(wtsd),
        won_at_showdown: if wtsd > 0 { 100.0 * won_at_showdown as f64 / wtsd as f64 } else { 0.0 },
        aggression_factor,
        net_bb_per_100: 100.0 * net_bb / n,
    }
}

/// Groups hero's hand-history hands by tag, used by the drill generator to
/// pull real hands from a user's own history for a weak category instead of
/// only synthetic solved spots (e.g. "you struggled with fold-to-3bet
/// spots -- here are 3 from your own play to review").
pub fn group_by_tag(hands: &[ParsedHand]) -> HashMap<String, Vec<usize>> {
    let mut map: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, h) in hands.iter().enumerate() {
        for tag in &h.tags {
            map.entry(tag.clone()).or_default().push(i);
        }
    }
    map
}
