import { createContext, useContext, useState, useEffect, useRef, ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import type { TransferProgressEvent, TransferCompleteEvent } from "./types";

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
  
  // Receive state
  activeReceiveProgress: ProgressState | null;
  receiveSpeed: number;
  recentTransfers: RecentTransfer[];
  
  // Controls
  resetSendState: () => void;
  startSendTracking: () => void;
}

const TransferContext = createContext<TransferContextValue | null>(null);

export function TransferProvider({ children }: { children: ReactNode }) {
  // Send state
  const [sendProgress, setSendProgress] = useState<ProgressState | null>(null);
  const [sendSpeed, setSendSpeed] = useState(0);
  const [sendElapsedSecs, setSendElapsedSecs] = useState(0);
  const [isSending, setIsSending] = useState(false);
  const [sendComplete, setSendComplete] = useState(false);
  const [sendHash, setSendHash] = useState("");
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);

  // Receive state
  const [activeReceiveProgress, setActiveReceiveProgress] = useState<ProgressState | null>(null);
  const [receiveSpeed, setReceiveSpeed] = useState(0);
  const [recentTransfers, setRecentTransfers] = useState<RecentTransfer[]>([]);

  // Refs for precise speed calculation
  const sendPrevBytes = useRef(0);
  const sendPrevTime = useRef(Date.now());
  const sendStartTime = useRef(Date.now());

  const recvPrevBytes = useRef(0);
  const recvPrevTime = useRef(Date.now());
  const recvTotalBytes = useRef(0);

  const startSendTracking = () => {
    setSendProgress(null);
    setSendSpeed(0);
    setSendElapsedSecs(0);
    sendPrevBytes.current = 0;
    sendPrevTime.current = Date.now();
    sendStartTime.current = Date.now();
  };

  const resetSendState = () => {
    setIsSending(false);
    setSendComplete(false);
    setSendHash("");
    setSendProgress(null);
    setSendSpeed(0);
    setSendElapsedSecs(0);
    setActiveRequestId(null);
  };

  useEffect(() => {
    // 1. Send Progress
    const unlistenSendProgress = listen<TransferProgressEvent>("send-progress", (event) => {
      const now = Date.now();
      const bytesDelta = event.payload.bytes_transferred - sendPrevBytes.current;
      const timeDelta = (now - sendPrevTime.current) / 1000;

      if (timeDelta > 0.05 && bytesDelta > 0) {
        const currentSpeed = bytesDelta / timeDelta;
        setSendSpeed(prev => prev === 0 ? currentSpeed : prev * 0.7 + currentSpeed * 0.3);
      }

      sendPrevBytes.current = event.payload.bytes_transferred;
      sendPrevTime.current = now;
      setSendElapsedSecs((now - sendStartTime.current) / 1000);
      setSendProgress({ sent: event.payload.bytes_transferred, total: event.payload.total });
    });

    // 2. Receive Progress
    const unlistenRecvProgress = listen<TransferProgressEvent>("transfer-progress", (event) => {
      const now = Date.now();
      const bytesDelta = event.payload.bytes_transferred - recvPrevBytes.current;
      const timeDelta = (now - recvPrevTime.current) / 1000;

      if (timeDelta > 0.05 && bytesDelta > 0) {
        const currentSpeed = bytesDelta / timeDelta;
        setReceiveSpeed(prev => prev === 0 ? currentSpeed : prev * 0.7 + currentSpeed * 0.3);
      }

      recvPrevBytes.current = event.payload.bytes_transferred;
      recvPrevTime.current = now;
      recvTotalBytes.current = event.payload.total;
      setActiveReceiveProgress({ sent: event.payload.bytes_transferred, total: event.payload.total });
    });

    // 3. Receive Complete
    const unlistenComplete = listen<TransferCompleteEvent>("transfer-complete", (event) => {
      setRecentTransfers(prev => [{
        request_id: event.payload.request_id,
        file_path: event.payload.file_path,
        blake3_hash: event.payload.blake3_hash,
        elapsed_secs: event.payload.elapsed_secs,
        file_size: recvTotalBytes.current,
      }, ...prev].slice(0, 10));
      
      setActiveReceiveProgress(null);
      setReceiveSpeed(0);
      recvPrevBytes.current = 0;
    });

    return () => {
      unlistenSendProgress.then(fn => fn());
      unlistenRecvProgress.then(fn => fn());
      unlistenComplete.then(fn => fn());
    };
  }, []);

  return (
    <TransferContext.Provider value={{
      sendProgress, sendSpeed, sendElapsedSecs, isSending, setIsSending, sendComplete, setSendComplete, sendHash, setSendHash, activeRequestId, setActiveRequestId,
      activeReceiveProgress, receiveSpeed, recentTransfers,
      resetSendState, startSendTracking
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
