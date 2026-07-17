import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { usePolling } from "../hooks/usePolling";
import { ChunkFlowBar, formatBytes } from "../components/ChunkFlowBar";
import { StatusPill } from "../components/StatusPill";
import type { Transfer } from "../types";

type FilterKey = "all" | "active" | "completed" | "failed";

const FILTERS: { key: FilterKey; label: string }[] = [
  { key: "all",       label: "All" },
  { key: "active",    label: "Active" },
  { key: "completed", label: "Completed" },
  { key: "failed",    label: "Failed" },
];

function basename(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

function relativeTime(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  const s  = Math.floor(ms / 1000);
  if (s < 60)   return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return new Date(iso).toLocaleDateString();
}

const EMPTY_COPY: Record<FilterKey, { title: string; body: string }> = {
  all:       { title: "No transfers yet",              body: "Send a file from the Send page or open a listener to receive one." },
  active:    { title: "Nothing transferring right now", body: "Active, pending, and paused transfers appear here while they run." },
  completed: { title: "No completed transfers",         body: "Successful transfers will appear here once they finish." },
  failed:    { title: "No failed transfers",            body: "Transfers that couldn't complete will appear here." },
};

export default function History() {
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [filter,    setFilter]    = useState<FilterKey>("all");
  const [error,     setError]     = useState<string | null>(null);
  const [loading,   setLoading]   = useState(true);

  const fetchTransfers = useCallback(async () => {
    try {
      const t = await invoke<Transfer[]>("list_transfers");
      setTransfers(t);
      setError(null);
    } catch {
      setError("Couldn't load transfer history. The backend may be unavailable.");
    } finally {
      setLoading(false);
    }
  }, []);

  usePolling(fetchTransfers, 5_000);

  const filtered = transfers.filter(t => {
    const s = t.status.toLowerCase();
    if (filter === "active")    return ["pending", "in_progress", "paused"].includes(s);
    if (filter === "completed") return s === "completed";
    if (filter === "failed")    return s === "failed";
    return true;
  });

  const empty = EMPTY_COPY[filter];

  return (
    <div className="page">
      <div className="page-header flex items-center justify-between">
        <div>
          <h1 className="page-title">Transfer history</h1>
          <p className="page-subtitle">All transfers from the resume database — refreshed every 5 seconds</p>
        </div>
        <div className="font-mono text-xs text-mist">
          {transfers.length} record{transfers.length !== 1 ? "s" : ""} total
        </div>
      </div>

      {error && (
        <div className="alert alert-error mb-6" role="alert">{error}</div>
      )}

      {/* ── Filter tabs ── */}
      <div className="filter-tabs mb-6">
        {FILTERS.map(f => (
          <button
            key={f.key}
            className={`filter-tab${filter === f.key ? " active" : ""}`}
            onClick={() => setFilter(f.key)}
            id={`filter-${f.key}`}
          >
            {f.label}
            <span style={{
              marginLeft: "6px",
              fontFamily: "var(--font-mono)",
              fontSize: "0.7em",
              opacity: 0.75,
            }}>
              {f.key === "all"
                ? transfers.length
                : transfers.filter(t => {
                    const s = t.status.toLowerCase();
                    if (f.key === "active")    return ["pending", "in_progress", "paused"].includes(s);
                    if (f.key === "completed") return s === "completed";
                    if (f.key === "failed")    return s === "failed";
                    return true;
                  }).length
              }
            </span>
          </button>
        ))}
      </div>

      {/* ── Table ── */}
      {loading ? (
        <div className="panel">
          {[...Array(4)].map((_, i) => (
            <div key={i} className="skeleton" style={{ height: 44, marginBottom: 8, borderRadius: "var(--r-md)" }} />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="panel">
          <div className="empty-state">
            <svg className="empty-state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <line x1="3" y1="9" x2="21" y2="9" />
              <line x1="9" y1="21" x2="9" y2="9" />
            </svg>
            <p className="empty-state-title">{empty.title}</p>
            <p className="empty-state-body">{empty.body}</p>
          </div>
        </div>
      ) : (
        <div className="panel" style={{ padding: 0, overflow: "hidden" }}>
          <table className="data-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>File</th>
                <th>Peer</th>
                <th>Size</th>
                <th>Progress</th>
                <th>Status</th>
                <th style={{ textAlign: "right" }}>Time</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map(t => {
                const pct = t.file_size > 0
                  ? Math.round((t.bytes_transferred / t.file_size) * 100)
                  : 0;
                return (
                  <tr key={t.id}>
                    <td className="mono" title={t.id}>
                      {t.id.substring(0, 8)}
                    </td>
                    <td>
                      <span
                        title={t.file_path}
                        style={{
                          maxWidth: 200,
                          display: "block",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          color: "var(--ink-bright)",
                          fontWeight: 500,
                        }}
                      >
                        {basename(t.file_path)}
                      </span>
                    </td>
                    <td className="mono">{t.peer_addr}</td>
                    <td className="mono" style={{ textAlign: "right" }}>
                      {formatBytes(t.file_size)}
                    </td>
                    <td style={{ minWidth: 160 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: "var(--sp-2)" }}>
                        <ChunkFlowBar
                          fileSize={t.file_size}
                          bytesTransferred={t.bytes_transferred}
                          status={t.status}
                          thumb={true}
                          showMeta={false}
                        />
                        <span className="mono text-xs text-mist" style={{ flexShrink: 0 }}>
                          {pct}%
                        </span>
                      </div>
                    </td>
                    <td><StatusPill status={t.status} /></td>
                    <td className="mono text-xs text-mist" style={{ textAlign: "right", whiteSpace: "nowrap" }}>
                      {relativeTime(t.created_at)}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
