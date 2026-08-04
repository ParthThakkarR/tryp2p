import { useState, useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import { appWindow } from "@tauri-apps/api/window";
import { useTransfer } from "../TransferContext";
import { formatSpeed } from "../utils";

export default function OverlayReceive() {
  const [searchParams] = useSearchParams();
  const reqId = searchParams.get("id") || "";
  const [downloadDir, setDownloadDir] = useState<string>("");
  const [accepted, setAccepted] = useState(false);
  const [rejected, setRejected] = useState(false);

  const { activeReceiveProgress, receiveSpeed, receiveError, receiveComplete, cancelTransfer, isPaused, pauseTransfer, resumeTransfer, wasCancelled, setWasCancelled, activeReceiveRequestId } = useTransfer();

  // If we can get default download dir
  useState(() => {
    invoke<string>("get_default_download_dir").then(setDownloadDir).catch(() => {});
  });

  // Close this overlay window when the transfer is cancelled from either side
  useEffect(() => {
    if (wasCancelled) {
      setWasCancelled(false);
      appWindow.close();
    }
  }, [wasCancelled]);

  useEffect(() => {
    if (receiveComplete) {
      setTimeout(() => appWindow.close(), 1500);
    }
  }, [receiveComplete]);

  const handleAccept = async () => {
    if (!reqId) return;
    try {
      await invoke("set_output_dir", { dir: downloadDir });
      await invoke("respond_to_transfer", {
        requestId: reqId,
        accept: true,
      });
      setAccepted(true);
    } catch { /* silent */ }
  };

  const handleReject = async () => {
    if (!reqId) return;
    try {
      await invoke("respond_to_transfer", {
        requestId: reqId,
        accept: false,
      });
      setRejected(true);
      setTimeout(() => appWindow.close(), 1000);
    } catch { /* silent */ }
  };

  const togglePause = async () => {
    // Prefer the backend-registered request ID over the URL param (TCP path compatibility)
    const id = activeReceiveRequestId || reqId;
    if (id) {
      if (isPaused) {
        await resumeTransfer(id);
      } else {
        await pauseTransfer(id);
      }
    }
  };

  const handleCancel = async () => {
    const id = activeReceiveRequestId || reqId;
    if (id) await cancelTransfer(id);
  };

  const pickDir = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: downloadDir || undefined,
    });
    if (selected && !Array.isArray(selected)) {
      setDownloadDir(selected);
    }
  };

  const pct = receiveComplete ? 100 : (activeReceiveProgress && activeReceiveProgress.total > 0
    ? Math.round((activeReceiveProgress.sent / activeReceiveProgress.total) * 100)
    : 0);

  return (
    <div className="overlay-window">
      <div className="overlay-glass-panel overlay-single-pane">
        
        {/* Removed custom close button in favor of native titlebar */}

        {/* Animated Icon */}
        <div className="overlay-icon-container">
          <div className={`overlay-icon-circle large ${accepted && pct < 100 ? 'active icon-ring-pulse' : rejected ? 'error' : ''}`}>
            {rejected ? (
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>
            ) : accepted && pct === 100 ? (
              <svg viewBox="0 0 24 24" fill="none" stroke="var(--signal)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>
            ) : (
              <svg viewBox="0 0 24 24" fill="none" stroke={accepted ? 'var(--signal)' : 'currentColor'} strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
            )}
          </div>
        </div>

        {/* Text Content */}
        <div style={{ marginBottom: '2rem' }}>
          <h2 className="overlay-title">
            {rejected ? "Transfer Rejected" : accepted && pct === 100 ? "Transfer Complete" : "Incoming File"}
          </h2>
          {!accepted && !rejected && (
            <p className="overlay-subtitle">
              Someone on your network wants to send you a file via P2P.
            </p>
          )}
        </div>

        {/* Dynamic State Area */}
        <div className="overlay-card animate-slide-up">
          
          {!accepted && !rejected && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '1rem' }}>
              <div>
                <label className="overlay-section-title" style={{ marginBottom: '0.375rem' }}>Save to Directory</label>
                <div className="input-file-row">
                  <input 
                    type="text" 
                    className="input-field mono" 
                    style={{ flex: 1, fontSize: 'var(--text-sm)' }}
                    value={downloadDir} 
                    readOnly
                    placeholder="Select a folder…"
                  />
                  <button
                    className="btn btn-ghost"
                    onClick={pickDir}
                    style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', whiteSpace: 'nowrap' }}
                    title="Browse for output directory"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                      <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
                    </svg>
                    Browse
                  </button>
                </div>
              </div>
              
              <div style={{ display: 'flex', gap: '0.75rem', marginTop: '0.5rem' }}>
                <button className="btn btn-ghost" style={{ flex: 1, borderColor: 'rgba(255,90,60,0.2)', color: 'var(--ember)' }} onClick={handleReject}>Reject</button>
                <button className="btn btn-primary" style={{ flex: 1, boxShadow: '0 4px 14px rgba(0,229,160,0.2)' }} onClick={handleAccept}>Accept</button>
              </div>
            </div>
          )}

          {accepted && (
            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end', marginBottom: '0.5rem' }}>
                <span style={{ fontSize: 'var(--text-sm)', color: 'var(--ink-bright)', fontWeight: 500 }}>
                  {pct === 100 ? "Saved successfully" : "Receiving..."}
                </span>
                <span style={{ fontSize: 'var(--text-xl)', fontFamily: 'var(--font-display)', fontWeight: 700, color: 'var(--signal)' }}>
                  {pct}%
                </span>
              </div>
              
              <div className="overlay-progress-bar-container">
                <div 
                  className="overlay-progress-bar-fill" 
                  style={{ width: `${pct}%` }}
                ></div>
              </div>

              {pct < 100 && (
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)', color: 'var(--mist)', marginTop: '0.75rem' }}>
                  <span>Encrypted P2P Stream</span>
                  <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
                    <span style={{ color: 'var(--ink-bright)' }}>{formatSpeed(receiveSpeed)}</span>
                    <button 
                      onClick={togglePause} 
                      style={{ background: 'transparent', border: 'none', color: 'var(--mist)', cursor: 'pointer', fontSize: 'var(--text-xs)', textDecoration: 'underline' }}
                    >
                      {isPaused ? "Resume" : "Pause"}
                    </button>
                    <button 
                      onClick={handleCancel} 
                      style={{ background: 'transparent', border: 'none', color: 'var(--ember)', cursor: 'pointer', fontSize: 'var(--text-xs)', textDecoration: 'underline' }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {rejected && (
            <div style={{ textAlign: 'center', fontSize: 'var(--text-sm)', color: 'var(--mist)' }}>
              You declined the incoming transfer.
            </div>
          )}

          {receiveError && (
            <div style={{ fontSize: 'var(--text-xs)', color: 'var(--ember)', textAlign: 'center', marginTop: '1rem', padding: '0.5rem', background: 'rgba(255,90,60,0.1)', borderRadius: 'var(--r-md)', border: '1px solid rgba(255,90,60,0.2)', wordBreak: 'break-word' }}>
              {receiveError}
            </div>
          )}
        </div>
        
      </div>
    </div>
  );
}
