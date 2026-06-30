use crate::model::{ParseHandHistoryError, ParsedHand};

pub trait SiteParser: Send + Sync {
    fn site_name(&self) -> &'static str;

    /// Cheap heuristic: does this raw upload look like this site's format?
    /// Used by `detect::detect_site` to pick a parser without trying all of
    /// them in full.
    fn sniff(&self, raw: &str) -> bool;

    /// Split a multi-hand file into individual hand blocks. Sites separate
    /// hands with blank lines, so the default works for all of them; a site
    /// can override if it does something unusual.
    fn split_hands<'a>(&self, raw: &'a str) -> Vec<&'a str> {
        raw.split("\n\n")
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .collect()
    }

    fn parse_hand(&self, block: &str) -> Result<ParsedHand, ParseHandHistoryError>;
}
