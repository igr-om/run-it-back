//! `rib-solver`: the actual GTO-solving engine.
//!
//! GTOWizard-class tools run Pluribus-grade *blueprint* solving, which
//! requires a multi-day, cluster-scale offline abstraction/training
//! pipeline (k-means clustering over trillions of abstracted states) --
//! not something that can run on demand for an arbitrary user-specified
//! spot. What this crate implements instead is a from-scratch CFR+ solver
//! over a *concretely abstracted* bet tree (a fixed, configurable set of
//! bet sizings rather than a learned action abstraction, and specific
//! 2-card hand combos rather than learned card buckets). That keeps trees
//! small enough to solve in real time while still being a genuine
//! equilibrium computation, not a lookup table or heuristic. Hand
//! evaluation/equity, the one piece that's both performance-critical and
//! not worth reinventing per-crate, is delegated to `rib-evaluator` (a
//! from-scratch 5-7 card evaluator -- see that crate's `handrank.rs`).
//!
//! Two ways spots get solved, matching the product decision to support both:
//! - **Live**: `engine::solve` runs this crate's CFR+ end to end for
//!   whatever spot the request describes, capped at `MAX_LIVE_ITERATIONS`.
//! - **Library**: `library::SpotKey` + `library::curated_preflop_seed_list`
//!   define a set of common spots that get solved once (by a background
//!   worker, see `rib-worker`) and cached in Postgres, so the trainer's most
//!   common preflop charts load instantly instead of re-solving every time.

pub mod cfr;
pub mod engine;
pub mod hand_index;
pub mod library;
pub mod payoff;
pub mod pot;
pub mod request;
pub mod tree;

pub use engine::{solve, MAX_LIVE_ITERATIONS, MIN_LIVE_ITERATIONS};
pub use library::{curated_preflop_seed_list, SpotKey};
pub use request::{NodeContext, Player, SolveRequest, SolveResponse};
