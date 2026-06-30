import { useState } from "react";
import { api, ApiError } from "../api/client";
import type { DrillView, GradeResult } from "../api/types";
import { actionLabel } from "../api/types";
import { CATEGORY_LABELS } from "../api/types";
import Card from "../components/PlayingCard";

export default function Trainer() {
  const [drill, setDrill] = useState<DrillView | null>(null);
  const [result, setResult] = useState<GradeResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function loadNext() {
    setLoading(true);
    setError(null);
    setResult(null);
    try {
      const d = await api.generateDrill();
      setDrill(d);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Couldn't generate a drill");
    } finally {
      setLoading(false);
    }
  }

  async function answer(idx: number) {
    if (!drill) return;
    setSubmitting(true);
    setError(null);
    try {
      const r = await api.answerDrill(drill.id, idx);
      setResult(r);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Couldn't grade that answer");
    } finally {
      setSubmitting(false);
    }
  }

  const snap = drill?.spot_snapshot as Record<string, any> | undefined;

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Drill Trainer</h1>
          <p>Adaptive, weakness-weighted reps -- pulled from the solver, graded against it.</p>
        </div>
        <button className="btn btn-primary" onClick={loadNext} disabled={loading}>
          {loading ? "Solving..." : drill ? "Next drill" : "Start drilling"}
        </button>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {!drill && !loading && (
        <div className="card">
          <p>Click "Start drilling" and the adaptive generator will pick a spot -- weighted toward whatever you're currently worst at.</p>
        </div>
      )}

      {drill && (
        <div className="card">
          <span className="badge badge-neutral" style={{ marginBottom: 12 }}>
            {CATEGORY_LABELS[drill.category] ?? drill.category}
          </span>
          <p style={{ fontSize: 15, color: "var(--text)" }}>{snap?.description}</p>

          {snap?.board && Array.isArray(snap.board) && snap.board.length > 0 && (
            <div style={{ marginBottom: 16 }}>
              <label>Board</label>
              <div className="card-row">
                {(snap.board as string[]).map((c, i) => (
                  <Card key={i} code={c} />
                ))}
              </div>
            </div>
          )}

          <div style={{ marginBottom: 20 }}>
            <label>Your hand</label>
            <div className="card-row">
              {drill.dealt_hand.map((c, i) => (
                <Card key={i} code={c} />
              ))}
            </div>
          </div>

          <div className="muted" style={{ fontSize: 12.5, marginBottom: 16 }}>
            Stack: {String(snap?.stack_bb ?? "--")}bb
            {snap?.facing_bb ? ` -- facing ${Number(snap.facing_bb).toFixed(1)}bb` : ""}
          </div>

          {!result ? (
            <div className="btn-row">
              {drill.available_actions.map((a, i) => (
                <button key={i} className="btn btn-action" disabled={submitting} onClick={() => answer(i)}>
                  {actionLabel(a)}
                </button>
              ))}
            </div>
          ) : (
            <div>
              <div className="btn-row" style={{ marginBottom: 16 }}>
                <span className={"badge " + (result.is_correct ? "badge-positive" : "badge-negative")} style={{ fontSize: 13, padding: "6px 14px" }}>
                  {result.is_correct ? "Correct" : "Not quite"}
                </span>
                <span className="badge badge-neutral" style={{ fontSize: 13, padding: "6px 14px" }}>
                  You chose: {result.chosen_action}
                </span>
                {!result.is_correct && (
                  <span className="badge badge-warning" style={{ fontSize: 13, padding: "6px 14px" }}>
                    Best: {result.best_action}
                  </span>
                )}
                <span className={"badge " + (result.ev_loss_bb > 0.04 ? "badge-negative" : "badge-positive")} style={{ fontSize: 13, padding: "6px 14px" }}>
                  {result.ev_loss_bb > 0.001 ? `-${result.ev_loss_bb.toFixed(2)}bb` : "0.00bb lost"}
                </span>
              </div>
              <div className="card" style={{ background: "var(--surface-2)" }}>
                <p style={{ color: "var(--text)", marginBottom: 0 }}>{result.explanation}</p>
              </div>
              <button className="btn btn-primary" style={{ marginTop: 16 }} onClick={loadNext}>
                Next drill
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
