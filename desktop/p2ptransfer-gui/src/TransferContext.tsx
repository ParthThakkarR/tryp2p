import { createContext, useContext, useState, useEffect, useRef, ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/tauri";
import type { TransferProgressEvent, TransferCompleteEvent } from "./types";

// ── Local-storage keys ────────────────────────────────────
const LS_OUTPUT_DIR    = "blip_output_dir";
const LS_ACTIVE_REQ_ID = "blip_active_request_id";

// ── Shared types ──────────────────────────────────────────
interface ProgressState {
  sent: number;
  total: number;
}

interface RecentTransfer {
  request_id: string;
  file_path: string;
  blake3_hash: string;
  elapsed_secs: number;
  file_size: number;
}

interface SendStatusPayload {
  request_id: string;
  /** "connecting" | "waiting_for_accept" | "accepted" | "transferring" | "done" */
  status: string;
}

interface TransferRejectedPayload {
  request_id: string;
}

interface TransferErrorPayload {
  request_id: string;
  error: string;
}

// ── Context shape ─────────────────────────────────────────
interface TransferContextValue {
  // Send state
  sendProgress: ProgressState | null;
  sendSpeed: number;
  sendElapsedSecs: number;
  isSending: boolean;
  setIsSending: (val: boolean) => void;
  sendComplete: boolean;
  setSendComplete: (val: boolean) => void;
  sendHash: string;
  setSendHash: (val: string) => void;
  activeRequestId: string | null;
  setActiveRequestId: (val: string | null) => void;
  /** Live phase label from the backend: "Connecting…" / "Waiting for accept…" / etc. */
  sendStatus: string;
  setSendStatus: (val: string) => void;
  /** Set when the receiver explicitly rejected the transfer. */
  sendRejected: boolean;
  /** Set when a transfer-error event fires. */
  sendError: string | null;
  isPaused: boolean;
  /** True when the user cancelled — overlay windows watch this to auto-close. */
  wasCancelled: boolean;
  setWasCancelled: (val: boolean) => void;

  // Receive state
  activeReceiveProgress: ProgressState | null;
  receiveSpeed: number;
  recentTransfers: RecentTransfer[];
  /** Error string for a receive-side error. */
  receiveError: string | null;

  // Persisted output dir (restored from localStorage on mount)
  savedOutputDir: string;
  setSavedOutputDir: (dir: string) => void;

  // Controls
  resetSendState: () => void;
  startSendTracking: () => void;
  pauseTransfer: (id: string) => Promise<void>;
  resumeTransfer: (id: string) => Promise<void>;
  cancelTransfer: (id: string) => Promise<void>;
}

const TransferContext = createContext<TransferContextValue | null>(null);

export function TransferProvider({ children }: { children: ReactNode }) {
  // ── Send state ──────────────────────────────────────────
  const [sendProgress, setSendProgress]       = useState<ProgressState | null>(null);
  const [sendSpeed, setSendSpeed]             = useState(0);
  const [sendElapsedSecs, setSendElapsedSecs] = useState(0);
  const [isSending, setIsSending]             = useState(false);
  const [sendComplete, setSendComplete]       = useState(false);
  const [sendHash, setSendHash]               = useState("");
  const [sendStatus, setSendStatus]           = useState("");
  const [sendRejected, setSendRejected]       = useState(false);
  const [sendError, setSendError]             = useState<string | null>(null);
  const [isPaused, setIsPaused]               = useState(false);
  // wasCancelled: set true when cancel fires; overlays watch this to auto-close their window
  const [wasCancelled, setWasCancelled]       = useState(false);

  // Restore activeRequestId from localStorage (survives F5 if it slips through)
  const [activeRequestId, _setActiveRequestId] = useState<string | null>(() =>
    localStorage.getItem(LS_ACTIVE_REQ_ID)
  );
  const setActiveRequestId = (val: string | null) => {
    _setActiveRequestId(val);
    if (val) {
      localStorage.setItem(LS_ACTIVE_REQ_ID, val);
    } else {
      localStorage.removeItem(LS_ACTIVE_REQ_ID);
    }
  };

  // ── Receive state ───────────────────────────────────────
  const [activeReceiveProgress, setActiveReceiveProgress] = useState<ProgressState | null>(null);
  const [receiveSpeed, setReceiveSpeed]                   = useState(0);
  const [recentTransfers, setRecentTransfers]             = useState<RecentTransfer[]>([]);
  const [receiveError, setReceiveError]                   = useState<string | null>(null);

  // ── Persisted output dir ─────────────────────────────────
  const [savedOutputDir, _setSavedOutputDir] = useState<string>(() =>
    localStorage.getItem(LS_OUTPUT_DIR) ?? ""
  );
  const setSavedOutputDir = (dir: string) => {
    _setSavedOutputDir(dir);
    localStorage.setItem(LS_OUTPUT_DIR, dir);
  };

  // ── Speed-calculation refs ───────────────────────────────
  const sendPrevBytes   = useRef(0);
  const sendPrevTime    = useRef(Date.now());
  const sendStartTime   = useRef(Date.now());

  const recvTotalBytes  = useRef(0);
  const totalPauseDuration = useRef(0);
  const pauseStartTime  = useRef<number | null>(null);

  // ── Helper actions ───────────────────────────────────────
  const startSendTracking = () => {
    // Reset ALL send state so no stale values bleed from a previous transfer.
    setSendProgress(null);
    setSendSpeed(0);
    setSendElapsedSecs(0);
    setSendRejected(false);
    setSendError(null);
    setSendHash("");
    setSendComplete(false);
    setSendStatus("");
    setIsPaused(false);
    setActiveRequestId(null);
    // wasCancelled is intentionally NOT reset here - overlays watch it to close.
    sendPrevBytes.current  = 0;
    sendPrevTime.current   = Date.now();
    sendStartTime.current  = Date.now();
    totalPauseDuration.current = 0;
    pauseStartTime.current = null;
  };

  const resetSendState = () => {
    setIsSending(false);
    setSendComplete(false);
    setSendHash("");
    setSendProgress(null);
    setSendSpeed(0);
    setSendElapsedSecs(0);
    setSendStatus("");
    setSendRejected(false);
    setSendError(null);
    setActiveRequestId(null);
    setIsPaused(false);
    // NOTE: do NOT reset wasCancelled here — overlays need to see it to close themselves
  };

  // ── Map backend status strings to human-readable labels ──
  function statusLabel(status: string): string {
    switch (status) {
      case "queued":             return "Queued — waiting for receiver to come online…";
      case "connecting":        return "Connecting to peer…";
      case "waiting_for_accept": return "Waiting for receiver to accept…";
      case "accepted":           return "Accepted — starting transfer…";
      case "transferring":       return "Uploading…";
      case "done":               return "Done";
      default:                   return status;
    }
  }

  // ── Event listeners ──────────────────────────────────────
  useEffect(() => {
    // 0. Rehydrate active transfers on mount
    import("@tauri-apps/api/tauri").then(({ invoke }) => {
      invoke<any[]>("get_active_transfers")
        .then((active) => {
          if (active.length > 0) {
            const t = active[0];
            if (t.role === "sender") {
              setIsSending(true);
              setSendStatus(statusLabel(t.status));
              setSendProgress({ sent: t.bytes_transferred, total: t.total_bytes });
              setActiveRequestId(t.request_id);
            } else if (t.role === "receiver") {
              setActiveReceiveProgress({ sent: t.bytes_transferred, total: t.total_bytes });
            }
          }
        })
        .catch(console.error);
    });

    // 1. Send progress
    const unlistenSendProgress = listen<TransferProgressEvent>("send-progress", (event) => {
      setSendSpeed(event.payload.speed_bytes_per_sec || 0);

      const now = Date.now();
      const pauseAdjustment = totalPauseDuration.current + (pauseStartTime.current ? now - pauseStartTime.current : 0);
      setSendElapsedSecs((now - sendStartTime.current - pauseAdjustment) / 1000);
      
      setSendProgress({ sent: event.payload.bytes_transferred, total: event.payload.total });
    });

    // 2. Send phase status transitions
    const unlistenSendStatus = listen<SendStatusPayload>("send-status", (event) => {
      setSendStatus(statusLabel(event.payload.status));
    });

    // 3. Transfer rejected by receiver
    const unlistenRejected = listen<TransferRejectedPayload>("transfer-rejected", (_event) => {
      setSendRejected(true);
      setSendStatus("");
      setIsSending(false);
    });

    // 4. Transfer error (sender or receiver side)
    const unlistenError = listen<TransferErrorPayload>("transfer-error", (event) => {
      const msg = event.payload.error;
      // Determine if it affects the send or receive side by checking the active request ID
      // Both sides emit this event; show in whichever state is active.
      setSendError(prev => prev ?? msg);   // don't overwrite an earlier error
      setReceiveError(prev => prev ?? msg);
      setIsSending(false);
    });

    const unlistenRecvProgress = listen<TransferProgressEvent>("transfer-progress", (event) => {
      setReceiveSpeed(event.payload.speed_bytes_per_sec || 0);

      recvTotalBytes.current = event.payload.total;
      setActiveReceiveProgress({ sent: event.payload.bytes_transferred, total: event.payload.total });
      setReceiveError(null); // clear any previous error once data starts flowing
      setActiveRequestId(event.payload.request_id);
    });

    // 7. Transfer complete
    const unlistenComplete = listen<TransferCompleteEvent>("transfer-complete", (event) => {
      setRecentTransfers(prev => [{
        request_id:   event.payload.request_id,
        file_path:    event.payload.file_path,
        blake3_hash:  event.payload.blake3_hash,
        elapsed_secs: event.payload.elapsed_secs,
        file_size:    recvTotalBytes.current,
      }, ...prev].slice(0, 10));

      setActiveReceiveProgress(null);
      setReceiveSpeed(0);
      setReceiveError(null);
      setIsPaused(false);
    });

    const unlistenPaused = listen("transfer-paused", () => {
      setIsPaused(true);
      if (!pauseStartTime.current) pauseStartTime.current = Date.now();
    });

    const unlistenResumed = listen("transfer-resumed", () => {
      setIsPaused(false);
      if (pauseStartTime.current) {
        totalPauseDuration.current += Date.now() - pauseStartTime.current;
        pauseStartTime.current = null;
      }
    });

    const unlistenCancelled = listen("transfer-cancelled", () => {
      // BUG #5 fix: reset ALL send-side state, not just isSending.
      // Without this, stale sendProgress/sendStatus/sendHash/activeRequestId
      // linger and can be briefly visible if a new send overlay opens before
      // the first event from the new transfer overwrites them.
      resetSendState();
      // Also clear the receive side in case the receiver cancelled.
      setIsPaused(false);
      setActiveReceiveProgress(null);
      setReceiveSpeed(0);
      // Signal all overlay windows to close themselves.
      // NOTE: set wasCancelled AFTER resetSendState so the overlay's useEffect
      // sees the clean state before triggering appWindow.close().
      setWasCancelled(true);
    });

    return () => {
      unlistenSendProgress.then(fn => fn());
      unlistenSendStatus.then(fn => fn());
      unlistenRejected.then(fn => fn());
      unlistenError.then(fn => fn());
      unlistenRecvProgress.then(fn => fn());
      unlistenComplete.then(fn => fn());
      unlistenPaused.then(fn => fn());
      unlistenResumed.then(fn => fn());
      unlistenCancelled.then(fn => fn());
    };
  }, []);

  const pauseTransfer = async (id: string) => {
    try { await invoke("pause_transfer", { requestId: id }); } catch (e) { console.error(e); }
  };

  const resumeTransfer = async (id: string) => {
    try { await invoke("resume_transfer", { requestId: id }); } catch (e) { console.error(e); }
  };

  const cancelTransfer = async (id: string) => {
    try { await invoke("cancel_transfer", { requestId: id }); } catch (e) { console.error(e); }
  };

  return (
    <TransferContext.Provider value={{
      sendProgress, sendSpeed, sendElapsedSecs,
      isSending, setIsSending,
      sendComplete, setSendComplete,
      sendHash, setSendHash,
      activeRequestId, setActiveRequestId,
      sendStatus, setSendStatus,
      sendRejected,
      sendError, isPaused,
      wasCancelled, setWasCancelled,
      activeReceiveProgress, receiveSpeed, recentTransfers,
      receiveError,
      savedOutputDir, setSavedOutputDir,
      resetSendState, startSendTracking,
      pauseTransfer, resumeTransfer, cancelTransfer,
    }}>
      {children}
    </TransferContext.Provider>
  );
}

export function useTransfer() {
  const ctx = useContext(TransferContext);
  if (!ctx) throw new Error("useTransfer must be used within TransferProvider");
  return ctx;
}
