use std::str::FromStr;

use rib_core::{Card, Position};

/// Parses any of "$1.25", "1.25", "1,25" (some EU clients use comma decimal
/// separators), "[$1.25]", or "0.25" into a plain f64. Returns `None` rather
/// than erroring so callers can decide whether a missing amount on a given
/// line is fatal or just means "no amount on this line" (e.g. a fold).
pub fn parse_money(s: &str) -> Option<f64> {
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-')
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    // Heuristic: if there's exactly one comma and no dot, treat comma as the
    // decimal separator (EU-style); otherwise commas are thousands
    // separators and get stripped.
    let normalized = if cleaned.matches(',').count() == 1 && !cleaned.contains('.') {
        cleaned.replace(',', ".")
    } else {
        cleaned.replace(',', "")
    };
    normalized.parse::<f64>().ok()
}

/// Parses a bracketed/space/comma separated card list, e.g. "[Ah Kd]",
/// "[ Ah, Kd ]", "Ah Kd 7s", into `Vec<Card>`. Unparseable individual tokens
/// are skipped rather than failing the whole line, since a malformed single
/// card shouldn't sink an otherwise-good hand.
pub fn parse_card_list(s: &str) -> Vec<Card> {
    s.trim_matches(|c: char| c == '[' || c == ']')
        .split([' ', ','])
        .map(str::trim)
        .filter(|t| t.len() == 2)
        .filter_map(|t| Card::from_str(t).ok())
        .collect()
}

/// Assigns a named `Position` to every seat at the table, given which seat
/// has the button. `active_seats` should be every seat that's dealt in
/// (sorted ascending); rotates the table so the button lands last, matching
/// the convention `Position::for_table_size` returns (`[..., Btn, Sb, Bb]`
/// for 3+ handed, `[Btn, Bb]` heads-up).
pub fn assign_positions(active_seats: &[u8], button_seat: u8) -> std::collections::HashMap<u8, Position> {
    let n = active_seats.len();
    let mut map = std::collections::HashMap::new();
    if n == 0 {
        return map;
    }
    let btn_idx = active_seats.iter().position(|s| *s == button_seat).unwrap_or(0);
    let labels = Position::for_table_size(n);
    // `labels` is preflop acting order, e.g. for 6-max: [Utg, Hj, Co, Btn, Sb, Bb]
    // -- note Btn is third-from-last, not last, since SB/BB still act after
    // it preflop. Heads-up has no separate SB (button posts small blind and
    // acts first), so its rotation starts at the button itself instead of
    // three seats after it.
    let start_offset: usize = if n == 2 { 0 } else { 3 };
    let rotated: Vec<u8> = (0..n).map(|i| active_seats[(btn_idx + start_offset + i) % n]).collect();
    for (seat, label) in rotated.into_iter().zip(labels.into_iter()) {
        map.insert(seat, label);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_basic() {
        assert_eq!(parse_money("$1.25"), Some(1.25));
        assert_eq!(parse_money("[$0.50]"), Some(0.50));
        assert_eq!(parse_money("1,234.56"), Some(1234.56));
    }

    #[test]
    fn cards_basic() {
        assert_eq!(parse_card_list("[Ah Kd]").len(), 2);
        assert_eq!(parse_card_list("[ Ah, Kd, 7s ]").len(), 3);
    }
}
