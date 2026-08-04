import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import { useTransfer } from "../TransferContext";
import { formatSize, formatSpeed, formatEta } from "../utils";

/* ── Icons ──────────────────────────────────────────────── */
const CopyIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
    <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1" />
  </svg>
);

const CheckIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const FolderIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
  </svg>
);

/* ── Component ─────────────────────────────────────────── */
export default function Receive() {
  const [deviceId,    setDeviceId]    = useState("");
  const [deviceName,  setDeviceName]  = useState("");
  const [outputDir,   setOutputDir]   = useState("");
  const [isLoading,   setIsLoading]   = useState(true);
  const [copied,      setCopied]      = useState(false);

  const { activeReceiveProgress: activeProgress, receiveSpeed: speed, recentTransfers: recents, receiveError, savedOutputDir, setSavedOutputDir, isPaused, activeRequestId, activeReceiveRequestId, pauseTransfer, resumeTransfer, cancelTransfer } = useTransfer();

  const targetId = activeReceiveRequestId || activeRequestId;

  const togglePause = async () => {
    if (targetId) {
      if (isPaused) {
        await resumeTransfer(targetId);
      } else {
        await pauseTransfer(targetId);
      }
    }
  };

  const doCancel = async () => {
    if (targetId) {
      await cancelTransfer(targetId);
    }
  };

  // Load device ID and output dir.
  // For outputDir, we use the localStorage value first (instant, no flicker),
  // then update it from the backend in the background.
  useEffect(() => {
    // Restore output dir from localStorage immediately (non-blocking)
    if (savedOutputDir) {
      setOutputDir(savedOutputDir);
    }

    Promise.all([
      invoke<string>("get_device_id"),
      invoke<string>("get_short_device_id"),
      invoke<string>("get_device_name"),
      invoke<string>("get_default_download_dir"),
    ]).then(([id, , name, dir]) => {
      setDeviceId(id);
      setDeviceName(name);
      // Only override if the user hasn't set a custom dir
      if (!savedOutputDir) {
        setOutputDir(dir);
        setSavedOutputDir(dir);
      }
      setIsLoading(false);
    }).catch(() => setIsLoading(false));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Update output dir in backend and persist to localStorage
  const handleSetOutputDir = async (dir: string) => {
    setOutputDir(dir);
    setSavedOutputDir(dir);
    try { await invoke("set_output_dir", { dir }); } catch { /* silent */ }
  };

  const pickDir = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: outputDir || undefined,
    });
    if (selected && !Array.isArray(selected)) {
      handleSetOutputDir(selected);
    }
  };

  const copyDeviceId = () => {
    navigator.clipboard.writeText(deviceId.replace(/-/g, "")).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  const pct = activeProgress && activeProgress.total > 0
    ? Math.min(100, Math.round((activeProgress.sent / activeProgress.total) * 100))
    : 0;
  const remaining = activeProgress ? activeProgress.total - activeProgress.sent : 0;



  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Receive files</h1>
        <p className="page-subtitle">
          Your device is always reachable — contacts can send you files directly (LAN + WAN)
        </p>
      </div>

      <div style={{ maxWidth: 600 }}>

        {/* ── Device Identity ── */}
        <div className="panel mb-6">
          <div className="panel-title-sm">Your Device Key</div>
          <p className="text-sm text-mist mb-3">
            Share this key with the sender — they enter it in their Send page to reach you directly.
          </p>

          {/* Short key — prominent, compact */}
          <div style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            background: "var(--bg-raised)",
            border: "1px solid var(--border-signal)",
            borderRadius: "var(--radius)",
            padding: "12px 16px",
            boxShadow: "0 0 18px var(--signal-glow)",
          }}>
            {/* Key icon */}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--signal)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0 }}>
              <circle cx="7.5" cy="15.5" r="5.5" />
              <path d="M21 2l-9.6 9.6" />
              <path d="M15.5 7.5l3 3L22 7l-3-3" />
            </svg>

            {/* Short ID */}
            <span style={{
              fontFamily: "var(--font-mono)",
              fontSize: "1.25rem",
              fontWeight: 700,
              letterSpacing: "0.15em",
              color: "var(--signal)",
              flex: 1,
            }}>
              {isLoading ? "Loading…" : deviceId}
            </span>

            {/* Copy short key button */}
            <button
              className={`ticket-copy-btn${copied ? " copied" : ""}`}
              onClick={copyDeviceId}
              disabled={isLoading}
              style={{ position: "relative", top: "auto", right: "auto", flexShrink: 0 }}
            >
              {copied ? <><CheckIcon /> Copied</> : <><CopyIcon /> Copy</>}
            </button>
          </div>

          {/* Online status row */}
          <div className="mt-3 flex items-center gap-3" style={{ padding: "var(--sp-2) var(--sp-3)", background: "var(--bg-raised)", borderRadius: "var(--r-md)", border: "1px solid var(--border)" }}>
            <div className="wan-serving-dot" />
            <span className="text-sm text-ink">
              Online as <strong>{deviceName || "…"}</strong> — reachable via QUIC (LAN direct · WAN via relay)
            </span>
          </div>

        </div>

        {/* ── Output directory ── */}
        <div className="panel mb-6">
          <div className="panel-title-sm">Save received files to</div>
          <div className="input-file-row">
            <input
              className="input-field mono"
              type="text"
              value={isLoading ? "Loading…" : outputDir}
              readOnly
              placeholder="Select a folder…"
              aria-label="Output directory"
            />
            <button
              className="btn btn-ghost"
              onClick={pickDir}
              aria-label="Browse for output directory"
            >
              <FolderIcon />
              Browse
            </button>
          </div>
          <span className="form-hint mt-2">
            Files land here after transfer. BLAKE3 hash is computed automatically for verification.
          </span>
        </div>

        {/* ── Active transfer progress ── */}
        {activeProgress && (
          <div className="panel mb-6 active" style={{ borderColor: "var(--signal)" }}>
            <div className="flex items-center justify-between mb-4">
              <span className="panel-title-sm" style={{ marginBottom: 0 }}>Receiving file…</span>
              {speed > 0 && (
                <span className="font-mono text-sm text-signal" style={{ fontWeight: 600 }}>
                  {formatSpeed(speed)}
                </span>
              )}
            </div>

            {/* Progress bar */}
            <div style={{ width: "100%", height: 8, background: "var(--bg-raised)", borderRadius: 4, overflow: "hidden", marginBottom: "var(--sp-3)", border: "1px solid var(--border)" }}>
              <div style={{
                width: `${pct}%`,
                height: "100%",
                background: "linear-gradient(90deg, var(--signal-dim), var(--signal))",
                borderRadius: 4,
                transition: "width 0.3s ease",
                boxShadow: "0 0 12px var(--signal-glow)",
              }} />
            </div>

            {/* Stats row */}
            <div className="flex justify-between" style={{ gap: "var(--sp-4)" }}>
              <span className="text-xs text-mist">
                {formatSize(activeProgress.sent)} / {formatSize(activeProgress.total)}
              </span>
              <span className="font-mono text-sm text-signal" style={{ fontWeight: 700 }}>
                {pct}%
              </span>
              <span className="text-xs text-mist" style={{ textAlign: "right" }}>
                ETA: {formatEta(remaining, speed)}
              </span>
            </div>

            <div className="flex justify-center mt-4" style={{ gap: "var(--sp-2)" }}>
              <button className="btn btn-ghost btn-sm" onClick={togglePause}>
                {isPaused ? "▶ Resume Transfer" : "⏸ Pause Transfer"}
              </button>
              <button className="btn btn-ghost btn-sm text-red-500" onClick={doCancel} style={{ color: "var(--error)" }}>
                Cancel
              </button>
            </div>

          </div>
        )}

        {/* ── Receive-side error ── */}
        {receiveError && !activeProgress && (
          <div className="alert alert-error mb-6" role="alert">
            <strong>Transfer error:</strong> {receiveError}
          </div>
        )}



        {/* ── Recent transfers ── */}
        {recents.length > 0 && (
          <div className="panel">
            <div className="panel-title-sm">Recent incoming transfers</div>
            <div className="flex-col gap-3">
              {recents.map(r => (
                <div key={r.request_id} className="wan-result" style={{ marginTop: 0 }}>
                  <div className="wan-result-row">
                    <span className="wan-result-label">File</span>
                    <span className="wan-result-value">{r.file_path.split(/[/\\]/).pop()}</span>
                  </div>
                  <div className="wan-result-row">
                    <span className="wan-result-label">BLAKE3</span>
                    <span className="wan-result-value" style={{ fontSize: "var(--text-xs)" }}>
                      {r.blake3_hash.slice(0, 16)}…
                    </span>
                  </div>
                  <div className="wan-result-row">
                    <span className="wan-result-label">Time</span>
                    <span className="wan-result-value">{r.elapsed_secs.toFixed(2)}s</span>
                  </div>
                  {r.file_size > 0 && r.elapsed_secs > 0 && (
                    <div className="wan-result-row">
                      <span className="wan-result-label">Avg speed</span>
                      <span className="wan-result-value">{formatSpeed(r.file_size / r.elapsed_secs)}</span>
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* ── How it works note ── */}
        <div className="alert alert-info mt-6">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ flexShrink: 0, marginTop: 1 }}>
            <circle cx="12" cy="12" r="10" />
            <path d="M12 16v-4M12 8h.01" />
          </svg>
          <span>
            When someone sends you a file, a <strong>popup will appear</strong> asking if you want to accept.
            Your device is always listening via iroh QUIC — on LAN it connects directly (fast), on WAN it uses relay-assisted NAT traversal. No port forwarding needed.
          </span>
        </div>
      </div>
    </div>
  );
}
