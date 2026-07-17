import { BrowserRouter, Routes, Route, NavLink } from "react-router-dom";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import Home from "./pages/Home";
import Send from "./pages/Send";
import Receive from "./pages/Receive";
import History from "./pages/History";
import Settings from "./pages/Settings";
import { TransferProvider } from "./TransferContext";
import type { IncomingTransferEvent } from "./types";

/* ── SVG icons (inline, no external dependency) ────────────── */
const Icons = {
  Home: () => (
    <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
      <polyline points="9 22 9 12 15 12 15 22" />
    </svg>
  ),
  Send: () => (
    <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="22" y1="2" x2="11" y2="13" />
      <polygon points="22 2 15 22 11 13 2 9 22 2" />
    </svg>
  ),
  Receive: () => (
    <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>
  ),
  History: () => (
    <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="12 8 12 12 14 14" />
      <path d="M3.05 11a9 9 0 1 0 .5-4H1" />
      <polyline points="1 3 1 7 5 7" />
    </svg>
  ),
  Settings: () => (
    <svg className="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.07 4.93a10 10 0 0 1 0 14.14M4.93 4.93a10 10 0 0 0 0 14.14" />
      <path d="M12 2v2M12 20v2M2 12h2M20 12h2" />
    </svg>
  ),
  Lock: () => (
    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
    </svg>
  ),
};

const NAV_ITEMS = [
  { label: "Dashboard", path: "/",         icon: Icons.Home },
  { label: "Send",      path: "/send",     icon: Icons.Send },
  { label: "Receive",   path: "/receive",  icon: Icons.Receive },
  { label: "History",   path: "/history",  icon: Icons.History },
  { label: "Settings",  path: "/settings", icon: Icons.Settings },
];

/* ── Format helpers ───────────────────────────────────────── */
function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i];
}

/* ── Incoming transfer popup ─────────────────────────────── */
function TransferPopup({
  transfer,
  onAccept,
  onReject,
}: {
  transfer: IncomingTransferEvent;
  onAccept: () => void;
  onReject: () => void;
}) {
  return (
    <div className="transfer-popup-overlay">
      <div className="transfer-popup">
        <div className="transfer-popup-header">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--signal)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <span>Incoming Transfer</span>
        </div>

        <div className="transfer-popup-body">
          <div className="transfer-popup-sender">
            <span className="text-mist text-sm">From</span>
            <span className="text-ink-bright">{transfer.sender_name}</span>
          </div>
          <div className="transfer-popup-file">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--ink)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
              <polyline points="14 2 14 8 20 8" />
            </svg>
            <div>
              <div className="text-ink-bright" style={{ fontWeight: 600 }}>{transfer.file_name}</div>
              <div className="text-mist text-sm">{formatSize(transfer.file_size)}</div>
            </div>
          </div>
        </div>

        <div className="transfer-popup-actions">
          <button className="btn btn-ghost" onClick={onReject} id="reject-transfer-btn">
            Reject
          </button>
          <button className="btn btn-primary" onClick={onAccept} id="accept-transfer-btn">
            Accept
          </button>
        </div>
      </div>
    </div>
  );
}

/* ── Main app ────────────────────────────────────────────── */
function App() {
  const [backendOk, setBackendOk] = useState<boolean | null>(null);
  const [incomingTransfer, setIncomingTransfer] = useState<IncomingTransferEvent | null>(null);

  const checkBackend = useCallback(() => {
    invoke<string>("ping")
      .then(() => setBackendOk(true))
      .catch(() => setBackendOk(false));
  }, []);

  useEffect(() => {
    checkBackend();
    const id = setInterval(checkBackend, 10_000);
    return () => clearInterval(id);
  }, [checkBackend]);

  // Listen for incoming transfer requests
  useEffect(() => {
    const unlisten = listen<IncomingTransferEvent>("transfer-incoming", (event) => {
      setIncomingTransfer(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  const handleAccept = async () => {
    if (!incomingTransfer) return;
    try {
      await invoke("respond_to_transfer", {
        requestId: incomingTransfer.request_id,
        accept: true,
      });
    } catch { /* silent */ }
    setIncomingTransfer(null);
  };

  const handleReject = async () => {
    if (!incomingTransfer) return;
    try {
      await invoke("respond_to_transfer", {
        requestId: incomingTransfer.request_id,
        accept: false,
      });
    } catch { /* silent */ }
    setIncomingTransfer(null);
  };

  const dotClass =
    backendOk === null  ? "status-dot" :
    backendOk           ? "status-dot online" :
                          "status-dot offline";

  const dotLabel =
    backendOk === null  ? "Connecting…" :
    backendOk           ? "Online — reachable" :
                          "Backend offline";

  return (
    <TransferProvider>
      <BrowserRouter>
        {/* ── Incoming transfer popup ── */}
        {incomingTransfer && (
          <TransferPopup
            transfer={incomingTransfer}
            onAccept={handleAccept}
            onReject={handleReject}
          />
        )}

        {/* ── Sidebar ── */}
        <nav className="sidebar" role="navigation" aria-label="Main navigation">
          <div className="sidebar-brand">
            <div className="sidebar-brand-name">BLIP</div>
            <div className="sidebar-brand-sub">encrypted p2p</div>
          </div>

          <div className="sidebar-nav">
            {NAV_ITEMS.map(({ label, path, icon: Icon }) => (
              <NavLink
                key={path}
                to={path}
                end={path === "/"}
                className={({ isActive }) =>
                  isActive ? "nav-link active" : "nav-link"
                }
                aria-label={label}
              >
                <Icon />
                <span>{label}</span>
              </NavLink>
            ))}
          </div>

          <div className="sidebar-footer">
            <div className="backend-status" title={dotLabel}>
              <div className={dotClass} />
              <span>{dotLabel}</span>
            </div>
            <div
              className="flex items-center gap-2 mt-2"
              style={{ fontSize: "var(--text-xs)", color: "var(--mist)", fontFamily: "var(--font-mono)" }}
            >
              <Icons.Lock />
              <span>QUIC · iroh relay</span>
            </div>
          </div>
        </nav>

        {/* ── Main content ── */}
        <main className="main-content" role="main">
          <Routes>
            <Route path="/"         element={<Home />} />
            <Route path="/send"     element={<Send />} />
            <Route path="/receive"  element={<Receive />} />
            <Route path="/history"  element={<History />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </main>
      </BrowserRouter>
    </TransferProvider>
  );
}

export default App;
