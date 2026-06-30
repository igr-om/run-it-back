use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use rib_core::{Card, Street};

use crate::common::{assign_positions, parse_card_list, parse_money};
use crate::model::{ActionKind, ParseHandHistoryError, ParsedAction, ParsedHand};
use crate::site_parser::SiteParser;

// PartyPoker: "***** Hand History for Game 123456789 *****" / "Table ... 6 max"
// WPN (Bovada/Ignition/ACR): "Game #123456789: Hold'em No Limit ($0.25/$0.50) - ..."
// or "Ignition Hand #123: ..." -- both close enough to handle with one regex
// set tolerant of either casing/wording.
static HEADER_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)(?:Game|Hand)\s*#?\s*:?\s*(?P<id>\d+)").unwrap());
static STAKES: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\(?\$?(?P<sb>[\d.,]+)\s*/\s*\$?(?P<bb>[\d.,]+)\)?.*?(Hold'em|Omaha)").unwrap()
});
static TABLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Table\s+.*?(?P<size>\d+)[\s-]*max").unwrap());
static BTN_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)Seat\s+#?(?P<seat>\d+)\s+is\s+the\s+button").unwrap());
static SEAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Seat\s+(?P<seat>\d+):\s+(?P<name>.+?)\s+\(\$?[\d.,]+(?:\s+in chips)?\)").unwrap());
// Both "PlayerA: posts small blind $0.25" (PokerStars-ish) and
// "PlayerA: Small Blind $0.25" / "Hero posts small blind [$0.25]" variants.
static POSTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<name>.+?):?\s+(?:posts\s+)?(?P<kind>small blind|big blind|ante)\s+\[?\$?(?P<amt>[\d.,]+)\]?").unwrap()
});
static DEALT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Dealt to (?P<name>.+?)\s*\[\s*(?P<cards>.+?)\s*\]").unwrap());
static STREET_HDR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\*\*\*\s*(FLOP|TURN|RIVER)\s*\*\*\*\s*(?:\[.*?\]\s*)?\[(?P<cards>.+?)\]").unwrap());
static ACTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<name>.+?):\s+(?P<verb>Folds|folds|Checks|checks|Calls|calls|Bets|bets|Raises|raises)(?:\s+\$?(?P<amt>[\d.,]+))?(?:\s+to\s+\$?(?P<to>[\d.,]+))?").unwrap()
});
static SHOWS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(?P<name>.+?):\s+[Ss]hows\s*\[\s*(?P<cards>.+?)\s*\]").unwrap());
static COLLECTED: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(?P<name>.+?)\s+collected\s+\$?(?P<amt>[\d.,]+)\s+from").unwrap());

pub struct PartyPokerParser;
pub struct WpnParser;

impl SiteParser for PartyPokerParser {
    fn site_name(&self) -> &'static str {
        "partypoker"
    }
    fn sniff(&self, raw: &str) -> bool {
        raw.lines().take(6).any(|l| l.to_ascii_lowercase().contains("partypoker") || l.contains("***** Hand History"))
    }
    fn parse_hand(&self, block: &str) -> Result<ParsedHand, ParseHandHistoryError> {
        parse_wpn_family_hand(block, "partypoker")
    }
}

impl SiteParser for WpnParser {
    fn site_name(&self) -> &'static str {
        "wpn"
    }
    fn sniff(&self, raw: &str) -> bool {
        raw.lines().take(6).any(|l| {
            let lower = l.to_ascii_lowercase();
            lower.contains("ignition") || lower.contains("bovada") || lower.contains("americas cardroom") || lower.contains("acr ")
        })
    }
    fn parse_hand(&self, block: &str) -> Result<ParsedHand, ParseHandHistoryError> {
        parse_wpn_family_hand(block, "wpn")
    }
}

/// PartyPoker and WPN-skin (Bovada/Ignition/Americas Cardroom) hand
/// histories vary more from each other -- and across software versions --
/// than the other sites' exports do, since WPN in particular has been
/// re-skinned and re-licensed many times over the years. This parser is
/// deliberately tolerant: it never panics on an unrecognized line, simply
/// skipping it, so a partially-unfamiliar export still yields a best-effort
/// `ParsedHand` for every field it *could* find rather than failing the
/// whole hand. If you hit a file this consistently mis-parses, please open
/// an issue with a redacted sample -- see the README's parser section.
fn parse_wpn_family_hand(block: &str, site: &str) -> Result<ParsedHand, ParseHandHistoryError> {
    let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Err(ParseHandHistoryError::Malformed("empty hand block".into()));
    }

    let site_hand_id = lines.iter().find_map(|l| HEADER_ID.captures(l)).map(|c| c["id"].to_string());
    let stakes = lines.iter().take(3).find_map(|l| STAKES.captures(l));
    let game_type = match &stakes {
        Some(c) if c[3].eq_ignore_ascii_case("omaha") => "plo",
        _ => "nlhe",
    };
    let big_blind_amount = stakes.as_ref().and_then(|c| parse_money(&c["bb"])).unwrap_or(1.0);

    let mut table_size: u8 = 6;
    let mut button_seat: u8 = 1;
    let mut seats: HashMap<u8, String> = HashMap::new();
    let mut hero_name: Option<String> = None;
    let mut hero_seat: Option<u8> = None;
    let mut hero_cards: Vec<Card> = Vec::new();
    let mut board: Vec<Card> = Vec::new();
    let mut street = Street::Preflop;
    let mut actions: Vec<ParsedAction> = Vec::new();
    let mut winners: Vec<(String, f64)> = Vec::new();
    let mut showdown_names: Vec<String> = Vec::new();

    for line in &lines {
        if let Some(c) = TABLE.captures(line) {
            table_size = c["size"].parse().unwrap_or(table_size);
            continue;
        }
        if let Some(c) = BTN_LINE.captures(line) {
            button_seat = c["seat"].parse().unwrap_or(button_seat);
            continue;
        }
        if let Some(c) = SEAT.captures(line) {
            let seat: u8 = c["seat"].parse().unwrap_or(0);
            let name = c["name"].trim().to_string();
            if name.eq_ignore_ascii_case("hero") {
                hero_name = Some(name.clone());
                hero_seat = Some(seat);
            }
            seats.insert(seat, name);
            continue;
        }
        if let Some(c) = STREET_HDR.captures(line) {
            street = match &c[1].to_ascii_uppercase()[..] {
                "FLOP" => Street::Flop,
                "TURN" => Street::Turn,
                "RIVER" => Street::River,
                _ => street,
            };
            for card in parse_card_list(&c["cards"]) {
                if !board.contains(&card) {
                    board.push(card);
                }
            }
            continue;
        }
        if let Some(c) = POSTS.captures(line) {
            let name = c["name"].trim().to_string();
            let amt = parse_money(&c["amt"]).unwrap_or(0.0);
            let kind = match c["kind"].to_ascii_lowercase().as_str() {
                "small blind" => ActionKind::PostSmallBlind,
                "big blind" => ActionKind::PostBigBlind,
                _ => ActionKind::PostAnte,
            };
            actions.push(ParsedAction {
                seat: seat_of(&seats, &name),
                player_name: name.clone(),
                is_hero: is_hero(&hero_name, &name),
                street,
                kind,
                amount_bb: Some(amt / big_blind_amount),
                position: None,
            });
            continue;
        }
        if let Some(c) = DEALT.captures(line) {
            if c["name"].eq_ignore_ascii_case("hero") {
                hero_cards = parse_card_list(&c["cards"]);
            }
            continue;
        }
        if let Some(c) = SHOWS.captures(line) {
            showdown_names.push(c["name"].trim().to_string());
            continue;
        }
        if let Some(c) = COLLECTED.captures(line) {
            winners.push((c["name"].trim().to_string(), parse_money(&c["amt"]).unwrap_or(0.0)));
            continue;
        }
        if let Some(c) = ACTION.captures(line) {
            let name = c["name"].trim().to_string();
            let kind = match c["verb"].to_ascii_lowercase().as_str() {
                "folds" => ActionKind::Fold,
                "checks" => ActionKind::Check,
                "calls" => ActionKind::Call,
                "bets" => ActionKind::Bet,
                "raises" => ActionKind::Raise,
                _ => continue,
            };
            let total = c.name("to").and_then(|m| parse_money(m.as_str())).or_else(|| c.name("amt").and_then(|m| parse_money(m.as_str())));
            actions.push(ParsedAction {
                seat: seat_of(&seats, &name),
                player_name: name.clone(),
                is_hero: is_hero(&hero_name, &name),
                street,
                kind,
                amount_bb: total.map(|t| t / big_blind_amount),
                position: None,
            });
            continue;
        }
    }

    let mut active: Vec<u8> = seats.keys().copied().collect();
    active.sort_unstable();
    if !active.is_empty() {
        table_size = table_size.max(active.len() as u8);
    }
    let position_map = assign_positions(&active, button_seat);
    for a in &mut actions {
        a.position = position_map.get(&a.seat).copied();
    }
    let hero_position = hero_seat.and_then(|s| position_map.get(&s).copied());

    let invested = super::pokerstars::total_hero_invested(&actions);
    let won: f64 = hero_name
        .as_ref()
        .map(|hn| winners.iter().filter(|(n, _)| n == hn).map(|(_, a)| a / big_blind_amount).sum())
        .unwrap_or(0.0);
    let went_to_showdown = hero_name.as_ref().map(|hn| showdown_names.iter().any(|n| n == hn)).unwrap_or(false);
    let won_hand = hero_name.as_ref().map(|hn| winners.iter().any(|(n, _)| n == hn)).unwrap_or(false);

    Ok(ParsedHand {
        site: site.to_string(),
        site_hand_id,
        game_type: game_type.to_string(),
        table_size,
        big_blind_amount,
        hero_seat,
        hero_position,
        hero_cards,
        board,
        played_at: None,
        actions,
        result_bb: won - invested,
        went_to_showdown,
        won_hand,
        tags: Vec::new(),
    })
}

fn seat_of(seats: &HashMap<u8, String>, name: &str) -> u8 {
    seats.iter().find(|(_, n)| n.as_str() == name).map(|(s, _)| *s).unwrap_or(0)
}

fn is_hero(hero_name: &Option<String>, name: &str) -> bool {
    hero_name.as_deref().map(|h| h == name).unwrap_or(false) || name.eq_ignore_ascii_case("hero")
}
