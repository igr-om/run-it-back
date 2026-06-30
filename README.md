# Run It Back

A free, self-hosted GTO training app for No-Limit Hold'em inspired by GTOWizard, built entirely on a Rust backend. Solve any spot with a real CFR+ engine, drill adaptively against your own weaknesses, and upload your hand histories to find leaks in your actual game.

This is a v1 of a genuinely large project. It's real, working software not a mockup but it makes deliberate scope decisions explained in [Honest limitations](#honest-limitations) below. Read that section before you assume it does something it doesn't.

## What's actually in here

- **A from-scratch CFR+ solver** ('crates/solver') over a concretely-abstracted bet tree (configurable sizings, specific 2-card hand combos, real card-removal). Two ways to get a strategy:
  - **Live solve**: any spot you describe, solved on demand, capped at a few hundred to ~1500 iterations so it returns in real time.
  - **Precomputed library**: a curated set of common preflop spots (every position pair, SRP/3-bet/4-bet, multiple stack depths) pre-solved by a background worker on startup and cached in Postgres, so the trainer's most common charts load instantly.
- **A from-scratch hand evaluator** ('crates/evaluator', see 'handrank.rs') for both NLHE (best of 5-7 cards) and Omaha (best of *exactly* 2 hole + 3 board cards, the one rule that makes Omaha evaluation different from hold'em) no external poker-logic dependency, fully covered by tests.
- **PLO solving now works.** The CFR+ engine, hand universe builder, and payoff precomputation are generalized over both 2-card (NLHE) and 4-card (Omaha) hands the same solver, same code path. You can solve a real Omaha spot (custom ranges, any board) from the Range Explorer's Custom Solve tab. What's *not* there yet: a PLO preflop library, PLO drill categories, and a "+"-style range shorthand for Omaha (ranges are explicit rank-patterns for now see [Honest limitations](#honest-limitations)).
- **A dynamic worker pool** ('crates/worker', tokio + crossbeam) a fixed set of threads that pull from whichever of the parse-queue / solve-queue / library-warm-queue has backlog, via 'crossbeam_channel::select!'. Capacity shifts automatically between hand-history parsing and CFR solving instead of being statically split.
- **An adaptive drill generator** ('crates/drills') covering 9 categories across every street preflop (open, defend vs. an open, facing a 3-bet) plus flop/turn/river, each with both sides of the decision (betting and facing a bet) weighted toward your worst categories, with a grading engine that explains *why* an answer was wrong in terms of real poker concepts (fold equity, pot odds, reverse implied odds, blockers, position, SPR, board texture, range polarization) not just "you're wrong."
- **A hand history parser** ('crates/parser') for PokerStars, GGPoker, 888poker, PartyPoker, and WPN-network sites (Bovada / Ignition / Americas Cardroom), each parsed into one canonical schema, with VPIP/PFR/3-bet/c-bet/etc. derived from it.
- **A Postgres-backed account system** with your drill history, weakness profile, saved ranges, and uploaded hands, so you can run this locally and actually keep your progress.
- **A React + TypeScript frontend**, dark professional theme, served by the same Rust binary in production (no separate static host needed).

## Architecture
crates/
  core/    Shared poker types: cards, ranges, positions, actions, bet categorization
  evaluator/ From-scratch hand evaluator (NLHE + Omaha) and Monte Carlo equity
  solver/  The CFR+ engine, bet-tree builder, payoff precomputation, solved-spot cache keys
  parser/  Per-site hand history parsers -> one canonical schema -> derived stats
  db/    Postgres models, queries, migrations (sqlx, runtime-checked, no compile-time DB needed)
  worker/  Dynamic worker pool (tokio + crossbeam) + curated-spot startup warming
  drills/  Adaptive drill generation + the rule-based grading/explanation engine
  server/  Axum HTTP API, JWT auth, route handlers, static frontend serving

web/     React + Vite + TypeScript SPA

Everything talks to Postgres through 'rib-db'; nothing else touches SQL directly. The solver and evaluator have zero knowledge of HTTP, Postgres, or each other's callers 'rib-server' is the only crate that wires HTTP requests to the rest.

## Honest limitations

Real talk, so you don't build expectations this can't meet:

- **PLO drilling, the preflop library, and "+"-style range shorthand aren't implemented yet.** Solving itself works (see above) what's missing is the adaptive drill generator's PLO categories, a curated/precomputed PLO spot library (NLHE's relies on the 169-class abstraction, which Omaha doesn't have an equivalent of), and range shorthand like NLHE's "22+" (Omaha ranges are explicit rank-patterns for now, e.g. "AAKK,AKQJds", comma separated see the Custom Solve tab's hint text).
- **The solver is a real CFR+ implementation, not a literal recreation of GTOWizard's solver for either game.** GTOWizard-class tools run abstraction pipelines (k-means clustering over trillions of states) on cluster hardware for hours to days to produce their preflop/postflop solutions. That's not something that can run on demand in a web request, for NLHE or PLO. What this app does instead, for both: a genuine CFR+ equilibrium computation over a *concretely* abstracted tree a fixed menu of bet sizings rather than continuous/learned sizing, specific hand combos rather than learned card buckets. That's the same trade every real-time solver makes, just at a coarser grain; it's a real equilibrium for the abstracted game actually solved, not a lookup table or heuristic.
- **The one gap specific to PLO**: a real pot-limit raise can never exceed the size of the pot, but this solver's sizing menu (including the "All-in" option) doesn't compute or enforce that cap it's the same fixed menu NLHE uses, where any size up to the stack is always legal. In deep-stacks-relative-to-pot spots, PLO solves may therefore offer (and find +EV for) a sizing too large to be legal at a real table. The live solve response's 'warnings' field flags this on every PLO solve. NLHE has no equivalent legality gap, only the precision/coverage trade-off described above.
- **Wide ranges get subsampled for live solving, by necessity.** A "100%" NLHE range carries 1,326 specific combos (an unrestricted Omaha range carries far more), and the payoff precomputation that has to run *before a single CFR iteration starts* is O(hero_combos x villain_combos) that many pairs would take on the order of minutes per spot regardless of iteration count. Live solves cap each side at a small number of representative combos (one per class, selected by weighted random sample if the range is wider than the cap), and the bet tree itself caps raises per street and uses a smaller sizing menu for re-raises than for opening bets, since the alternative a full sizing menu applied at every raise depth grows the tree combinatorially (sizings^raise-depth). All of this trades some precision for the solve actually returning in a few seconds instead of hanging. The response's 'warnings' field says when subsampling kicked in. See the in-app **About** page for the actual formulas behind all of this.
- **Checking/calling preflop with no street extension ('streets_to_extend: 0', the default for preflop drill categories) resolves straight to a random 5-card runout with no further betting modeled** there's no flop/turn/river decision in between. This is a real simplification of what "checking preflop" means in actual poker (where play continues across more streets), not a bug, but it means preflop strategies from this mode should be read as "is this hand worth getting to showdown with no further skill applied," not a full multi-street equilibrium. Postflop categories don't have this issue since they start from an existing board.
- **The curated preflop library uses 100% ranges on both sides.** That's not a simplification it's the textbook-correct way to compute an opening/defending range from scratch, since you want the solver to discover which hands should raise/call/fold for *every* holding, not be told the answer in advance. Postflop drill categories (c-bet, facing a c-bet) use simplified representative ranges for "what realistically gets to this spot" rather than chaining the actual upstream preflop solve's combo weights through.
- **The hand history parsers are best-effort**, especially for the WPN-network sites (Bovada / Ignition / Americas Cardroom), whose export format has varied the most across operators and software versions over the years. Each parser is written to degrade gracefully an unrecognized line is skipped, not fatal but very old or unusual exports may parse partially. PokerStars' and GGPoker's formats are the most stable and best-supported, for both hold'em and Omaha hands.
- **This hasn't been run against a live Postgres instance or load-tested.** Treat this as a serious, working v1 not a project that's been battle-tested in the field yet.


## License

MIT. Built for poker players who don't want to pay a subscription to study GTO.
