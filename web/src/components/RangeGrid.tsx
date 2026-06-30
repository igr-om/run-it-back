import { useState } from "react";
import type { RawAction } from "../api/types";
import { actionLabel } from "../api/types";

const RANKS = ["A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2"];

const PALETTE = ["#5c6577", "#4c8dff", "#34d399", "#fbbf24", "#fb923c", "#f87171", "#a855f7", "#ec4899"];

function colorFor(action: RawAction, index: number): string {
  if (action === "fold") return "#3a4150";
  if (action === "check" || action === "call") return "#4c8dff";
  return PALETTE[index % PALETTE.length];
}

function comboLabel(i: number, j: number): string {
  if (i === j) return RANKS[i] + RANKS[i];
  if (i < j) return RANKS[i] + RANKS[j] + "s";
  return RANKS[j] + RANKS[i] + "o";
}

function gradientFor(freqs: number[] | undefined, actions: RawAction[]): string {
  if (!freqs) return "var(--surface-3)";
  let cursor = 0;
  const stops: string[] = [];
  freqs.forEach((f, idx) => {
    const color = colorFor(actions[idx], idx);
    const start = cursor;
    cursor += f * 100;
    if (f > 0.001) stops.push(`${color} ${start}% ${cursor}%`);
  });
  return stops.length > 0 ? `linear-gradient(to right, ${stops.join(", ")})` : "#3a4150";
}

export default function RangeGrid({
  actions,
  frequencies,
}: {
  actions: RawAction[];
  frequencies: Record<string, number[]>;
}) {
  // Selection is click-driven and sticky -- it stays in the side panel
  // until a different cell is clicked, rather than disappearing the
  // moment the mouse leaves (hovering still gives a quick zoomed preview
  // via pure CSS, but doesn't change what's "pinned").
  const [selected, setSelected] = useState<string | null>(null);
  const selectedFreqs = selected ? frequencies[selected] : undefined;

  return (
    <div className="range-explorer-layout">
      <div className="range-grid">
        {RANKS.map((_, i) =>
          RANKS.map((_, j) => {
            const label = comboLabel(i, j);
            const freqs = frequencies[label];
            const background = gradientFor(freqs, actions);
            const isSelected = selected === label;
            return (
              <div
                key={label}
                className={"range-cell" + (isSelected ? " range-cell-selected" : "")}
                title={label}
                onClick={() => setSelected(label)}
              >
                <div className="range-cell-fill" style={{ background }} />
                <span className="range-cell-label">{label}</span>
              </div>
            );
          }),
        )}
      </div>

      <div className="range-magnifier">
        {selected ? (
          <>
            <div className="range-magnifier-label">{selected}</div>
            <div className="range-magnifier-bar" style={{ background: gradientFor(selectedFreqs, actions) }} />
            <div className="range-magnifier-breakdown">
              {selectedFreqs ? (
                actions.map((a, i) =>
                  selectedFreqs[i] > 0.001 ? (
                    <div key={i} className="range-magnifier-row">
                      <span className="range-magnifier-swatch" style={{ background: colorFor(a, i) }} />
                      <span className="range-magnifier-action">{actionLabel(a)}</span>
                      <span className="mono range-magnifier-pct">{(selectedFreqs[i] * 100).toFixed(1)}%</span>
                    </div>
                  ) : null,
                )
              ) : (
                <p className="muted" style={{ margin: 0 }}>
                  Not in either range on this board.
                </p>
              )}
            </div>
          </>
        ) : (
          <div className="range-magnifier-empty">
            <p className="muted" style={{ margin: 0 }}>
              Click any cell to pin its exact strategy here -- the grid is necessarily tiny for 169 hands at once.
            </p>
          </div>
        )}
      </div>

      <div className="btn-row range-legend">
        {actions.map((a, i) => (
          <span key={i} className="badge badge-neutral" style={{ borderLeft: `3px solid ${colorFor(a, i)}` }}>
            {actionLabel(a)}
          </span>
        ))}
      </div>
    </div>
  );
}
