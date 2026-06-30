import { useState } from "react";
import { api, ApiError, pollSolveJob } from "../api/client";
import type { SolveResponse } from "../api/types";
import RangeGrid from "../components/RangeGrid";
import OmahaStrategyList from "../components/OmahaStrategyList";

const POSITIONS = ["UTG", "HJ", "CO", "BTN", "SB", "BB"];
const POT_TYPES = [
  { value: "srp", label: "Single Raised Pot" },
  { value: "three_bet", label: "3-Bet Pot" },
  { value: "four_bet", label: "4-Bet Pot" },
];

const DEFAULTS = {
  nlhe: {
    heroRange: "22+,A2+,K9+,QTs+",
    villainRange: "22+,A2+,K2+,Q5+,J8+,T8+,98s,87s,76s",
  },
  plo: {
    heroRange: "AAKK,AAQQ,AAJJ,AKQJds,AKQTds,AKJTds,KQJTds",
    villainRange: "TT99,9988,8877,T987ss,9876ss,JT98ds,9876ds",
  },
};

export default function RangeExplorer() {
  const [tab, setTab] = useState<"preflop" | "custom">("preflop");

  // -- preflop library state (NLHE only -- PLO has no curated library) --
  const [hero, setHero] = useState("BTN");
  const [villain, setVillain] = useState("BB");
  const [stackBb, setStackBb] = useState(100);
  const [potType, setPotType] = useState("srp");

  // -- custom solve state --
  const [game, setGame] = useState<"nlhe" | "plo">("nlhe");
  const [heroRange, setHeroRange] = useState(DEFAULTS.nlhe.heroRange);
  const [villainRange, setVillainRange] = useState(DEFAULTS.nlhe.villainRange);
  const [board, setBoard] = useState("Ah 7d 2c");
  const [stackCustom, setStackCustom] = useState(100);
  const [potCustom, setPotCustom] = useState(8);
  const [facing, setFacing] = useState<number | "">("");
  const [iterations, setIterations] = useState(800);

  const [result, setResult] = useState<SolveResponse | null>(null);
  const [resultGame, setResultGame] = useState<"nlhe" | "plo">("nlhe");
  const [loading, setLoading] = useState(false);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);

  function switchGame(g: "nlhe" | "plo") {
    setGame(g);
    setHeroRange(DEFAULTS[g].heroRange);
    setVillainRange(DEFAULTS[g].villainRange);
  }

  async function loadPreflop() {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const r = await api.preflopLibrary(hero, villain, stackBb, potType);
      setResult(r);
      setResultGame("nlhe");
    } catch (err) {
      if (err instanceof ApiError && err.status === 404) {
        setError("Not in the precomputed library yet. Try the Custom Solve tab to solve this exact spot live.");
      } else {
        setError(err instanceof ApiError ? err.message : "Something went wrong");
      }
    } finally {
      setLoading(false);
    }
  }

  async function runCustomSolve() {
    setLoading(true);
    setError(null);
    setResult(null);
    setProgress(0);
    try {
      const req = {
        game,
        board: board.trim().length > 0 ? board.trim().split(/\s+/) : [],
        effective_stack_bb: stackCustom,
        starting_pot_bb: potCustom,
        hero_invested_bb: potCustom / 2,
        villain_invested_bb: potCustom / 2,
        hero_range: heroRange,
        villain_range: villainRange,
        hero_is_in_position: true,
        context: facing === "" ? "first_to_act" : { facing_bet: { size_bb: Number(facing) } },
        sizings: null,
        streets_to_extend: 0,
        iterations,
      };
      const { job_id } = await api.enqueueSolve(req);
      const job = await pollSolveJob(job_id, (j) => setProgress(j.progress));
      if (job.status === "failed") throw new ApiError(500, job.error ?? "solve failed");
      setResult(job.result);
      setResultGame(game);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Something went wrong");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Range Explorer</h1>
          <p>Instant preflop charts from the precomputed library, or a live custom solve for any spot -- NLHE or PLO.</p>
        </div>
      </div>

      <div className="btn-row" style={{ marginBottom: 20 }}>
        <button className={"btn" + (tab === "preflop" ? " btn-primary" : "")} onClick={() => setTab("preflop")}>
          Preflop Library (NLHE)
        </button>
        <button className={"btn" + (tab === "custom" ? " btn-primary" : "")} onClick={() => setTab("custom")}>
          Custom Solve
        </button>
      </div>

      <div className="grid grid-2">
        <div className="card">
          {tab === "preflop" ? (
            <>
              <h3>Spot</h3>
              <div className="field">
                <label>Hero position</label>
                <select value={hero} onChange={(e) => setHero(e.target.value)}>
                  {POSITIONS.map((p) => (
                    <option key={p}>{p}</option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label>Villain position</label>
                <select value={villain} onChange={(e) => setVillain(e.target.value)}>
                  {POSITIONS.map((p) => (
                    <option key={p}>{p}</option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label>Pot type</label>
                <select value={potType} onChange={(e) => setPotType(e.target.value)}>
                  {POT_TYPES.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label>Effective stack (bb)</label>
                <select value={stackBb} onChange={(e) => setStackBb(Number(e.target.value))}>
                  {[20, 40, 60, 100].map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </div>
              <button className="btn btn-primary btn-block" onClick={loadPreflop} disabled={loading}>
                {loading ? "Loading..." : "Load chart"}
              </button>
            </>
          ) : (
            <>
              <h3>Custom spot</h3>
              <div className="field">
                <label>Game</label>
                <div className="btn-row">
                  <button className={"btn" + (game === "nlhe" ? " btn-primary" : "")} onClick={() => switchGame("nlhe")} type="button">
                    NLHE
                  </button>
                  <button className={"btn" + (game === "plo" ? " btn-primary" : "")} onClick={() => switchGame("plo")} type="button">
                    PLO
                  </button>
                </div>
              </div>
              <div className="field">
                <label>Hero range{game === "plo" ? " (4-rank patterns, e.g. AAKK, AKQJds)" : ""}</label>
                <input value={heroRange} onChange={(e) => setHeroRange(e.target.value)} />
              </div>
              <div className="field">
                <label>Villain range</label>
                <input value={villainRange} onChange={(e) => setVillainRange(e.target.value)} />
              </div>
              {game === "plo" && (
                <p className="muted" style={{ marginTop: -8, fontSize: 12 }}>
                  PLO ranges are rank patterns (4 letters, repeats allowed) with an optional suit filter: ds = double
                  suited, ss = single suited, mono = all one suit, r = rainbow. No "+" shorthand yet -- list each
                  pattern explicitly, comma separated.
                </p>
              )}
              <div className="field">
                <label>Board (blank for preflop)</label>
                <input value={board} onChange={(e) => setBoard(e.target.value)} placeholder="e.g. Ah 7d 2c" />
              </div>
              <div className="grid grid-2">
                <div className="field">
                  <label>Effective stack (bb)</label>
                  <input type="number" value={stackCustom} onChange={(e) => setStackCustom(Number(e.target.value))} />
                </div>
                <div className="field">
                  <label>Starting pot (bb)</label>
                  <input type="number" value={potCustom} onChange={(e) => setPotCustom(Number(e.target.value))} />
                </div>
              </div>
              <div className="field">
                <label>Facing a bet of (bb) -- leave blank if first to act</label>
                <input
                  type="number"
                  value={facing}
                  onChange={(e) => setFacing(e.target.value === "" ? "" : Number(e.target.value))}
                />
              </div>
              <div className="field">
                <label>Iterations</label>
                <input type="number" value={iterations} onChange={(e) => setIterations(Number(e.target.value))} />
              </div>
              <button className="btn btn-primary btn-block" onClick={runCustomSolve} disabled={loading}>
                {loading ? `Solving... ${(progress * 100).toFixed(0)}%` : "Solve"}
              </button>
            </>
          )}
          {error && <div className="error-banner" style={{ marginTop: 16 }}>{error}</div>}
        </div>

        <div className="card">
          <h3>Strategy</h3>
          {!result ? (
            <p>Run a solve to see the strategy.</p>
          ) : (
            <>
              {resultGame === "plo" ? (
                <OmahaStrategyList actions={result.hero_strategy.actions} frequencies={result.hero_strategy.frequencies} />
              ) : (
                <RangeGrid actions={result.hero_strategy.actions} frequencies={result.hero_strategy.frequencies} />
              )}
              <div className="grid grid-3" style={{ marginTop: 20 }}>
                <div className="stat">
                  <div className="stat-label">Hero EV</div>
                  <div className={"stat-value " + (result.hero_ev_bb >= 0 ? "ev-positive" : "ev-negative")}>
                    {result.hero_ev_bb.toFixed(2)}bb
                  </div>
                </div>
                <div className="stat">
                  <div className="stat-label">Iterations</div>
                  <div className="stat-value">{result.iterations_run}</div>
                </div>
                <div className="stat">
                  <div className="stat-label">Combos</div>
                  <div className="stat-value">
                    {result.n_hero_combos}x{result.n_villain_combos}
                  </div>
                </div>
              </div>
              {result.warnings.length > 0 && (
                <p className="muted" style={{ marginTop: 12 }}>
                  {result.warnings.join(" ")}
                </p>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
