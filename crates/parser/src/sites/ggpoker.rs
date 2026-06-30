//! GGPoker's `.txt` export is structurally a close cousin of PokerStars'
//! (same `*** FLOP/TURN/RIVER ***`, `Seat N:`, `posts small blind` idioms),
//! differing mainly in the header line ("Poker Hand #TM..." rather than
//! "PokerStars Hand #...", and table names like "NLHE" without the site
//! name). We reuse all of `pokerstars.rs`'s body-parsing logic and only
//! supply our own header regex.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::model::{ParseHandHistoryError, ParsedHand};
use crate::site_parser::SiteParser;
use crate::sites::pokerstars::parse_pokerstars_family_hand;

static HEADER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)^Poker\s+Hand\s+#(?P<id>[A-Za-z0-9]+):\s+(?P<game>Hold'em|Omaha)\s+(?:No\s+Limit|Pot\s+Limit|Limit)\s+\(\$?(?P<sb>[\d.,]+)/\$?(?P<bb>[\d.,]+)").unwrap()
});

pub struct GgPokerParser;

impl SiteParser for GgPokerParser {
    fn site_name(&self) -> &'static str {
        "ggpoker"
    }

    fn sniff(&self, raw: &str) -> bool {
        raw.lines().take(5).any(|l| {
            let lower = l.to_ascii_lowercase();
            lower.starts_with("poker hand #") && !lower.contains("pokerstars")
        })
    }

    fn parse_hand(&self, block: &str) -> Result<ParsedHand, ParseHandHistoryError> {
        parse_pokerstars_family_hand(block, "ggpoker", &HEADER)
    }
}
