import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import type { ContactEntry } from "../types";
import { useTransfer } from "../TransferContext";
import { formatSize, formatSpeed, formatEta } from "../utils";

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

/* ── Helpers removed (moved to utils.ts) ──────────────────── */

/* ── Component ────────────────────────────────────────────── */
export default function Send() {
  const [contacts,        setContacts]        = useState<ContactEntry[]>([]);
  const [selectedFile,    setSelectedFile]    = useState("");
  const [selectedContact, setSelectedContact] = useState("");
  // Local-only errors (form validation) separate from transfer-error events
  const [localError, setLocalError]           = useState<string | null>(null);

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
    sendStatus,      // live phase label from Rust
    sendRejected,    // true when receiver explicitly rejected
    sendError: ctxSendError, // error from transfer-error event
    isPaused,
    resetSendState,
    startSendTracking
  } = useTransfer();

  const togglePause = async () => {
    if (!activeRequestId) return;
    try {
      if (isPaused) {
        await invoke("resume_transfer", { requestId: activeRequestId });
      } else {
        await invoke("pause_transfer", { requestId: activeRequestId });
      }
    } catch (e) { console.error(e); }
  };

  const doCancel = async () => {
    if (!activeRequestId) return;
    try {
      await invoke("cancel_transfer", { requestId: activeRequestId });
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
    const selected = await open({ multiple: false, directory: false });
    if (selected && !Array.isArray(selected)) {
      setSelectedFile(selected);
      setLocalError(null);
      resetSendState();
    }
  };

  const pickFolder = async () => {
    const selected = await open({ multiple: false, directory: true });
    if (selected && !Array.isArray(selected)) {
      setSelectedFile(selected);
      setLocalError(null);
      resetSendState();
    }
  };

  const handleSend = async () => {
    if (!selectedFile) { setLocalError("Choose a file first."); return; }
    if (!selectedContact) { setLocalError("Choose a contact to send to."); return; }

    setLocalError(null);
    startSendTracking();
    setIsSending(true);

    const reqId = crypto.randomUUID();
    setActiveRequestId(reqId);

    try {
      const hash = await invoke<string>("send_to_contact", {
        requestId: reqId,
        path: selectedFile,
        contactName: selectedContact,
      });
      setSendHash(hash);
      setSendComplete(true);
      setLocalError(null);
    } catch (e: unknown) {
      const msg = typeof e === "string" ? e : "Transfer failed.";
      // "REJECTED" is the sentinel returned by the backend when the receiver declines.
      if (msg !== "REJECTED") {
        setLocalError(msg);
      }
      // sendRejected is set by the transfer-rejected event listener in context.
    } finally {
      setIsSending(false);
    }
  };

  const reset = () => {
    setSelectedFile("");
    setSelectedContact("");
    setLocalError(null);
    resetSendState();
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
              Browse File
            </button>
            <button
              className="btn btn-ghost"
              onClick={pickFolder}
              disabled={isSending}
              aria-label="Browse for folder"
            >
              <FolderIcon />
              Folder
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
                    <div className="peer-addr" style={{ fontSize: "var(--text-xs)", display: 'flex', flexDirection: 'column', gap: '2px' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: c.online ? 'var(--signal)' : 'rgba(255,255,255,0.3)', boxShadow: c.online ? '0 0 6px rgba(0,229,160,0.5)' : 'none' }}></span>
                        {c.online ? "Online" : "Offline"}
                        <span style={{ marginLeft: '4px' }}>
                          ({c.node_id.length === 8 ? `${c.node_id.slice(0, 4)}-${c.node_id.slice(4, 8)}` : c.node_id})
                        </span>
                      </div>
                      {/* Only show actionable errors (e.g. "Peer address not found") */}
                      {c.online === false && c.online_error && (
                        <div style={{ color: 'var(--ember)', fontSize: '10px' }}>
                          {c.online_error}
                        </div>
                      )}
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
              No contacts yet. Add contacts in the <strong>Contacts</strong> tab by sharing Device IDs.
            </div>
          )}


        </div>

        {/* ── Transfer progress ── */}
        {(isSending || sendComplete) && (
          <div className={`panel mb-6${sendComplete ? "" : " active"}`}
               style={{ borderColor: sendComplete ? "var(--signal-dim)" : "var(--signal)" }}>
            <div className="flex items-center justify-between mb-4">
              <span className="panel-title-sm" style={{ marginBottom: 0 }}>
                {sendComplete ? "Transfer complete ✓"
            : (progress && progress.sent > 0) ? "Uploading…"
            : sendStatus || "Transferring…"}
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
                  <span>Avg: {formatSpeed(elapsedSecs > 0 ? progress.sent / elapsedSecs : 0)}</span>
                </div>
                
                <div className="flex justify-center mt-2" style={{ gap: "var(--sp-2)" }}>
                  <button className="btn btn-ghost btn-sm" onClick={togglePause}>
                    {isPaused ? "▶ Resume Transfer" : "⏸ Pause Transfer"}
                  </button>
                  <button className="btn btn-ghost btn-sm text-red-500" onClick={doCancel} style={{ color: "var(--error)" }}>
                    Cancel
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

        {/* ── Rejection message ── */}
        {sendRejected && (
          <div className="alert alert-error mb-4" role="alert">
            <strong>Rejected by receiver.</strong> The other device declined the transfer.
          </div>
        )}

        {/* ── Generic error (transfer events from backend) ── */}
        {ctxSendError && !sendRejected && (
          <div className="alert alert-error mb-4" role="alert">
            {ctxSendError}
          </div>
        )}

        {/* ── Local error (form validation / WAN) ── */}
        {localError && (
          <div className="alert alert-error mb-4" role="alert">
            {localError}
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
