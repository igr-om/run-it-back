export default function About() {
  return (
    <div className="about-page">
      <div className="page-header">
        <div>
          <h1>About / How it works</h1>
          <p>The broad strokes of what's actually computing your strategies, for anyone curious.</p>
        </div>
      </div>

      <div className="card">
        <h2>What this is, in one paragraph</h2>
        <p>
          Run It Back computes poker strategy the same way every real GTO solver does at its core: it builds a tree of
          every decision a hand can reach, assigns a payoff to every way that tree can end, and then runs an algorithm
          called <strong>counterfactual regret minimization</strong> (CFR) over and over until both players' strategies
          stop being exploitable. The tree, the payoffs, and the number of times the algorithm runs are all deliberately
          capped so a solve finishes in seconds instead of hours -- that's the entire difference between this and a
          commercial solver: scope, not method.
        </p>
      </div>

      <div className="card">
        <h2>1. Hand evaluation</h2>
        <p>
          Before anything else, the engine needs to know which 5-card hand beats which. This app implements that from
          scratch (no external poker library): every hand is classified into one of the 9 standard categories
          (high card through straight flush), and within a category, ties are broken by comparing ranks in order
          (pair rank, then kickers, highest to lowest). For 6 or 7 card hands (hold'em on the turn/river), it tries
          every possible 5-card subset and keeps the best one -- there are at most 21 such subsets, so this is cheap.
        </p>
        <p>
          <strong>Omaha is different in one specific way</strong>: a legal Omaha hand must use{" "}
          <em>exactly</em> 2 of your 4 hole cards and exactly 3 of the 5 board cards -- never more, never fewer. So
          for Omaha, the evaluator tries every combination of (2 of your 4 hole cards) x (3 of the board's cards) and
          keeps the best 5-card hand among <em>those</em> combinations specifically. This is the one rule that breaks
          the most poker software that tries to bolt Omaha onto hold'em code, and it has its own dedicated test in
          this codebase to make sure it's never silently violated.
        </p>
      </div>

      <div className="card">
        <h2>2. Equity: Monte Carlo simulation</h2>
        <p>
          "Equity" is the question "if these two hands' cards were locked in and the rest of the board were dealt out
          randomly, what fraction of the time does each hand win?" When the board isn't complete yet (preflop, flop,
          or turn), there are too many possible runouts to check exactly in real time, so the engine estimates it the
          standard way: deal the remaining cards randomly, see who wins, repeat many times, and average the results.
          This is called <strong>Monte Carlo simulation</strong>. More repetitions (samples) means a more precise
          estimate but takes longer; this app uses on the order of dozens to a couple hundred samples per matchup,
          which is precise enough to guide strategy without making a solve take minutes.
        </p>
      </div>

      <div className="card">
        <h2>3. The bet tree</h2>
        <p>
          A real poker decision tree is continuous -- you could bet any amount from 1 chip to your entire stack.
          That's solvable in theory but not in a few seconds, so this app (like every fast solver) restricts each
          betting decision to a <strong>fixed menu of sizes</strong>: thirds-pot, two-thirds-pot, pot, and all-in for
          an opening bet, and a smaller pot/all-in-only menu for re-raises (since allowing the full menu again at
          every raise level makes the tree's size grow exponentially with how many raises deep the hand goes -- 4
          options at 3 raise levels is already hundreds of distinct branches to track). The strategy this app finds
          is the optimal strategy <em>among the sizes it was allowed to consider</em>, not the optimal strategy among
          literally every possible bet size. That's a real simplification, and it's the same one essentially every
          interactive solver makes -- the alternative is a multi-day offline computation, which doesn't fit a web
          request.
        </p>
      </div>

      <div className="card">
        <h2>4. CFR+: how the strategy is actually found</h2>
        <p>
          Once the tree and the payoffs at every ending are set up, the engine repeatedly plays the game against
          itself and learns from regret. In plain terms, on each pass: for every decision point and every hand the
          player could be holding there, the algorithm asks "looking back, how much better or worse would each
          available action have done compared to what I actually played?" That difference is called{" "}
          <strong>regret</strong>. Actions with positive regret (ones that would have done better) get played more
          often on the next pass, in proportion to how much better they would have done. Do this thousands of times,
          and the strategy converges toward one that can't be meaningfully exploited -- a Nash equilibrium.
        </p>
        <p>
          The "+" in CFR+ (the specific variant this app uses) refers to one refinement: regret is never allowed to
          go below zero. An action that's been bad for a while gets "forgotten" faster than in the original CFR
          algorithm, which in practice means the strategy converges in meaningfully fewer iterations -- useful when
          every iteration costs real time in a live solve. The final strategy reported isn't just the last pass's
          decision, either; it's an average of the strategy used across every pass, weighted toward later passes
          (which tend to be more refined) -- a single pass can be noisy, but the running average is stable.
        </p>
        <p>
          Live solves in this app run on the order of a few hundred to ~1,500 of these passes, capped so a solve
          reliably finishes in a few seconds. The response includes a rough exploitability estimate (total
          accumulated positive regret, normalized by iteration count) so you can get a sense of how converged a
          given result is -- lower is more converged.
        </p>
      </div>

      <div className="card">
        <h2>5. Why wide ranges get trimmed</h2>
        <p>
          Before a single CFR pass can even run, the engine has to know the payoff for every one of your hand vs.
          every one of your opponent's hand at every point the hand could end in a showdown. If both ranges are wide
          (a "100%" range -- every possible starting hand -- has 1,326 specific hold'em combos, and an unrestricted
          Omaha range has far more), that's well over a million pairs to evaluate, each needing its own Monte Carlo
          equity calculation, before any actual solving starts. That's minutes of work for a single spot, which
          isn't an iteration-count problem -- it doesn't matter if you ask for 1 pass or 10,000, the setup cost is
          the same.
        </p>
        <p>
          So live solves cap each side at a couple dozen representative hands, chosen so every distinct hand class
          (every "AA", every "AKs", and so on) is guaranteed at least one representative -- nothing gets silently
          erased from the result, but very wide ranges get a sampled, not exhaustive, treatment. The response's{" "}
          <code>warnings</code> field tells you exactly when this happened. The precomputed preflop library sidesteps
          this entirely for the spots it covers, since those were solved once, in full, ahead of time, and just get
          looked up instantly.
        </p>
      </div>

      <div className="card">
        <h2>6. PLO specifically</h2>
        <p>
          Everything above -- hand evaluation, equity, the bet tree, CFR+ -- works for Omaha using the same code as
          hold'em; the only thing that changes under the hood is hands carry 4 cards instead of 2, and the evaluator
          enforces the exactly-2-from-hole rule described above. One gap specific to PLO: a real pot-limit raise can
          never be larger than the pot, but this solver's bet-sizing menu doesn't currently calculate or enforce that
          cap -- it reuses the same fixed menu hold'em uses, where any size up to the stack is always an option. In
          deep-stacks-relative-to-pot spots, that means a PLO solve might suggest a sizing that's technically too
          large to be legal at a real table. It's flagged in the response every time. Hold'em has no equivalent gap.
        </p>
      </div>

      <div className="card">
        <h2>7. Drill categories</h2>
        <p>
          The trainer covers nine categories spanning every street: three preflop (opening, defending against an
          open, and facing a 3-bet) and three postflop streets, each with both the "I'm betting" and "I'm facing a
          bet" side of the decision (flop c-bet, turn barrel, river bet). Turn and river spots deal a board of the
          right length and solve that exact node directly -- a river board resolves to an exact comparison with no
          simulation needed at all, since every card is already known, which is why river drills tend to load
          fastest. Which category you see next is weighted toward whatever your weakness profile says you're
          currently worst at, with a floor so every category stays in rotation even once you've improved.
        </p>
      </div>

      <div className="card">
        <h2>8. Drill grading</h2>
        <p>
          When you answer a drill, the app already has the EV (in big blinds) of every available action from the
          solve that generated it. Grading compares the EV of what you chose against the best available action's EV;
          if the gap is small (or the solver itself plays your chosen action a meaningful fraction of the time as
          part of a legitimately mixed strategy) it's marked correct. Otherwise, a set of rules checks the specific
          situation -- bet sizes, position, board texture, blockers, stack depth -- and explains the mistake in terms
          of whichever real poker concept actually applies (fold equity, pot odds, reverse implied odds, blockers,
          range polarization, and so on), rather than a generic "wrong answer." Only the rules that are actually
          relevant to that specific spot fire, so the explanation should always be about the mistake you made, not a
          canned list of poker concepts.
        </p>
      </div>

      <div className="card">
        <h2>Source</h2>
        <p style={{ marginBottom: 0 }}>
          Every piece described above is implemented in this app's own backend (Rust), not delegated to an external
          poker engine. If you're curious to go deeper than this page, the relevant code lives in{" "}
          <code>crates/evaluator</code> (hand ranking + equity), <code>crates/solver</code> (the bet tree and CFR+
          engine), and <code>crates/drills</code> (the grading rules) -- each file has comments explaining the
          specific reasoning behind the choices summarized here.
        </p>
      </div>
    </div>
  );
}
