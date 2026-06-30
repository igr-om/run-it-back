//! Grading does two things: (1) score the answer (EV-loss in bb, a
//! correctness boolean with a small tolerance since GTO strategies often
//! genuinely mix), and (2) explain *why* in terms of real poker concepts.
//! The explanation engine in this file is rule-based, not a lookup table of
//! canned strings per category -- each rule inspects the actual situation
//! (bet sizes, position, board texture, blockers, stack depth) and only
//! fires when it's actually relevant, so a fold-equity explanation only
//! shows up when the mistake was actually about missing fold equity.

use rib_core::{Action, Board, Card, Rank};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct GradeResult {
    pub is_correct: bool,
    pub chosen_action: String,
    pub best_action: String,
    pub chosen_ev_bb: f32,
    pub best_ev_bb: f32,
    pub ev_loss_bb: f32,
    pub explanation: String,
}

/// A mistake smaller than this is within CFR/abstraction noise or a
/// genuinely close decision -- not worth flagging as "wrong".
const EV_TOLERANCE_BB: f32 = 0.04;
/// ...or, even if the raw EV gap is bigger, if the solver itself plays the
/// chosen action a meaningful fraction of the time, it's a legitimate part
/// of the equilibrium mix, not an error.
const FREQUENCY_TOLERANCE: f32 = 0.15;

pub fn grade(
    actions: &[Action],
    action_ev_bb: &[f32],
    frequencies: &[f32],
    chosen_idx: usize,
    category: &str,
    hero_hand: (Card, Card),
    board: &[Card],
    snapshot: &Value,
) -> anyhow::Result<GradeResult> {
    if actions.is_empty() || action_ev_bb.len() != actions.len() {
        anyhow::bail!("grading requires one EV value per available action");
    }
    if chosen_idx >= actions.len() {
        anyhow::bail!("chosen_idx {chosen_idx} is out of range for this spot's {} available actions", actions.len());
    }
    let (best_idx, _) = action_ev_bb
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    let chosen_ev_bb = action_ev_bb[chosen_idx];
    let best_ev_bb = action_ev_bb[best_idx];
    let ev_loss_bb = (best_ev_bb - chosen_ev_bb).max(0.0);
    let chosen_freq = frequencies.get(chosen_idx).copied().unwrap_or(0.0);
    let is_correct = ev_loss_bb <= EV_TOLERANCE_BB || chosen_freq >= FREQUENCY_TOLERANCE;

    let ctx = Context {
        category,
        hero_hand,
        board,
        chosen: actions[chosen_idx],
        best: actions[best_idx],
        ev_loss_bb,
        snapshot,
    };
    let explanation = build_explanation(&ctx, is_correct);

    Ok(GradeResult {
        is_correct,
        chosen_action: actions[chosen_idx].label(),
        best_action: actions[best_idx].label(),
        chosen_ev_bb,
        best_ev_bb,
        ev_loss_bb,
        explanation,
    })
}

struct Context<'a> {
    category: &'a str,
    hero_hand: (Card, Card),
    board: &'a [Card],
    chosen: Action,
    best: Action,
    ev_loss_bb: f32,
    snapshot: &'a Value,
}

impl Context<'_> {
    fn f(&self, key: &str) -> Option<f32> {
        self.snapshot.get(key).and_then(|v| v.as_f64()).map(|v| v as f32)
    }
    fn s(&self, key: &str) -> Option<&str> {
        self.snapshot.get(key).and_then(|v| v.as_str())
    }
    fn hero_in_position(&self) -> bool {
        // Postflop snapshots don't all set this explicitly; default to
        // "out of position" (the more common, more cautious assumption)
        // when the field is absent.
        self.snapshot.get("hero_in_position").and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

fn build_explanation(ctx: &Context, is_correct: bool) -> String {
    if is_correct {
        return format!(
            "Good read -- {} is right in line with the solver's strategy here (EV loss under {:.2}bb).",
            ctx.chosen.label(),
            EV_TOLERANCE_BB
        );
    }

    let mut reasons: Vec<(u8, String)> = Vec::new();
    for rule in RULES {
        if let Some(r) = rule(ctx) {
            reasons.push(r);
        }
    }
    reasons.sort_by(|a, b| b.0.cmp(&a.0));
    reasons.truncate(2);

    let headline = format!(
        "You chose {} over {}, giving up about {:.2}bb in expectation.",
        ctx.chosen.label(),
        ctx.best.label(),
        ctx.ev_loss_bb
    );

    let body = if reasons.is_empty() {
        format!(
            "The solver's mix simply favors {} more often with this exact hand in this spot -- it's a close enough decision that the gap mostly comes down to small equity/blocker differences rather than one big strategic idea.",
            ctx.best.label()
        )
    } else {
        reasons.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join(" ")
    };

    let tip = closing_tip(ctx);

    format!("{headline} {body} {tip}")
}

type Rule = fn(&Context) -> Option<(u8, String)>;

const RULES: &[Rule] = &[
    rule_fold_equity,
    rule_pot_odds,
    rule_equity_realization,
    rule_blockers,
    rule_position,
    rule_stack_depth,
    rule_board_texture,
    rule_range_polarization,
];

/// Missed fold equity: passive action (check/call) chosen when an
/// aggressive one (bet/raise/all-in) was correct. Betting doesn't only
/// realize your hand's showdown value -- it also wins the pot outright
/// whenever the opponent folds, and that extra equity is easy to undervalue
/// if you're only thinking in terms of "is my hand good enough to bet".
fn rule_fold_equity(ctx: &Context) -> Option<(u8, String)> {
    if ctx.best.is_aggressive() && !ctx.chosen.is_aggressive() {
        let opponent = ctx.s("villain_position").map(|p| format!("the {p}")).unwrap_or_else(|| "your opponent".to_string());
        Some((90, format!(
            "{} doesn't just rely on having the best hand right now -- it also wins the pot immediately whenever {} folds. That 'fold equity' is real, additional value that {} simply can't capture, which is why the aggressive line is worth more here even though your hand isn't always ahead.",
            ctx.best.label(), opponent, ctx.chosen.label()
        )))
    } else {
        None
    }
}

/// Missed pot odds: folded to a bet when calling (or raising) was correct.
fn rule_pot_odds(ctx: &Context) -> Option<(u8, String)> {
    if !matches!(ctx.chosen, Action::Fold) || matches!(ctx.best, Action::Fold) {
        return None;
    }
    let pot = ctx.f("pot_bb").or_else(|| ctx.f("starting_pot_bb"));
    let facing = ctx.f("facing_bb");
    if let (Some(pot), Some(facing)) = (pot, facing) {
        if facing > 0.0 {
            let required_equity = 100.0 * facing / (pot + facing);
            return Some((85, format!(
                "Facing a {facing:.1}bb bet into a {pot:.1}bb pot, you're only risking {facing:.1} to win {total:.1}, so you need roughly {required_equity:.0}% equity for a call to break even on its own -- and this hand clears that bar (plus has extra value from implied odds and the times your opponent gives up later). Folding throws away that edge.",
                facing = facing, pot = pot, total = pot + facing, required_equity = required_equity
            )));
        }
    }
    Some((60, "Folding here gives up a profitable price -- the amount you'd need to call is small relative to what's already in the pot, so continuing shows a positive return even without a strong hand.".to_string()))
}

/// Missed reverse implied odds / poor equity realization: called or raised
/// when folding was correct, typically because the hand can't comfortably
/// continue across future streets even though it has some raw equity.
fn rule_equity_realization(ctx: &Context) -> Option<(u8, String)> {
    if matches!(ctx.best, Action::Fold) && !matches!(ctx.chosen, Action::Fold) {
        let oop_note = if !ctx.hero_in_position() && !ctx.board.is_empty() {
            " and you're out of position, which makes it harder to control the size of the pot or see a cheap showdown"
        } else {
            ""
        };
        return Some((80, format!(
            "This hand has some raw equity, but it doesn't realize that equity well in practice{oop_note}. Hands like this often end up calling another bet (or two) before finding out they were behind, which costs more in expectation than the pot odds on this single street suggest -- that's the 'reverse implied odds' problem, and it's why folding beats just looking at whether you're getting a price."
        )));
    }
    None
}

/// Blockers: holding a card that removes combos from the opponent's
/// continuing range makes an aggressive/bluffing line more effective
/// (preflop: blocking AA/AK; postflop on flush/straight-heavy boards:
/// blocking the nut draw or made hand).
fn rule_blockers(ctx: &Context) -> Option<(u8, String)> {
    if !ctx.best.is_aggressive() {
        return None;
    }
    let (a, b) = ctx.hero_hand;
    let is_preflop_aggro_category = matches!(ctx.category, "vs_open_defend" | "vs_three_bet" | "open_raise");
    if is_preflop_aggro_category && (a.rank == Rank::Ace || b.rank == Rank::Ace) && !ctx.chosen.is_aggressive() {
        return Some((70, "Holding an ace also removes one of the four combinations of AA and several combinations of AK/AQ from your opponent's range, since they can't hold a card you already have. That blocker effect makes your raise more effective as a bluff/semi-bluff -- fewer of their strongest continuing hands are actually possible.".to_string()));
    }
    if !ctx.board.is_empty() {
        let texture = Board { cards: ctx.board.to_vec() }.texture();
        if texture.flush_draw || texture.monotone {
            let flush_suit = ctx.board.iter().map(|c| c.suit).next();
            if let Some(suit) = flush_suit {
                if a.suit == suit || b.suit == suit {
                    return Some((65, "You also hold a card of the flush suit, which blocks some of the strongest flush combinations your opponent could be continuing with. That makes an aggressive line safer and more profitable than it would be without that blocker, since you're less likely to run into the exact hand that snaps it off.".to_string()));
                }
            }
        }
    }
    None
}

/// Position: in-position hands realize equity more reliably and support a
/// wider aggressive range; out-of-position hands need to be more selective.
fn rule_position(ctx: &Context) -> Option<(u8, String)> {
    if ctx.board.is_empty() {
        return None; // position matters less for the preflop decision itself
    }
    if ctx.best.is_aggressive() && ctx.hero_in_position() && !ctx.chosen.is_aggressive() {
        return Some((55, "Being in position means you get to see your opponent act first on every remaining street, which lets you realize a hand's value (or apply pressure) more reliably -- that's part of why the more aggressive line is favored here.".to_string()));
    }
    if !ctx.best.is_aggressive() && !ctx.hero_in_position() && ctx.chosen.is_aggressive() {
        return Some((55, "Out of position, betting commits more chips before you get to see how your opponent reacts, and you'll have less information on future streets if they continue. That extra risk is why a more controlled line holds up better here than betting does.".to_string()));
    }
    None
}

/// Stack depth / SPR: short stacks compress decisions toward all-in-or-fold
/// (small bets accomplish little when nobody can fold profitably to a
/// follow-up); deep stacks add implied-odds value to speculative hands.
fn rule_stack_depth(ctx: &Context) -> Option<(u8, String)> {
    let stack = ctx.f("stack_bb")?;
    if stack <= 25.0 && matches!(ctx.best, Action::AllIn(_) | Action::Raise(_)) {
        return Some((50, format!(
            "With only about {stack:.0}bb behind, there isn't much room for a smaller sizing to accomplish anything -- a bigger, more committing action removes your opponent's ability to make a cheap, well-timed exploit of a tentative line."
        )));
    }
    if stack >= 100.0 && matches!(ctx.best, Action::Call) && !ctx.board.is_empty() {
        return Some((45, "With this much behind, speculative or drawing hands pick up real extra value from implied odds -- the chance to win a much bigger pot on a later street if you improve -- which is worth more than its raw equity suggests on this street alone.".to_string()));
    }
    None
}

/// Board texture: wet/dynamic boards (lots of live draws, connected,
/// two-tone) reward betting for protection/denial; dry/static boards
/// reduce the urgency to bet since there's less for a checked hand to lose
/// to.
fn rule_board_texture(ctx: &Context) -> Option<(u8, String)> {
    if ctx.board.is_empty() {
        return None;
    }
    let t = Board { cards: ctx.board.to_vec() }.texture();
    let wet = t.connected || t.flush_draw || t.monotone;
    if wet && ctx.best.is_aggressive() && !ctx.chosen.is_aggressive() {
        return Some((40, "This board has plenty of live draws on it. Checking lets those draws see another card for free, while betting now charges them a price (or makes them fold outright) -- that 'equity denial' is worth more on textures like this than on a dry board.".to_string()));
    }
    if !wet && !ctx.best.is_aggressive() && ctx.chosen.is_aggressive() {
        return Some((40, "This board is fairly dry and unlikely to change much of strategic relevance on the next card, so there's less urgency to bet for protection -- a more controlled, pot-sized-down approach loses less value here than it would on a wetter board.".to_string()));
    }
    None
}

/// Range polarization: in raised preflop pots (3-bet/4-bet), correct
/// strategies are usually polarized between strong value and a smaller
/// number of bluffs/blockers rather than a smooth, linear range.
fn rule_range_polarization(ctx: &Context) -> Option<(u8, String)> {
    if !matches!(ctx.category, "vs_three_bet") {
        return None;
    }
    if matches!(ctx.best, Action::AllIn(_) | Action::Raise(_)) {
        return Some((35, "In 3-bet pots, the strategy that holds up best is usually polarized -- a mix of very strong value hands and a smaller number of well-chosen bluffs/blockers -- rather than calling everything in between. This hand fits better on the raising side of that split than in a flatter calling range.".to_string()));
    }
    None
}

fn closing_tip(ctx: &Context) -> String {
    let category_tip = match ctx.category {
        "open_raise" => "Next time, anchor the open/fold decision to your position and stack depth first, then double-check for blockers before talking yourself out of a borderline raise.",
        "vs_open_defend" => "Next time, weigh the price you're getting against how well this hand plays across multiple streets, not just whether it's 'good enough' in isolation.",
        "vs_three_bet" => "Next time, think about which side of a polarized range this hand belongs on (strong value vs. a blocker-heavy bluff) rather than treating every continue as a simple call.",
        "cbet_flop" => "Next time, factor in board texture and position before defaulting to a single 'standard' c-bet size -- wetter boards and being in position both push toward more aggression.",
        "vs_cbet_flop" => "Next time, run the pot-odds math explicitly (call amount vs. resulting pot) before folding, and watch for blockers that make continuing (or raising) better than it looks at first glance.",
        "turn_barrel" => "Next time, ask whether this specific turn card actually improved your range relative to villain's before firing a second barrel -- a 'standard' continuation bet isn't standard if the card helped them more than you.",
        "vs_turn_barrel" => "Next time, reassess your hand's continuing equity on the new card rather than anchoring to how good it looked on the flop -- a turn barrel often signals a narrower, stronger range than a flop bet did.",
        "river_bet" => "Next time, think in terms of a polarized value/bluff split for your river betting range, not 'do I have a hand' -- thin value and well-chosen bluffs both belong in a good river-betting range, medium-strength hands usually don't.",
        "vs_river_bet" => "Next time, weigh exact pot odds against how often this specific river bet looks like value vs. a bluff in villain's spot, rather than defaulting to how strong your hand feels in isolation.",
        _ => "Next time, weigh fold equity, pot odds, and position together rather than any single factor in isolation.",
    };
    format!("Try this: {category_tip}")
}
