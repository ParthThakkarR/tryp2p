import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { usePolling } from "../hooks/usePolling";
import { ChunkFlowBar } from "../components/ChunkFlowBar";
import { StatusPill } from "../components/StatusPill";
import type { Peer, Transfer } from "../types";

/* ── Helpers ──────────────────────────────────────────────── */
function relativeTime(unixSec: number): string {
  const diff = Math.floor(Date.now() / 1000) - unixSec;
  if (diff < 5)   return "just now";
  if (diff < 60)  return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

function basename(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}

/* ── Icons ────────────────────────────────────────────────── */
const PeerIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="8" r="4" />
    <path d="M20 21a8 8 0 10-16 0" />
  </svg>
);

const FileIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
    <polyline points="14 2 14 8 20 8" />
  </svg>
);

const RefreshIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="23 4 23 10 17 10" />
    <path d="M20.49 15a9 9 0 11-2.12-9.36L23 10" />
  </svg>
);

/* ── Component ───────────────────────────────────────────── */
export default function Home() {
  const [peers,     setPeers]     = useState<Peer[]>([]);
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [error,     setError]     = useState<string | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date>(new Date());

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const [p, t] = await Promise.all([
        invoke<Peer[]>("list_peers").catch(() => [] as Peer[]),
        invoke<Transfer[]>("list_transfers").catch(() => [] as Transfer[]),
      ]);
      setPeers(p);
      setTransfers(t);
      setLastRefresh(new Date());
    } catch (e) {
      setError("Couldn't reach the backend. Is the app running correctly?");
    }
  }, []);

  usePolling(refresh, 3_000);

  const active    = transfers.filter(t => ["pending", "in_progress", "paused"].includes(t.status.toLowerCase()));
  const recent    = transfers.filter(t => ["completed", "failed"].includes(t.status.toLowerCase())).slice(0, 5);
  const failedCnt = transfers.filter(t => t.status.toLowerCase() === "failed").length;

  return (
    <div className="page">
      <div className="page-header flex items-center justify-between">
        <div>
          <h1 className="page-title">Dashboard</h1>
          <p className="page-subtitle">
            Live view of your network — refreshed every 3 seconds
          </p>
        </div>
        <button
          className="btn btn-ghost btn-sm"
          onClick={refresh}
          title="Refresh now"
          aria-label="Refresh dashboard"
        >
          <RefreshIcon />
          Refresh
        </button>
      </div>

      {/* ── Error Banner ── */}
      {error && (
        <div className="alert alert-error mb-6" role="alert">
          <span>{error}</span>
        </div>
      )}

      {/* ── Stat Row ── */}
      <div className="stat-grid">
        <div className="stat-card">
          <div className="stat-label">Live Peers</div>
          <div className={`stat-value ${peers.length > 0 ? "signal" : ""}`}>
            {peers.length}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Active Transfers</div>
          <div className={`stat-value ${active.length > 0 ? "signal" : ""}`}>
            {active.length}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Failed</div>
          <div className={`stat-value ${failedCnt > 0 ? "ember" : ""}`}>
            {failedCnt}
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-label">Last Updated</div>
          <div style={{ fontFamily: "var(--font-mono)", fontSize: "var(--text-sm)", color: "var(--ink)", paddingTop: 4 }}>
            {lastRefresh.toLocaleTimeString()}
          </div>
        </div>
      </div>

      {/* ── Main Grid ── */}
      <div className="grid-2">

        {/* ── Peers Panel ── */}
        <div className="panel">
          <div className="panel-title">
            <PeerIcon />
            Peers on this network
          </div>

          {peers.length === 0 ? (
            <div className="empty-state" style={{ padding: "var(--sp-8) 0" }}>
              <svg className="empty-state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <circle cx="12" cy="8" r="4" />
                <path d="M20 21a8 8 0 10-16 0" />
                <line x1="22" y1="2" x2="2" y2="22" strokeWidth="1.5" />
              </svg>
              <p className="empty-state-title">No peers found on this network yet</p>
              <p className="empty-state-body">
                Start a listener on another device on the same LAN and it will appear here automatically.
              </p>
            </div>
          ) : (
            <div className="flex-col gap-3">
              {peers.map((peer) => (
                <div key={peer.addr} className="peer-item">
                  <div className="peer-signal-dot" />
                  <div className="peer-info">
                    <div className="peer-name">{peer.name || "Unnamed device"}</div>
                    <div className="peer-addr">{peer.addr}</div>
                  </div>
                  <div className="peer-time">{relativeTime(peer.last_seen)}</div>
                </div>
              ))}
            </div>
          )}
        </div>

        {/* ── Transfers Panel ── */}
        <div className="panel">
          <div className="panel-title">
            <FileIcon />
            Active &amp; recent transfers
          </div>

          {active.length === 0 && recent.length === 0 ? (
            <div className="empty-state" style={{ padding: "var(--sp-8) 0" }}>
              <svg className="empty-state-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                <polyline points="14 2 14 8 20 8" />
                <line x1="12" y1="18" x2="12" y2="12" />
                <line x1="9" y1="15" x2="15" y2="15" />
              </svg>
              <p className="empty-state-title">No transfers yet</p>
              <p className="empty-state-body">
                Head to Send to push a file to a peer, or Receive to open a listener.
              </p>
            </div>
          ) : (
            <div className="flex-col gap-3">
              {[...active, ...recent].map((t) => {
                const isActive = ["pending", "in_progress", "paused"].includes(t.status.toLowerCase());
                return (
                  <div key={t.id} className={`transfer-card${isActive ? " active" : ""}`}>
                    <div className="transfer-card-header">
                      <div className="transfer-file-icon">
                        <FileIcon />
                      </div>
                      <div className="transfer-info">
                        <div className="transfer-filename" title={t.file_path}>
                          {basename(t.file_path)}
                        </div>
                        <div className="transfer-peer-addr">→ {t.peer_addr}</div>
                      </div>
                      <StatusPill status={t.status} />
                    </div>
                    <ChunkFlowBar
                      fileSize={t.file_size}
                      bytesTransferred={t.bytes_transferred}
                      status={t.status}
                      showMeta={true}
                    />
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
