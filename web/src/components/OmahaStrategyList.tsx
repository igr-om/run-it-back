import type { RawAction } from "../api/types";
import { actionLabel } from "../api/types";

const PALETTE = ["#5c6577", "#4c8dff", "#34d399", "#fbbf24", "#fb923c", "#f87171", "#a855f7", "#ec4899"];

function colorFor(action: RawAction, index: number): string {
  if (action === "fold") return "#3a4150";
  if (action === "check" || action === "call") return "#4c8dff";
  return PALETTE[index % PALETTE.length];
}

export default function OmahaStrategyList({
  actions,
  frequencies,
}: {
  actions: RawAction[];
  frequencies: Record<string, number[]>;
}) {
  const labels = Object.keys(frequencies).sort();

  return (
    <div>
      <p className="muted" style={{ marginTop: 0 }}>
        PLO ranges don't fit a 169-cell grid the way NLHE does -- here's the solved strategy per rank-class that was
        actually in the request's ranges.
      </p>
      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {labels.map((label) => {
          const freqs = frequencies[label];
          return (
            <div key={label}>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4 }}>
                <span className="mono" style={{ fontWeight: 700 }}>
                  {label}
                </span>
              </div>
              <div style={{ display: "flex", height: 22, borderRadius: 4, overflow: "hidden", border: "1px solid var(--border-soft)" }}>
                {freqs.map((f, i) =>
                  f > 0.005 ? (
                    <div
                      key={i}
                      title={`${actionLabel(actions[i])}: ${(f * 100).toFixed(1)}%`}
                      style={{ width: `${f * 100}%`, background: colorFor(actions[i], i) }}
                    />
                  ) : null,
                )}
              </div>
            </div>
          );
        })}
      </div>
      <div className="btn-row" style={{ marginTop: 16 }}>
        {actions.map((a, i) => (
          <span key={i} className="badge badge-neutral" style={{ borderLeft: `3px solid ${colorFor(a, i)}` }}>
            {actionLabel(a)}
          </span>
        ))}
      </div>
    </div>
  );
}
