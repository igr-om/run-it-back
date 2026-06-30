pub mod common;
pub mod detect;
pub mod model;
pub mod site_parser;
pub mod sites;
pub mod stats;

pub use detect::{detect_site, parse_file, ParseOutcome};
pub use model::{ActionKind, ParseHandHistoryError, ParsedAction, ParsedHand};
pub use site_parser::SiteParser;

/// Parses a raw upload and fills in each hand's `tags` field via
/// `stats::derive_tags` -- the one thing `detect::parse_file` doesn't do on
/// its own, since tag derivation is a separate concern from format parsing.
pub fn parse_and_tag(raw: &str) -> Result<ParseOutcome, ParseHandHistoryError> {
    let mut outcome = detect::parse_file(raw)?;
    for hand in &mut outcome.hands {
        hand.tags = stats::derive_tags(hand);
    }
    Ok(outcome)
}
