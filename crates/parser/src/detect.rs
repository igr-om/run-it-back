use crate::model::{ParseHandHistoryError, ParsedHand};
use crate::site_parser::SiteParser;
use crate::sites::{ggpoker::GgPokerParser, poker888::Poker888Parser, pokerstars::PokerStarsParser, wpn_family::{PartyPokerParser, WpnParser}};

fn all_parsers() -> Vec<Box<dyn SiteParser>> {
    vec![
        Box::new(PokerStarsParser),
        Box::new(GgPokerParser),
        Box::new(Poker888Parser),
        Box::new(PartyPokerParser),
        Box::new(WpnParser),
    ]
}

pub fn detect_site(raw: &str) -> Option<Box<dyn SiteParser>> {
    all_parsers().into_iter().find(|p| p.sniff(raw))
}

pub struct ParseOutcome {
    pub site: String,
    pub hands: Vec<ParsedHand>,
    pub total_blocks: usize,
    pub failed_blocks: usize,
}

/// Detects the site, splits the upload into hand blocks, and parses every
/// block it can. Individual malformed blocks are skipped (and counted in
/// `failed_blocks`) rather than failing the whole upload -- a hand history
/// file with one weird hand in it (disconnect, all-in insurance, a format
/// quirk) shouldn't prevent importing the other 500 hands.
pub fn parse_file(raw: &str) -> Result<ParseOutcome, ParseHandHistoryError> {
    // Normalize CRLF/CR to LF up front. This matters a lot in practice:
    // most poker clients run on Windows and export hand histories with
    // \r\n line endings, and \r\n\r\n does not contain the literal "\n\n"
    // substring that hand-block splitting (and several site parsers' own
    // line-by-line logic) looks for -- without this, a Windows-exported
    // multi-hand file would be treated as a single unparseable block
    // instead of hundreds of individual hands.
    let raw = raw.replace("\r\n", "\n").replace('\r', "\n");
    let raw = raw.as_str();

    let parser = detect_site(raw).ok_or(ParseHandHistoryError::UnknownSite)?;
    let blocks = parser.split_hands(raw);
    let total_blocks = blocks.len();
    let mut hands = Vec::with_capacity(total_blocks);
    let mut failed_blocks = 0usize;
    let mut last_plo_only = total_blocks > 0;

    for block in blocks {
        match parser.parse_hand(block) {
            Ok(h) => {
                last_plo_only = false;
                hands.push(h);
            }
            Err(ParseHandHistoryError::PloUnsupported) => {
                failed_blocks += 1;
            }
            Err(_) => {
                failed_blocks += 1;
                last_plo_only = false;
            }
        }
    }

    if hands.is_empty() {
        if last_plo_only && total_blocks > 0 {
            return Err(ParseHandHistoryError::PloUnsupported);
        }
        return Err(ParseHandHistoryError::NoHandsParsed { attempted: total_blocks });
    }

    Ok(ParseOutcome { site: parser.site_name().to_string(), hands, total_blocks, failed_blocks })
}
