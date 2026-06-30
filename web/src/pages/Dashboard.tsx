import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api } from "../api/client";
import type { DrillAttemptRecord, PlayerStats, WeaknessProfile } from "../api/types";
import { CATEGORY_LABELS } from "../api/types";

function pct(v: number | null | undefined, digits = 1): string {
  if (v === null || v === undefined) return "--";
  return `${v.toFixed(digits)}%`;
}

export default function Dashboard() {
  const [stats, setStats] = useState<PlayerStats | null>(null);
  const [weakness, setWeakness] = useState<WeaknessProfile[]>([]);
  const [attempts, setAttempts] = useState<DrillAttemptRecord[]>([]);
  const [loading, setLoading] = useState(true);

  function refresh() {
    return Promise.all([api.statsOverview(), api.weaknessProfile(), api.recentAttempts()]).then(([s, w, a]) => {
      setStats(s);
      setWeakness(w);
      setAttempts(a);
    });
  }

  useEffect(() => {
    refresh().finally(() => setLoading(false));
    // Hand history parsing happens in the background after upload, so if
    // the user uploaded a file and landed here right after, poll for a
    // little while so the real-game stats card fills in without needing a
    // manual refresh.
    const interval = setInterval(refresh, 5000);
    return () => clearInterval(interval);
  }, []);

  const overallAccuracy =
    attempts.length > 0 ? (100 * attempts.filter((a) => a.is_correct).length) / attempts.length : null;
  const worst = [...weakness].sort((a, b) => {
    const accA = a.attempts > 0 ? a.correct / a.attempts : 1;
    const accB = b.attempts > 0 ? b.correct / b.attempts : 1;
    return accA - accB;
  })[0];
  const hasHandStats = (stats?.sample_size ?? 0) > 0;

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Dashboard</h1>
          <p>Your training performance and real-game stats, in one place.</p>
        </div>
        <Link to="/trainer" className="btn btn-primary">
          Start a drill
        </Link>
      </div>

      {loading ? (
        <div className="spinner" />
      ) : (
        <>
          <div className="grid grid-4" style={{ marginBottom: 24 }}>
            <div className="stat">
              <div className="stat-label">Drill accuracy</div>
              <div className="stat-value">{overallAccuracy !== null ? `${overallAccuracy.toFixed(0)}%` : "--"}</div>
              <div className="stat-sub">{attempts.length} attempts logged</div>
            </div>
            <div className="stat">
              <div className="stat-label">Hands analyzed</div>
              <div className="stat-value">{stats?.sample_size ?? 0}</div>
              <div className="stat-sub">from uploaded hand histories</div>
            </div>
            <div className="stat">
              <div className="stat-label">VPIP / PFR</div>
              <div className="stat-value">
                {pct(stats?.vpip, 0)} / {pct(stats?.pfr, 0)}
              </div>
              <div className="stat-sub">voluntarily in / raised preflop</div>
            </div>
            <div className="stat">
              <div className="stat-label">Net bb/100</div>
              <div className={"stat-value " + ((stats?.net_bb_per_100 ?? 0) >= 0 ? "ev-positive" : "ev-negative")}>
                {stats?.net_bb_per_100 !== null && stats?.net_bb_per_100 !== undefined ? stats.net_bb_per_100.toFixed(1) : "--"}
              </div>
              <div className="stat-sub">across analyzed hands</div>
            </div>
          </div>

          <div className="card" style={{ marginBottom: 24 }}>
            <h3>Real-game stats</h3>
            {!hasHandStats ? (
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 16 }}>
                <p style={{ margin: 0 }}>
                  No hand history analyzed yet. Upload a hand history export and these fill in automatically (usually
                  within a few seconds).
                </p>
                <Link to="/hands" className="btn" style={{ flexShrink: 0 }}>
                  Upload hands
                </Link>
              </div>
            ) : (
              <div className="grid grid-4">
                <StatCell label="3-bet %" value={pct(stats?.three_bet, 1)} />
                <StatCell label="Fold to 3-bet" value={pct(stats?.fold_to_three_bet, 1)} />
                <StatCell label="C-bet flop" value={pct(stats?.cbet_flop, 1)} />
                <StatCell label="Fold to flop c-bet" value={pct(stats?.fold_to_cbet_flop, 1)} />
                <StatCell label="WTSD" value={pct(stats?.wtsd, 1)} />
                <StatCell label="Won at showdown" value={pct(stats?.won_at_showdown, 1)} />
                <StatCell label="Aggression factor" value={stats?.aggression_factor != null ? stats.aggression_factor.toFixed(2) : "--"} />
                <StatCell label="Sample size" value={String(stats?.sample_size ?? 0)} />
              </div>
            )}
          </div>

          <div className="grid grid-2">
            <div className="card">
              <h3>Weakest category</h3>
              {worst ? (
                <>
                  <h2 style={{ marginTop: 8 }}>{CATEGORY_LABELS[worst.category] ?? worst.category}</h2>
                  <p>
                    {worst.attempts > 0 ? `${((100 * worst.correct) / worst.attempts).toFixed(0)}% correct` : "Not drilled yet"}{" "}
                    over {worst.attempts} attempts, averaging {worst.avg_ev_loss_bb.toFixed(2)}bb lost per mistake.
                  </p>
                  <Link to="/trainer" className="btn">
                    Drill this category
                  </Link>
                </>
              ) : (
                <p>Run a few drills and your weakest spot will show up here.</p>
              )}
            </div>

            <div className="card">
              <h3>Recent attempts</h3>
              {attempts.length === 0 ? (
                <p>No drills answered yet.</p>
              ) : (
                <table>
                  <thead>
                    <tr>
                      <th>Result</th>
                      <th>EV lost</th>
                      <th>When</th>
                    </tr>
                  </thead>
                  <tbody>
                    {attempts.slice(0, 6).map((a) => (
                      <tr key={a.id}>
                        <td>
                          <span className={"badge " + (a.is_correct ? "badge-positive" : "badge-negative")}>
                            {a.is_correct ? "Correct" : "Wrong"}
                          </span>
                        </td>
                        <td className="mono">{a.ev_loss_bb.toFixed(2)}bb</td>
                        <td className="muted">{new Date(a.answered_at).toLocaleString()}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}

function StatCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="stat">
      <div className="stat-label">{label}</div>
      <div className="stat-value" style={{ fontSize: 18 }}>
        {value}
      </div>
    </div>
  );
}
