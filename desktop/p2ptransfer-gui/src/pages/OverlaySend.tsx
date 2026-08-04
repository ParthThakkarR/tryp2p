import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import { appWindow } from "@tauri-apps/api/window";
import { useTransfer } from "../TransferContext";
import type { ContactEntry } from "../types";
import { formatSpeed } from "../utils";

export default function OverlaySend() {
  const [searchParams] = useSearchParams();
  const initialFilePath = searchParams.get("file");
  const [fileList, setFileList] = useState<string[]>(initialFilePath ? [initialFilePath] : []);
  const [currentFileIndex, setCurrentFileIndex] = useState<number>(0);
  const [contacts, setContacts] = useState<ContactEntry[]>([]);
  const [selectedContact, setSelectedContact] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);

  // Add new contact state
  const [isAdding, setIsAdding] = useState(false);
  const [newName, setNewName] = useState("");
  const [newNodeId, setNewNodeId] = useState("");

  const {
    sendProgress,
    sendSpeed,
    isSending,
    setIsSending,
    sendComplete,
    setSendComplete,
    sendStatus,
    sendRejected,
    sendError,
    activeRequestId,
    setActiveRequestId,
    cancelTransfer,
    isPaused,
    pauseTransfer,
    resumeTransfer,
    wasCancelled,
    setWasCancelled,
    startSendTracking,
    resetSendState,
  } = useTransfer();

  // Reset any lingering send state when overlay opens
  useEffect(() => {
    resetSendState();
  }, []);

  // Listen for additional files passed when user selects multiple files in Explorer
  useEffect(() => {
    // 1. Rehydrate any files collected by backend during startup
    invoke<string[]>("get_pending_send_files")
      .then(pending => {
        if (pending && pending.length > 0) {
          setFileList(prev => {
            const combined = [...prev];
            for (const p of pending) {
              if (!combined.includes(p)) combined.push(p);
            }
            return combined;
          });
        }
      })
      .catch(console.error);

    // 2. Listen for real-time additions while overlay is open
    const unlisten = listen<{ path: string }>("add-file-to-send", (event) => {
      const newPath = event.payload.path;
      if (newPath) {
        setFileList(prev => prev.includes(newPath) ? prev : [...prev, newPath]);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // Close this overlay window when the transfer is cancelled from either side.
  useEffect(() => {
    if (wasCancelled) {
      appWindow.close().finally(() => setWasCancelled(false));
    }
  }, [wasCancelled]);

  const refreshContacts = () => {
    invoke<ContactEntry[]>("list_contacts").then(setContacts).catch(() => { });
  };

  useEffect(() => {
    refreshContacts();
  }, []);

  const handleAddContact = async () => {
    if (!newName || !newNodeId) return;
    try {
      await invoke("add_contact", { name: newName, nodeId: newNodeId });
      setIsAdding(false);
      setNewName("");
      setNewNodeId("");
      setSelectedContact(newName);
      refreshContacts();
    } catch (e: any) {
      setLocalError(e.toString());
    }
  };

  const togglePause = async () => {
    if (activeRequestId) {
      if (isPaused) {
        await resumeTransfer(activeRequestId);
      } else {
        await pauseTransfer(activeRequestId);
      }
    }
  };

  const handleSend = async () => {
    if (fileList.length === 0 || !selectedContact) return;
    setLocalError(null);
    setIsSending(true);

    for (let i = 0; i < fileList.length; i++) {
      setCurrentFileIndex(i);
      const filePath = fileList[i];
      startSendTracking();

      const reqId = crypto.randomUUID();
      setActiveRequestId(reqId);

      try {
        await invoke("send_to_contact", {
          requestId: reqId,
          path: filePath,
          contactName: selectedContact,
        });
      } catch (e: any) {
        if (e !== "REJECTED") {
          setLocalError(`Error sending ${filePath.split(/[/\\]/).pop()}: ${e}`);
        }
        setIsSending(false);
        return;
      }
    }
    setSendComplete(true);
    setIsSending(false);
  };

  const closeWindow = () => {
    invoke("clear_pending_send_files").catch(() => {});
    appWindow.close();
  };

  // Calculate total batch size & metadata
  const [totalBytes, setTotalBytes] = useState<number>(0);
  const [isFolder, setIsFolder] = useState<boolean>(false);
  useEffect(() => {
    if (fileList.length > 0) {
      invoke<{ path: string; name: string; size: number; is_dir: boolean }[]>("get_files_metadata", { paths: fileList })
        .then(metaList => {
          const sum = metaList.reduce((acc, curr) => acc + (curr.size || 0), 0);
          setTotalBytes(sum);
          if (metaList.length === 1 && metaList[0].is_dir) {
            setIsFolder(true);
          } else {
            setIsFolder(false);
          }
        })
        .catch(console.error);
    }
  }, [fileList]);

  const formatSize = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i];
  };

  const activeFilename = fileList[currentFileIndex]
    ? fileList[currentFileIndex].split(/[/\\]/).pop()
    : "Unknown File";

  const pct = sendProgress && sendProgress.total > 0
    ? Math.round((sendProgress.sent / sendProgress.total) * 100)
    : 0;

  return (
    <div className="overlay-window">
      <div className="overlay-glass-panel overlay-layout-split">

        {/* Left Pane - File Details */}
        <div className="overlay-pane-left">
          {fileList.length <= 1 ? (
            <>
              <div className={`overlay-icon-circle large ${isSending ? 'active icon-ring-pulse' : ''}`} style={{ marginBottom: '1rem' }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ color: sendComplete ? 'var(--signal)' : 'var(--mist)' }}>
                  {sendComplete ? (
                    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                  ) : isFolder ? (
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                  ) : (
                    <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"></path>
                  )}
                  {sendComplete ? (
                    <polyline points="22 4 12 14.01 9 11.01"></polyline>
                  ) : !isFolder ? (
                    <polyline points="13 2 13 9 20 9"></polyline>
                  ) : null}
                </svg>
              </div>

              <h2 className="overlay-title" style={{ padding: '0 1rem' }} title={fileList[0] || ""}>
                {activeFilename}
              </h2>
              <p className="overlay-subtitle" style={{ marginTop: '0.35rem' }}>
                {isFolder ? "Folder • " : ""}{totalBytes > 0 ? formatSize(totalBytes) : "Ready to transfer securely"}
              </p>
            </>
          ) : (
            <div style={{ width: '100%', display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
              {/* Stacked Cards Deck (Matching User Screenshot) */}
              <div className="overlay-stacked-deck">
                <div className="stacked-card card-3"></div>
                <div className="stacked-card card-2"></div>
                <div className={`stacked-card card-1 ${isSending ? 'active' : ''}`}>
                  <svg width="44" height="44" viewBox="0 0 24 24" fill="none" stroke="var(--solar)" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"></path>
                  </svg>
                </div>
              </div>

              <div className="overlay-items-title">
                {fileList.length} Items
              </div>
              <div className="overlay-items-subtitle" style={{ marginTop: '0.2rem', marginBottom: '0.75rem' }}>
                {formatSize(totalBytes)}
              </div>

              <div style={{
                maxHeight: '90px',
                overflowY: 'auto',
                width: '100%',
                background: 'rgba(0,0,0,0.25)',
                borderRadius: 'var(--r-md)',
                padding: '0.5rem 0.75rem',
                display: 'flex',
                flexDirection: 'column',
                gap: '0.25rem',
                fontSize: 'var(--text-xs)',
                color: 'var(--ink-bright)',
                border: '1px solid rgba(255,255,255,0.06)'
              }}>
                {fileList.map((f, idx) => (
                  <div key={f} style={{
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    color: idx === currentFileIndex && isSending ? 'var(--signal)' : 'var(--ink)',
                    fontWeight: idx === currentFileIndex && isSending ? 'bold' : 'normal'
                  }}>
                    {idx + 1}. {f.split(/[/\\]/).pop()}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Right Pane - Interaction & Status */}
        <div className="overlay-pane-right">
          <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
            <h3 className="overlay-section-title">
              <span>{isAdding ? "Add Contact" : (isSending || sendComplete ? "Transfer Status" : "Send To")}</span>
              {!isAdding && !isSending && !sendComplete && (
                <button onClick={() => setIsAdding(true)} style={{ background: 'transparent', border: 'none', color: 'var(--signal)', cursor: 'pointer' }} title="Add new contact">
                  <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="12" y1="5" x2="12" y2="19"></line><line x1="5" y1="12" x2="19" y2="12"></line></svg>
                </button>
              )}
            </h3>

            {!isSending && !sendComplete && !isAdding && (
              <div className="overlay-scrollable animate-slide-up">
                {contacts.length === 0 ? (
                  <div style={{ textAlign: 'center', color: 'var(--mist)', fontSize: 'var(--text-sm)', marginTop: '2rem' }}>
                    No contacts yet. <br /><span style={{ color: 'var(--signal)', cursor: 'pointer' }} onClick={() => setIsAdding(true)}>Add one now</span>
                  </div>
                ) : (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                    {contacts.map((c) => (
                      <div
                        key={c.name}
                        onClick={() => setSelectedContact(c.name)}
                        className={`contact-card-premium ${selectedContact === c.name ? 'selected' : ''}`}
                      >
                        <div style={{ width: '32px', height: '32px', borderRadius: '50%', background: 'rgba(255,255,255,0.05)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 'var(--text-xs)', fontWeight: 'bold', color: 'var(--ink-bright)' }}>
                          {c.name.charAt(0).toUpperCase()}
                        </div>
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--ink-bright)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.name}</div>
                          <div style={{ fontSize: 'var(--text-xs)', color: 'var(--mist)', display: 'flex', alignItems: 'center', gap: '6px' }}>
                            <span style={{ width: '6px', height: '6px', borderRadius: '50%', background: c.online ? 'var(--signal)' : 'rgba(255,255,255,0.3)', boxShadow: c.online ? '0 0 6px rgba(0,229,160,0.5)' : 'none' }}></span>
                            {c.online ? "Online" : "Offline"}
                          </div>
                          {/* Only show actionable errors, not generic offline/timeout messages */}
                          {c.online === false && c.online_error && (
                            <div style={{ fontSize: '10px', color: 'var(--ember)', marginTop: '2px' }}>
                              {c.online_error}
                            </div>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}

            {isAdding && (
              <div className="animate-slide-up" style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
                <input
                  type="text"
                  className="input-field"
                  style={{ background: 'rgba(15,21,32,0.5)', borderColor: 'rgba(255,255,255,0.08)' }}
                  placeholder="Contact Name (e.g. Alice's Phone)"
                  value={newName}
                  onChange={e => setNewName(e.target.value)}
                  autoFocus
                />
                <input
                  type="text"
                  className="input-field mono"
                  style={{ fontSize: 'var(--text-xs)', background: 'rgba(15,21,32,0.5)', borderColor: 'rgba(255,255,255,0.08)' }}
                  placeholder="P2P Node ID"
                  value={newNodeId}
                  onChange={e => setNewNodeId(e.target.value)}
                />
                <div style={{ display: 'flex', gap: '0.5rem', marginTop: '0.5rem' }}>
                  <button className="btn btn-primary" style={{ flex: 1 }} onClick={handleAddContact} disabled={!newName || !newNodeId}>Save</button>
                  <button className="btn btn-ghost" style={{ flex: 1 }} onClick={() => setIsAdding(false)}>Cancel</button>
                </div>
              </div>
            )}

            {(isSending || (sendComplete && !sendError && !localError && !sendRejected)) && (
              <div className="animate-slide-up" style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'center' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', marginBottom: '1.5rem' }}>
                  <div style={{ width: '40px', height: '40px', borderRadius: '50%', background: 'rgba(0,229,160,0.1)', border: '1px solid rgba(0,229,160,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--signal)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>
                  </div>
                  <div>
                    <div style={{ fontSize: 'var(--text-sm)', color: 'var(--mist)' }}>Sending to</div>
                    <div style={{ fontSize: 'var(--text-md)', fontWeight: 'bold', color: 'var(--ink-bright)' }}>{selectedContact}</div>
                  </div>
                </div>

                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end', marginBottom: '0.5rem' }}>
                  <span style={{ fontSize: 'var(--text-sm)', color: 'var(--ink-bright)', fontWeight: 500 }}>{sendComplete ? "Complete ✓" : sendStatus || "Transferring..."}</span>
                  <span style={{ fontSize: 'var(--text-xl)', fontFamily: 'var(--font-display)', fontWeight: 700, color: 'var(--signal)' }}>{pct}%</span>
                </div>

                <div className="overlay-progress-bar-container">
                  <div
                    className="overlay-progress-bar-fill"
                    style={{ width: `${pct}%` }}
                  ></div>
                </div>

                {isSending && (
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontSize: 'var(--text-xs)', fontFamily: 'var(--font-mono)', color: 'var(--mist)', marginTop: '0.75rem' }}>
                    <span>Encrypted P2P Stream</span>
                    <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
                      <span style={{ color: 'var(--ink-bright)' }}>{formatSpeed(sendSpeed)}</span>
                      <button
                        onClick={togglePause}
                        style={{ background: 'transparent', border: 'none', color: 'var(--mist)', cursor: 'pointer', fontSize: 'var(--text-xs)', textDecoration: 'underline' }}
                      >
                        {isPaused ? "Resume" : "Pause"}
                      </button>
                      <button
                        onClick={() => activeRequestId && cancelTransfer(activeRequestId)}
                        style={{ background: 'transparent', border: 'none', color: 'var(--ember)', cursor: 'pointer', fontSize: 'var(--text-xs)', textDecoration: 'underline' }}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* Bottom Action Area */}
            <div style={{ marginTop: '1rem', paddingTop: '1rem', borderTop: '1px solid rgba(255,255,255,0.05)' }}>
              {!isSending && !sendComplete && !isAdding && !sendError && !localError && !sendRejected && (
                <button
                  className="btn btn-primary"
                  style={{ width: '100%', boxShadow: '0 4px 14px rgba(0,229,160,0.2)' }}
                  onClick={handleSend}
                  disabled={!selectedContact}
                >
                  Start Secure Transfer
                </button>
              )}

              {sendComplete && !sendError && !localError && !sendRejected && (
                <button className="btn btn-ghost" style={{ width: '100%' }} onClick={closeWindow}>
                  Close Window
                </button>
              )}

              {sendRejected && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  <div style={{ fontSize: 'var(--text-xs)', color: 'var(--ember)', textAlign: 'center', padding: '0.5rem', background: 'rgba(255,90,60,0.1)', borderRadius: 'var(--r-md)', border: '1px solid rgba(255,90,60,0.2)' }}>
                    The receiver rejected the transfer.
                  </div>
                  <button className="btn btn-ghost" style={{ width: '100%' }} onClick={closeWindow}>
                    Close Window
                  </button>
                </div>
              )}

              {(sendError || localError) && !sendRejected && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
                  <div style={{ fontSize: 'var(--text-xs)', color: 'var(--ember)', textAlign: 'center', padding: '0.5rem', background: 'rgba(255,90,60,0.1)', borderRadius: 'var(--r-md)', border: '1px solid rgba(255,90,60,0.2)', wordBreak: 'break-word' }}>
                    {sendError || localError}
                  </div>
                  <button className="btn btn-ghost" style={{ width: '100%' }} onClick={closeWindow}>
                    Close Window
                  </button>
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
