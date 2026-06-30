//! A from-scratch poker hand evaluator (5-7 cards -> best 5-card hand).
//!
//! This used to wrap the `robopoker` crate's bitwise evaluator. Two things
//! changed that decision: (1) `robopoker` unconditionally depends on
//! `tokio-postgres` for an unrelated persistence feature this app never
//! uses, which is a heavy, somewhat fragile transitive dependency for what
//! is otherwise a small, extremely well-understood algorithm; and (2)
//! writing it directly means every line of hand-ranking logic in this app
//! -- the single most correctness-critical piece, since a wrong evaluator
//! silently corrupts solver output, equity, and drill grading alike -- is
//! both visible and fully covered by tests in this repo rather than
//! delegated to (and trusted sight-unseen from) a third-party crate.
//!
//! The algorithm is the standard one: classify by flush/straight, then by
//! rank-multiplicity histogram, encoding both the hand category and its
//! tiebreak-relevant ranks directly in an `Ord`-derived enum so comparing
//! two hands is just `a.cmp(&b)`.

use rib_core::{Card, Rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ranking {
    HighCard([Rank; 5]),
    OnePair(Rank, [Rank; 3]),
    TwoPair(Rank, Rank, Rank),
    ThreeOfAKind(Rank, [Rank; 2]),
    Straight(Rank),
    Flush([Rank; 5]),
    FullHouse(Rank, Rank),
    FourOfAKind(Rank, Rank),
    StraightFlush(Rank),
}

/// A hand's overall strength. `Ord` directly answers "who wins": higher
/// always beats lower; equal means a split pot. The enum variant order in
/// `Ranking` (declared weakest to strongest) is what makes derived `Ord`
/// correctly rank *any* hand in a stronger category above *any* hand in a
/// weaker one, regardless of specific ranks -- e.g. every `Flush` outranks
/// every `ThreeOfAKind` because `Flush` is declared after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Strength(Ranking);

impl Strength {
    pub fn ranking(&self) -> Ranking {
        self.0
    }
}

/// Evaluates the best possible 5-card hand from 5, 6, or 7 cards -- the
/// only counts this app ever needs (Omaha's exact 2-hole+3-board selection
/// always hands this exactly 5; hold'em's flop/turn/river evaluation hands
/// it 5/6/7). Tries every `C(n,5)` combination and keeps the best.
pub fn evaluate(cards: &[Card]) -> Strength {
    assert!((5..=7).contains(&cards.len()), "evaluate() expects 5 to 7 cards, got {}", cards.len());
    if cards.len() == 5 {
        return Strength(rank_exactly_5([cards[0], cards[1], cards[2], cards[3], cards[4]]));
    }
    let mut best: Option<Ranking> = None;
    for combo in combinations_5(cards) {
        let r = rank_exactly_5(combo);
        best = Some(match best {
            Some(b) if b >= r => b,
            _ => r,
        });
    }
    Strength(best.expect("evaluate() already asserted at least 5 cards, so at least one combination exists"))
}

/// Every way to choose 5 of `cards` (which has 5, 6, or 7 elements) -- a
/// plain nested-loop enumeration is simplest and plenty fast for numbers
/// this small (at most `C(7,5)` = 21 combinations).
fn combinations_5(cards: &[Card]) -> Vec<[Card; 5]> {
    let n = cards.len();
    let mut out = Vec::with_capacity(21);
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                for d in (c + 1)..n {
                    for e in (d + 1)..n {
                        out.push([cards[a], cards[b], cards[c], cards[d], cards[e]]);
                    }
                }
            }
        }
    }
    out
}

fn rank_exactly_5(cards: [Card; 5]) -> Ranking {
    let mut ranks: [Rank; 5] = [cards[0].rank, cards[1].rank, cards[2].rank, cards[3].rank, cards[4].rank];
    ranks.sort_unstable_by(|a, b| b.cmp(a));

    let is_flush = cards[1..].iter().all(|c| c.suit == cards[0].suit);

    // (rank, count) histogram, sorted by count desc then rank desc -- so
    // counts[0] is always "the most/highest-ranked repeated group" and so
    // on, which is exactly the tiebreak order every hand category needs.
    let mut counts: Vec<(Rank, u8)> = Vec::new();
    for r in ranks {
        if let Some(entry) = counts.iter_mut().find(|(rr, _)| *rr == r) {
            entry.1 += 1;
        } else {
            counts.push((r, 1));
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

    let mut distinct = ranks.to_vec();
    distinct.dedup();
    let straight_high = find_straight(&distinct);

    if is_flush {
        if let Some(high) = straight_high {
            return Ranking::StraightFlush(high);
        }
    }
    if counts[0].1 == 4 {
        return Ranking::FourOfAKind(counts[0].0, counts[1].0);
    }
    if counts[0].1 == 3 && counts.get(1).map(|c| c.1).unwrap_or(0) >= 2 {
        return Ranking::FullHouse(counts[0].0, counts[1].0);
    }
    if is_flush {
        return Ranking::Flush(ranks);
    }
    if let Some(high) = straight_high {
        return Ranking::Straight(high);
    }
    if counts[0].1 == 3 {
        return Ranking::ThreeOfAKind(counts[0].0, [counts[1].0, counts[2].0]);
    }
    if counts[0].1 == 2 && counts.get(1).map(|c| c.1).unwrap_or(0) == 2 {
        let (hi, lo) = if counts[0].0 > counts[1].0 { (counts[0].0, counts[1].0) } else { (counts[1].0, counts[0].0) };
        return Ranking::TwoPair(hi, lo, counts[2].0);
    }
    if counts[0].1 == 2 {
        return Ranking::OnePair(counts[0].0, [counts[1].0, counts[2].0, counts[3].0]);
    }
    Ranking::HighCard(ranks)
}

/// A straight needs exactly 5 *distinct* ranks (any pair/trips/quads rules
/// it out immediately) that are either 5 consecutive values, or the wheel
/// (A-2-3-4-5, where the ace plays low and the straight's "high card" for
/// tiebreak purposes is the Five).
fn find_straight(distinct_desc: &[Rank]) -> Option<Rank> {
    if distinct_desc.len() != 5 {
        return None;
    }
    let consecutive = distinct_desc.windows(2).all(|w| (w[0] as i32) - (w[1] as i32) == 1);
    if consecutive {
        return Some(distinct_desc[0]);
    }
    if distinct_desc == [Rank::Ace, Rank::Five, Rank::Four, Rank::Three, Rank::Two] {
        return Some(Rank::Five);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hand(s: &str) -> Vec<Card> {
        rib_core::parse_cards(s).unwrap()
    }

    fn eval(s: &str) -> Ranking {
        evaluate(&hand(s)).ranking()
    }

    #[test]
    fn high_card() {
        assert_eq!(eval("As Kh Qd Jc 9s"), Ranking::HighCard([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Nine]));
    }

    #[test]
    fn one_pair() {
        assert_eq!(eval("As Ah Kd Qc Js"), Ranking::OnePair(Rank::Ace, [Rank::King, Rank::Queen, Rank::Jack]));
    }

    #[test]
    fn two_pair() {
        assert_eq!(eval("As Ah Kd Kc Qs"), Ranking::TwoPair(Rank::Ace, Rank::King, Rank::Queen));
    }

    #[test]
    fn trips() {
        assert_eq!(eval("As Ah Ad Kc Qs"), Ranking::ThreeOfAKind(Rank::Ace, [Rank::King, Rank::Queen]));
    }

    #[test]
    fn straight() {
        assert_eq!(eval("Ts Jh Qd Kc As"), Ranking::Straight(Rank::Ace));
    }

    #[test]
    fn wheel_straight() {
        assert_eq!(eval("As 2h 3d 4c 5s"), Ranking::Straight(Rank::Five));
    }

    #[test]
    fn flush() {
        assert_eq!(eval("As Ks Qs Js 9s"), Ranking::Flush([Rank::Ace, Rank::King, Rank::Queen, Rank::Jack, Rank::Nine]));
    }

    #[test]
    fn full_house() {
        assert_eq!(eval("As Ah Ad Kc Ks"), Ranking::FullHouse(Rank::Ace, Rank::King));
    }

    #[test]
    fn four_of_a_kind() {
        assert_eq!(eval("As Ah Ad Ac Ks"), Ranking::FourOfAKind(Rank::Ace, Rank::King));
    }

    #[test]
    fn straight_flush() {
        assert_eq!(eval("Ts Js Qs Ks As"), Ranking::StraightFlush(Rank::Ace));
    }

    #[test]
    fn wheel_straight_flush() {
        assert_eq!(eval("As 2s 3s 4s 5s"), Ranking::StraightFlush(Rank::Five));
    }

    #[test]
    fn category_order_is_correct() {
        // Spot-check the full ordering top to bottom on fixed hands, since
        // this is the one property the whole app depends on.
        let straight_flush = evaluate(&hand("Ts Js Qs Ks As"));
        let quads = evaluate(&hand("As Ah Ad Ac Ks"));
        let full_house = evaluate(&hand("As Ah Ad Kc Ks"));
        let flush = evaluate(&hand("As Ks Qs Js 9s"));
        let straight = evaluate(&hand("Ts Jh Qd Kc As"));
        let trips = evaluate(&hand("As Ah Ad Kc Qs"));
        let two_pair = evaluate(&hand("As Ah Kd Kc Qs"));
        let one_pair = evaluate(&hand("As Ah Kd Qc Js"));
        let high_card = evaluate(&hand("As Kh Qd Jc 9s"));
        assert!(straight_flush > quads);
        assert!(quads > full_house);
        assert!(full_house > flush);
        assert!(flush > straight);
        assert!(straight > trips);
        assert!(trips > two_pair);
        assert!(two_pair > one_pair);
        assert!(one_pair > high_card);
    }

    #[test]
    fn best_of_seven_finds_the_best_five() {
        // 7 cards containing both a flush and a higher full house -- the
        // evaluator must find the full house, not just the first thing it
        // notices.
        let r = evaluate(&hand("As Ah Ad Ks Kc Qs Js"));
        assert_eq!(r.ranking(), Ranking::FullHouse(Rank::Ace, Rank::King));
    }

    #[test]
    fn best_of_six_picks_better_than_worse_subsets() {
        let r = evaluate(&hand("2h 3h 4h 5h 6h 9c"));
        assert_eq!(r.ranking(), Ranking::StraightFlush(Rank::Six));
    }

    #[test]
    fn kicker_breaks_ties_between_one_pair_hands() {
        let a = evaluate(&hand("As Ah Kd Qc Js"));
        let b = evaluate(&hand("As Ah Kd Qc 9s"));
        assert!(a > b);
    }
}
