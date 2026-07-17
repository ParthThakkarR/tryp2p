import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import type { ContactEntry } from "../types";
import { useTransfer } from "../TransferContext";

/* ── Icons ────────────────────────────────────────────────── */
const FolderIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
  </svg>
);

const CheckIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const SendIcon = () => (
  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="22" y1="2" x2="11" y2="13" />
    <polygon points="22 2 15 22 11 13 2 9 22 2" />
  </svg>
);

const UserIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
    <circle cx="12" cy="7" r="4" />
  </svg>
);

/* ── Step indicator ──────────────────────────────────────── */
type StepState = "idle" | "active" | "done";

function StepIndicator({ steps, current }: { steps: string[]; current: number }) {
  return (
    <div className="step-indicator">
      {steps.map((label, i) => {
        const state: StepState = i < current ? "done" : i === current ? "active" : "idle";
        return (
          <>
            <div key={label} className={`step ${state}`}>
              <div className="step-num">
                {state === "done" ? <CheckIcon /> : i + 1}
              </div>
              <span className="step-label">{label}</span>
            </div>
            {i < steps.length - 1 && (
              <div key={`c-${i}`} className={`step-connector${i < current ? " done" : ""}`} />
            )}
          </>
        );
      })}
    </div>
  );
}

/* ── Helpers ─────────────────────────────────────────────── */
function formatSize(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i];
}

function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "—";
  if (bytesPerSec >= 1024 * 1024 * 1024) return (bytesPerSec / (1024 * 1024 * 1024)).toFixed(1) + " GB/s";
  if (bytesPerSec >= 1024 * 1024) return (bytesPerSec / (1024 * 1024)).toFixed(1) + " MB/s";
  if (bytesPerSec >= 1024) return (bytesPerSec / 1024).toFixed(0) + " KB/s";
  return bytesPerSec.toFixed(0) + " B/s";
}

function formatEta(remainingBytes: number, bytesPerSec: number): string {
  if (bytesPerSec <= 0) return "—";
  const secs = Math.ceil(remainingBytes / bytesPerSec);
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

/* ── Component ────────────────────────────────────────────── */
export default function Send() {
  const [contacts,       setContacts]       = useState<ContactEntry[]>([]);
  const [selectedFile,   setSelectedFile]   = useState("");
  const [selectedContact, setSelectedContact] = useState("");
  const [sendError,      setSendError]      = useState<string | null>(null);
  const [sendStatus,     setSendStatus]     = useState("");
  const [wanUrl,         setWanUrl]         = useState("");
  const [isGeneratingWan,setIsGeneratingWan] = useState(false);

  const {
    sendProgress: progress,
    sendSpeed: speed,
    sendElapsedSecs: elapsedSecs,
    isSending,
    setIsSending,
    sendComplete,
    setSendComplete,
    sendHash,
    setSendHash,
    activeRequestId,
    setActiveRequestId,
    resetSendState,
    startSendTracking
  } = useTransfer();

  const [isPaused, setIsPaused] = useState(false);

  const togglePause = async () => {
    if (!activeRequestId) return;
    try {
      if (isPaused) {
        await invoke("resume_transfer", { requestId: activeRequestId });
      } else {
        await invoke("pause_transfer", { requestId: activeRequestId });
      }
      setIsPaused(!isPaused);
    } catch (e) { console.error(e); }
  };

  const currentStep = isSending || sendComplete ? 2 : selectedFile ? 1 : 0;

  // Load contacts
  useEffect(() => {
    invoke<ContactEntry[]>("list_contacts")
      .then(setContacts)
      .catch(() => {});
  }, []);



  const pickFile = async () => {
    const selected = await open({ multiple: false });
    if (selected && !Array.isArray(selected)) {
      setSelectedFile(selected);
      setSendError(null);
      resetSendState();
    }
  };

  const handleSend = async () => {
    if (!selectedFile) { setSendError("Choose a file first."); return; }
    if (!selectedContact) { setSendError("Choose a contact to send to."); return; }

    setSendError(null);
    setSendStatus("Connecting to peer…");
    startSendTracking();
    setIsSending(true);

    const reqId = crypto.randomUUID();
    setActiveRequestId(reqId);

    try {
      setSendStatus("Waiting for receiver to accept…");
      const hash = await invoke<string>("send_to_contact", {
        requestId: reqId,
        path: selectedFile,
        contactName: selectedContact,
      });
      setSendHash(hash);
      setSendComplete(true);
      setSendStatus("");
    } catch (e: unknown) {
      const msg = typeof e === "string" ? e : "Transfer failed.";
      setSendError(msg);
      setSendStatus("");
    } finally {
      setIsSending(false);
    }
  };

  const reset = () => {
    setSelectedFile("");
    setSelectedContact("");
    setSendError(null);
    setSendStatus("");
    setWanUrl("");
    setIsPaused(false);
    resetSendState();
  };

  const handleStartWan = async () => {
    if (!selectedFile) return;
    setIsGeneratingWan(true);
    setSendError(null);
    setWanUrl("");
    setSendStatus("Starting Cloudflare tunnel (may take ~10s)...");
    
    try {
      const url = await invoke<string>("start_wan_tunnel", {
        path: selectedFile
      });
      setWanUrl(url);
      setSendStatus("");
    } catch (e: any) {
      setSendError(e.toString());
      setSendStatus("");
    } finally {
      setIsGeneratingWan(false);
    }
  };

  const copyWanUrl = () => {
    navigator.clipboard.writeText(wanUrl);
  };

  const filename = selectedFile ? selectedFile.split(/[/\\]/).pop() ?? selectedFile : "";
  const pct = progress && progress.total > 0
    ? Math.min(100, Math.round((progress.sent / progress.total) * 100))
    : 0;
  const remaining = progress ? progress.total - progress.sent : 0;

  return (
    <div className="page">
      <div className="page-header">
        <h1 className="page-title">Send a file</h1>
        <p className="page-subtitle">
          Pick a file, choose a contact, send — works across any network (LAN + WAN)
        </p>
      </div>

      <StepIndicator steps={["Pick file", "Choose contact", "Transfer"]} current={currentStep} />

      <div style={{ maxWidth: 640 }}>

        {/* ── Step 0: File picker ── */}
        <div className="panel mb-6">
          <div className="panel-title-sm">① File to send</div>
          <div className="input-file-row">
            <input
              className="input-field mono"
              type="text"
              value={filename || ""}
              readOnly
              placeholder="No file selected — click Browse to choose"
              aria-label="Selected file path"
            />
            <button
              className="btn btn-ghost"
              onClick={pickFile}
              disabled={isSending}
              aria-label="Browse for file"
            >
              <FolderIcon />
              Browse
            </button>
          </div>

          {selectedFile && (
            <div className="mt-3 flex items-center gap-3" style={{ padding: "var(--sp-3)", background: "var(--bg-raised)", borderRadius: "var(--r-md)", border: "1px solid var(--border)" }}>
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--signal)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z" />
                <polyline points="14 2 14 8 20 8" />
              </svg>
              <span className="font-mono text-sm" style={{ color: "var(--ink-bright)", flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {filename}
              </span>
              <button className="btn btn-ghost btn-sm" onClick={reset} disabled={isSending}>
                Clear
              </button>
            </div>
          )}
        </div>

        {/* ── Step 1: Choose contact ── */}
        <div className="panel mb-6" style={{ opacity: selectedFile ? 1 : 0.5, transition: "opacity var(--t-base)" }}>
          <div className="panel-title-sm">② Choose contact</div>

          {contacts.length > 0 ? (
            <div className="flex-col gap-2">
              {contacts.map(c => (
                <button
                  key={c.name}
                  className={`peer-item w-full${selectedContact === c.name ? " border-signal" : ""}`}
                  style={{
                    cursor: "pointer",
                    background: selectedContact === c.name ? "var(--bg-hover)" : "var(--bg-raised)",
                    borderColor: selectedContact === c.name ? "var(--signal-dim)" : "var(--border)",
                    textAlign: "left",
                    width: "100%",
                  }}
                  onClick={() => setSelectedContact(c.name)}
                  disabled={isSending}
                  aria-pressed={selectedContact === c.name}
                >
                  <div style={{ width: 32, height: 32, borderRadius: "50%", background: "var(--bg-panel)", border: "1px solid var(--border)", display: "flex", alignItems: "center", justifyContent: "center" }}>
                    <UserIcon />
                  </div>
                  <div className="peer-info">
                    <div className="peer-name">{c.name}</div>
                    <div className="peer-addr" style={{ fontSize: "var(--text-xs)" }}>
                      {c.node_id.slice(0, 12)}…{c.node_id.slice(-6)}
                    </div>
                  </div>
                  {selectedContact === c.name && (
                    <span style={{ color: "var(--signal)", marginLeft: "auto" }}>
                      <CheckIcon />
                    </span>
                  )}
                </button>
              ))}
            </div>
          ) : (
            <div className="alert alert-info">
              No contacts yet. Add contacts in <strong>Settings → Contacts</strong> by sharing Device IDs.
            </div>
          )}

          <div className="mt-4" style={{ borderTop: "1px solid var(--border)", paddingTop: "1rem" }}>
            <div className="panel-title-sm mb-2" style={{ fontSize: "0.75rem", color: "var(--text-muted)" }}>OR: Generate High-Speed WAN Link</div>
            {wanUrl ? (
              <div className="alert alert-success">
                <div style={{ fontWeight: 600, marginBottom: "0.25rem" }}>WAN Link Ready!</div>
                <div style={{ marginBottom: "0.5rem", fontSize: "0.85rem" }}>Share this link with the receiver. They can download it instantly.</div>
                <div className="flex items-center gap-2">
                  <input type="text" className="input-field mono w-full" readOnly value={wanUrl} />
                  <button className="btn btn-primary" onClick={copyWanUrl}>Copy</button>
                </div>
              </div>
            ) : (
              <button
                className="btn btn-secondary w-full"
                onClick={handleStartWan}
                disabled={!selectedFile || isGeneratingWan || isSending}
              >
                {isGeneratingWan ? "Generating WAN Link..." : "Generate WAN Link (Cloudflare)"}
              </button>
            )}
          </div>
        </div>

        {/* ── Transfer progress ── */}
        {(isSending || sendComplete) && (
          <div className={`panel mb-6${sendComplete ? "" : " active"}`}
               style={{ borderColor: sendComplete ? "var(--signal-dim)" : "var(--signal)" }}>
            <div className="flex items-center justify-between mb-4">
              <span className="panel-title-sm" style={{ marginBottom: 0 }}>
                {sendComplete ? "Transfer complete ✓" : (progress && progress.sent > 0) ? "Uploading…" : sendStatus || "Transferring…"}
              </span>
              {!sendComplete && speed > 0 && (
                <span className="font-mono text-sm text-signal" style={{ fontWeight: 600 }}>
                  {formatSpeed(speed)}
                </span>
              )}
            </div>

            {!sendComplete && progress && (
              <>
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
                    {formatSize(progress.sent)} / {formatSize(progress.total)}
                  </span>
                  <span className="font-mono text-sm text-signal" style={{ fontWeight: 700 }}>
                    {pct}%
                  </span>
                  <span className="text-xs text-mist" style={{ textAlign: "right" }}>
                    ETA: {formatEta(remaining, speed)}
                  </span>
                </div>

                <div className="flex justify-between items-center text-sm text-mist mt-4 mb-2">
                  <span>Elapsed: {elapsedSecs.toFixed(0)}s</span>
                  <span>Avg: {formatSpeed(speed)}</span>
                </div>
                
                <div className="flex justify-center mt-2">
                  <button className="btn btn-ghost btn-sm" onClick={togglePause}>
                    {isPaused ? "▶ Resume Transfer" : "⏸ Pause Transfer"}
                  </button>
                </div>
              </>
            )}

            {sendComplete && (
              <>
                <div className="wan-result">
                  <div className="wan-result-row">
                    <span className="wan-result-label">BLAKE3 hash</span>
                    <span className="wan-result-value">{sendHash}</span>
                  </div>
                  <div className="wan-result-row">
                    <span className="wan-result-label">Time</span>
                    <span className="wan-result-value">{elapsedSecs.toFixed(2)}s</span>
                  </div>
                  {progress && elapsedSecs > 0 && (
                    <div className="wan-result-row">
                      <span className="wan-result-label">Average speed</span>
                      <span className="wan-result-value">{formatSpeed(progress.total / elapsedSecs)}</span>
                    </div>
                  )}
                </div>
                <div className="mt-4">
                  <button className="btn btn-ghost btn-sm" onClick={reset}>
                    Send another file
                  </button>
                </div>
              </>
            )}
          </div>
        )}

        {/* ── Error ── */}
        {sendError && (
          <div className="alert alert-error mb-4" role="alert">
            {sendError}
          </div>
        )}

        {/* ── Send button ── */}
        {!isSending && !sendComplete && (
          <button
            className="btn btn-primary btn-lg w-full"
            onClick={handleSend}
            disabled={!selectedFile || !selectedContact}
            id="send-btn"
          >
            <SendIcon />
            Send to {selectedContact || "…"}
          </button>
        )}
      </div>
    </div>
  );
}
