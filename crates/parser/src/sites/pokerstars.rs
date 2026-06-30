//! PokerStars `.txt` hand history export. This is the most stable,
//! well-documented format among the major sites and most other rooms'
//! exports (GGPoker, ACR) are structurally close to it, which is why this
//! module's helper regexes get reused/adapted by `ggpoker.rs` and `wpn.rs`.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use rib_core::{Card, Street};

use crate::common::{assign_positions, parse_card_list, parse_money};
use crate::model::{ActionKind, ParseHandHistoryError, ParsedAction, ParsedHand};
use crate::site_parser::SiteParser;

static HEADER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^PokerStars\s+Hand\s+#(?P<id>\d+):\s+(?P<game>Hold'em|Omaha)\s+(?:No\s+Limit|Pot\s+Limit|Limit)\s+\(\$?(?P<sb>[\d.,]+)/\$?(?P<bb>[\d.,]+)").unwrap()
});
static TABLE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Table\s+'.*?'\s+(?P<size>\d+)-max(?:\s+Seat\s+#(?P<btn>\d+)\s+is\s+the\s+button)?").unwrap());
static SEAT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Seat\s+(?P<seat>\d+):\s+(?P<name>.+?)\s+\(\$?[\d.,]+\s+in chips\)").unwrap());
static POSTS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<name>.+?):\s+posts\s+(?P<kind>small blind|big blind|ante)\s+\$?(?P<amt>[\d.,]+)").unwrap()
});
static DEALT: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Dealt to (?P<name>.+?) \[(?P<cards>.+?)\]").unwrap());
static ACTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^(?P<name>.+?):\s+(?P<verb>folds|checks|calls|bets|raises)(?:\s+\$?(?P<amt>[\d.,]+))?(?:\s+to\s+\$?(?P<to>[\d.,]+))?").unwrap()
});
static STREET_HDR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\*\*\*\s*(FLOP|TURN|RIVER)\s*\*\*\*\s*(?:\[.*?\]\s*)?\[(?P<cards>.+?)\]").unwrap());
static SUMMARY_BOARD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^Board\s+\[(?P<cards>.+?)\]").unwrap());
static COLLECTED: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(?P<name>.+?)\s+collected\s+\$?(?P<amt>[\d.,]+)\s+from").unwrap());
static SHOWS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^(?P<name>.+?):\s+shows\s+\[(?P<cards>.+?)\]").unwrap());

pub struct PokerStarsParser;

impl SiteParser for PokerStarsParser {
    fn site_name(&self) -> &'static str {
        "pokerstars"
    }

    fn sniff(&self, raw: &str) -> bool {
        raw.lines().take(5).any(|l| l.to_ascii_lowercase().contains("pokerstars hand #"))
    }

    fn parse_hand(&self, block: &str) -> Result<ParsedHand, ParseHandHistoryError> {
        parse_pokerstars_family_hand(block, "pokerstars", &HEADER)
    }
}

/// Shared by PokerStars and GGPoker, whose exports are structurally
/// identical aside from the header line and the room name in it. The header
/// regex must expose named capture groups `game` ("Hold'em"/"Omaha"), `bb`
/// (big blind amount), and optionally `id` (site hand id).
pub fn parse_pokerstars_family_hand(
    block: &str,
    site: &str,
    header_re: &Regex,
) -> Result<ParsedHand, ParseHandHistoryError> {
    let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return Err(ParseHandHistoryError::Malformed("empty hand block".into()));
    }

    let header = header_re.captures(lines[0]).ok_or_else(|| {
        ParseHandHistoryError::Malformed(format!("unrecognized header line: {}", lines[0]))
    })?;
    let game_type = if header.name("game").map(|g| g.as_str().eq_ignore_ascii_case("omaha")).unwrap_or(false) {
        "plo"
    } else {
        "nlhe"
    };
    let site_hand_id = header.name("id").map(|m| m.as_str().to_string());
    let big_blind_amount = header.name("bb").and_then(|m| parse_money(m.as_str())).unwrap_or(1.0);

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
    let mut in_summary = false;

    for line in &lines[1..] {
        if let Some(c) = TABLE.captures(line) {
            if let Some(size) = c.name("size").and_then(|m| m.as_str().parse::<u8>().ok()) {
                table_size = size;
            }
            if let Some(btn) = c.name("btn").and_then(|m| m.as_str().parse::<u8>().ok()) {
                button_seat = btn;
            }
            continue;
        }
        if let Some(c) = SEAT.captures(line) {
            let seat = c.name("seat").unwrap().as_str().parse::<u8>().unwrap_or(0);
            let name = c.name("name").unwrap().as_str().to_string();
            if name.eq_ignore_ascii_case("hero") {
                hero_name = Some(name.clone());
                hero_seat = Some(seat);
            }
            seats.insert(seat, name);
            continue;
        }
        if line.starts_with("*** HOLE CARDS") {
            street = Street::Preflop;
            continue;
        }
        if line.starts_with("*** SUMMARY") {
            in_summary = true;
            continue;
        }
        if let Some(c) = STREET_HDR.captures(line) {
            let label = &c[1];
            street = match label {
                "FLOP" => Street::Flop,
                "TURN" => Street::Turn,
                "RIVER" => Street::River,
                _ => street,
            };
            // FLOP shows all 3 cards; TURN/RIVER show full board then just
            // the new card in a trailing bracket -- recompute from scratch
            // each time using the *last* bracket group to be safe.
            let cards = parse_card_list(&c["cards"]);
            for card in cards {
                if !board.contains(&card) {
                    board.push(card);
                }
            }
            continue;
        }
        if in_summary {
            if let Some(c) = SUMMARY_BOARD.captures(line) {
                board = parse_card_list(&c["cards"]);
            }
            continue; // summary lines otherwise only restate info we already captured live
        }
        if let Some(c) = POSTS.captures(line) {
            let name = c.name("name").unwrap().as_str().to_string();
            let amt = c.name("amt").and_then(|m| parse_money(m.as_str())).unwrap_or(0.0);
            let kind = match c.name("kind").unwrap().as_str().to_ascii_lowercase().as_str() {
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
            if c.name("name").unwrap().as_str().eq_ignore_ascii_case("hero") {
                hero_cards = parse_card_list(&c["cards"]);
            }
            continue;
        }
        if let Some(c) = SHOWS.captures(line) {
            showdown_names.push(c.name("name").unwrap().as_str().to_string());
            continue;
        }
        if let Some(c) = COLLECTED.captures(line) {
            let name = c.name("name").unwrap().as_str().to_string();
            let amt = c.name("amt").and_then(|m| parse_money(m.as_str())).unwrap_or(0.0);
            winners.push((name, amt));
            continue;
        }
        if let Some(c) = ACTION.captures(line) {
            let name = c.name("name").unwrap().as_str().to_string();
            let verb = c.name("verb").unwrap().as_str().to_ascii_lowercase();
            let kind = match verb.as_str() {
                "folds" => ActionKind::Fold,
                "checks" => ActionKind::Check,
                "calls" => ActionKind::Call,
                "bets" => ActionKind::Bet,
                "raises" => ActionKind::Raise,
                _ => continue,
            };
            // "raises $2.00 to $6.00" -> the *total* this street is the `to`
            // amount; "bets $3.00" / "calls $3.00" -> the amount itself.
            let total = c
                .name("to")
                .and_then(|m| parse_money(m.as_str()))
                .or_else(|| c.name("amt").and_then(|m| parse_money(m.as_str())));
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
        // Unrecognized line (e.g. chat, "doesn't show", uncalled-bet
        // returns): silently skipped, since none of these change the
        // canonical action sequence in a way the stats/drill engine needs.
    }

    let active_seats: Vec<u8> = {
        let mut v: Vec<u8> = seats.keys().copied().collect();
        v.sort_unstable();
        v
    };
    let position_map = assign_positions(&active_seats, button_seat);
    for a in &mut actions {
        a.position = position_map.get(&a.seat).copied();
    }
    let hero_position = hero_seat.and_then(|s| position_map.get(&s).copied());

    let hero_net: f64 = {
        let invested = total_hero_invested(&actions);
        let won: f64 = hero_name
            .as_ref()
            .map(|hn| winners.iter().filter(|(n, _)| n == hn).map(|(_, a)| a / big_blind_amount).sum())
            .unwrap_or(0.0);
        won - invested
    };

    let went_to_showdown = hero_name
        .as_ref()
        .map(|hn| showdown_names.iter().any(|n| n == hn))
        .unwrap_or(false);
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
        result_bb: hero_net,
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

/// Hero's total chips committed across the whole hand. Each action's
/// `amount_bb` is already "total this player has put in *this street* as
/// of this action" (raises/calls report cumulative "to" amounts, not
/// deltas), so a player's final commitment on a given street is the *max*
/// (equivalently last) amount among their own actions on that street, and
/// the hand total is the sum of those per-street maxes.
pub fn total_hero_invested(actions: &[ParsedAction]) -> f64 {
    let mut per_street: HashMap<Street, f64> = HashMap::new();
    for a in actions.iter().filter(|a| a.is_hero) {
        if let Some(amt) = a.amount_bb {
            let entry = per_street.entry(a.street).or_insert(0.0);
            if amt > *entry {
                *entry = amt;
            }
        }
    }
    per_street.values().sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OMAHA_HAND: &str = r#"PokerStars Hand #123456789: Omaha Pot Limit ($0.25/$0.50 USD) - 2024/01/15 21:32:11 ET
Table 'Atlas IV' 6-max Seat #3 is the button
Seat 1: PlayerA ($50.00 in chips)
Seat 2: PlayerB ($48.50 in chips)
Seat 3: Hero ($52.25 in chips)
PlayerA: posts small blind $0.25
PlayerB: posts big blind $0.50
*** HOLE CARDS ***
Dealt to Hero [Ah Kh Qd Jc]
PlayerA: folds
Hero: raises $1.25 to $1.75
PlayerB: calls $1.25
*** FLOP *** [7h 8d 2c]
PlayerB: checks
Hero: bets $2.50
PlayerB: folds
Uncalled bet ($2.50) returned to Hero
Hero collected $3.75 from pot
*** SUMMARY ***
Total pot $3.75 | Rake $0
Board [7h 8d 2c]
Seat 3: Hero (button) collected ($3.75)"#;

    const HOLDEM_HAND: &str = r#"PokerStars Hand #123456790: Hold'em No Limit ($0.25/$0.50 USD) - 2024/01/15 21:33:11 ET
Table 'Atlas IV' 6-max Seat #3 is the button
Seat 1: PlayerA ($50.00 in chips)
Seat 2: PlayerB ($48.50 in chips)
Seat 3: Hero ($52.25 in chips)
PlayerA: posts small blind $0.25
PlayerB: posts big blind $0.50
*** HOLE CARDS ***
Dealt to Hero [Ah Kh]
PlayerA: folds
Hero: raises $1.25 to $1.75
PlayerB: folds
Uncalled bet ($1.25) returned to Hero
Hero collected $1.00 from pot"#;

    #[test]
    fn parses_omaha_hand_with_four_hole_cards() {
        let parser = PokerStarsParser;
        let hand = parser.parse_hand(OMAHA_HAND).expect("Omaha hand should parse, not be rejected");
        assert_eq!(hand.game_type, "plo");
        assert_eq!(hand.hero_cards.len(), 4);
        assert_eq!(hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>(), vec!["Ah", "Kh", "Qd", "Jc"]);
        assert_eq!(hand.board.len(), 3);
    }

    #[test]
    fn holdem_hand_still_parses_as_nlhe() {
        let parser = PokerStarsParser;
        let hand = parser.parse_hand(HOLDEM_HAND).expect("hold'em hand should parse");
        assert_eq!(hand.game_type, "nlhe");
        assert_eq!(hand.hero_cards.len(), 2);
    }

    #[test]
    fn full_file_parse_detects_and_tags_omaha() {
        let outcome = crate::parse_and_tag(OMAHA_HAND).expect("file-level parse should succeed for an Omaha hand");
        assert_eq!(outcome.site, "pokerstars");
        assert_eq!(outcome.hands.len(), 1);
        assert_eq!(outcome.hands[0].game_type, "plo");
    }

    #[test]
    fn windows_line_endings_still_split_multiple_hands() {
        // Most poker clients run on Windows and export hand histories with
        // \r\n line endings. \r\n\r\n does not contain the literal "\n\n"
        // the default blank-line hand splitter looks for -- this test
        // pins the fix in `detect::parse_file` that normalizes line
        // endings before splitting.
        let crlf_file = format!("{}\r\n\r\n{}", HOLDEM_HAND.replace('\n', "\r\n"), OMAHA_HAND.replace('\n', "\r\n"));
        let outcome = crate::parse_and_tag(&crlf_file).expect("CRLF multi-hand file should parse");
        assert_eq!(outcome.total_blocks, 2, "should split into 2 separate hand blocks, not 1 giant block");
        assert_eq!(outcome.hands.len(), 2);
        assert_eq!(outcome.hands[0].game_type, "nlhe");
        assert_eq!(outcome.hands[1].game_type, "plo");
    }
}
