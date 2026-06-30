import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { api, ApiError } from "../api/client";
import type { HandHistoryRecord, PlayerStats } from "../api/types";

function pct(v: number | null | undefined): string {
  if (v === null || v === undefined) return "--";
  return `${v.toFixed(0)}%`;
}

export default function HandHistory() {
  const [files, setFiles] = useState<HandHistoryRecord[]>([]);
  const [stats, setStats] = useState<PlayerStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pasted, setPasted] = useState("");

  async function refresh() {
    const [filesRes, statsRes] = await Promise.all([api.listHandHistories(), api.statsOverview()]);
    setFiles(filesRes);
    setStats(statsRes);
  }

  useEffect(() => {
    refresh().finally(() => setLoading(false));
    const interval = setInterval(refresh, 4000);
    return () => clearInterval(interval);
  }, []);

  async function onFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    setUploading(true);
    setError(null);
    try {
      const text = await file.text();
      await api.uploadHandHistory(text, file.name);
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Upload failed");
    } finally {
      setUploading(false);
      e.target.value = "";
    }
  }

  async function uploadPasted() {
    if (!pasted.trim()) return;
    setUploading(true);
    setError(null);
    try {
      await api.uploadHandHistory(pasted, "pasted.txt");
      setPasted("");
      await refresh();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Upload failed");
    } finally {
      setUploading(false);
    }
  }

  const anyParsing = files.some((f) => f.status === "pending" || f.status === "parsing");

  return (
    <div>
      <div className="page-header">
        <div>
          <h1>Hand History</h1>
          <p>Upload exports from PokerStars, GGPoker, 888poker, PartyPoker, or WPN sites (Bovada / Ignition / ACR).</p>
        </div>
      </div>

      {!loading && (
        <div className="card" style={{ marginBottom: 20 }}>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <h3 style={{ margin: 0 }}>Your stats so far</h3>
            <Link to="/" className="btn" style={{ flexShrink: 0 }}>
              Full breakdown
            </Link>
          </div>
          <div className="grid grid-4" style={{ marginTop: 16 }}>
            <div className="stat">
              <div className="stat-label">Hands analyzed</div>
              <div className="stat-value">{stats?.sample_size ?? 0}</div>
            </div>
            <div className="stat">
              <div className="stat-label">VPIP</div>
              <div className="stat-value">{pct(stats?.vpip)}</div>
            </div>
            <div className="stat">
              <div className="stat-label">PFR</div>
              <div className="stat-value">{pct(stats?.pfr)}</div>
            </div>
            <div className="stat">
              <div className="stat-label">Net bb/100</div>
              <div className={"stat-value " + ((stats?.net_bb_per_100 ?? 0) >= 0 ? "ev-positive" : "ev-negative")}>
                {stats?.net_bb_per_100 != null ? stats.net_bb_per_100.toFixed(1) : "--"}
              </div>
            </div>
          </div>
          {anyParsing && <p className="muted" style={{ marginTop: 12, marginBottom: 0 }}>Parsing in the background -- this updates automatically.</p>}
        </div>
      )}

      <div className="grid grid-2">
        <div className="card">
          <h3>Upload a file</h3>
          <p>Plain-text hand history exports (.txt). The site and game (NLHE/PLO) are detected automatically.</p>
          <input type="file" accept=".txt" onChange={onFileChange} disabled={uploading} />
          <h3 style={{ marginTop: 24 }}>...or paste hands directly</h3>
          <textarea rows={6} value={pasted} onChange={(e) => setPasted(e.target.value)} placeholder="Paste raw hand history text here" />
          <button className="btn btn-primary" style={{ marginTop: 12 }} onClick={uploadPasted} disabled={uploading}>
            {uploading ? "Uploading..." : "Upload"}
          </button>
          {error && <div className="error-banner" style={{ marginTop: 16 }}>{error}</div>}
        </div>

        <div className="card">
          <h3>Uploaded files</h3>
          {loading ? (
            <div className="spinner" />
          ) : files.length === 0 ? (
            <p>No hand histories uploaded yet.</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>File</th>
                  <th>Site</th>
                  <th>Hands</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {files.map((f) => (
                  <tr key={f.id}>
                    <td>{f.original_filename ?? "pasted"}</td>
                    <td className="muted">{f.site}</td>
                    <td className="mono">{f.hand_count}</td>
                    <td>
                      <span
                        className={
                          "badge " +
                          (f.status === "parsed" ? "badge-positive" : f.status === "failed" ? "badge-negative" : "badge-warning")
                        }
                        title={f.error ?? undefined}
                      >
                        {f.status}
                      </span>
                      {f.status === "parsed" && f.hand_count === 0 && (
                        <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>
                          File parsed but found 0 hands -- the format may not be recognized yet.
                        </div>
                      )}
                      {f.status === "failed" && f.error && (
                        <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>
                          {f.error}
                        </div>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      </div>
    </div>
  );
}
