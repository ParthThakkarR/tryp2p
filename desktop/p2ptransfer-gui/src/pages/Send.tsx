import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";

interface Peer {
  name: string;
  addr: string;
  last_seen: number;
}

function Send() {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [selectedFile, setSelectedFile] = useState("");
  const [selectedPeer, setSelectedPeer] = useState("");
  const [compression, setCompression] = useState(10);
  const [status, setStatus] = useState("");

  useEffect(() => {
    invoke<Peer[]>("list_peers")
      .then(setPeers)
      .catch(() => {});
  }, []);

  const handleSend = async () => {
    if (!selectedFile || !selectedPeer) {
      setStatus("Please select a file and peer");
      return;
    }
    setStatus("Sending...");
    try {
      const result = await invoke<string>("send_file", {
        path: selectedFile,
        peer: selectedPeer,
        compression,
      });
      setStatus(`Complete: ${result}`);
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
  };

  return (
    <div>
      <h1>Send File</h1>
      <div style={{ background: "#fff", padding: "1.5rem", borderRadius: 8, maxWidth: 500 }}>
        <div style={{ marginBottom: "1rem" }}>
          <label>File Path:</label>
          <div style={{ display: "flex", gap: "0.5rem", marginTop: 4 }}>
            <input
              type="text"
              value={selectedFile}
              readOnly
              placeholder="Select a file..."
              style={{ flex: 1, padding: "0.5rem" }}
            />
            <button
              onClick={async () => {
                const selected = await open({
                  multiple: false,
                });
                if (selected && !Array.isArray(selected)) {
                  setSelectedFile(selected);
                }
              }}
              style={{ padding: "0.5rem 1rem", cursor: "pointer" }}
            >
              Browse
            </button>
          </div>
        </div>

        <div style={{ marginBottom: "1rem" }}>
          <label>Peer (IP:port):</label>
          <input
            type="text"
            value={selectedPeer}
            onChange={(e) => setSelectedPeer(e.target.value)}
            placeholder="192.168.1.100:9877"
            style={{ width: "100%", padding: "0.5rem", marginTop: 4 }}
          />
        </div>

        <div style={{ marginBottom: "1rem" }}>
          <label>Compression Level (1-22): {compression}</label>
          <input
            type="range"
            min="1"
            max="22"
            value={compression}
            onChange={(e) => setCompression(Number(e.target.value))}
            style={{ width: "100%" }}
          />
        </div>

        <button
          onClick={handleSend}
          style={{
            padding: "0.5rem 1.5rem",
            background: "#0066cc",
            color: "#fff",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
          }}
        >
          Send
        </button>

        {status && (
          <p style={{ marginTop: "1rem", color: status.startsWith("Error") ? "red" : "green" }}>
            {status}
          </p>
        )}

        {peers.length > 0 && (
          <div style={{ marginTop: "1.5rem" }}>
            <h3>Discovered Peers</h3>
            <ul>
              {peers.map((p) => (
                <li key={p.addr}>
                  <button
                    onClick={() => setSelectedPeer(p.addr)}
                    style={{ background: "none", border: "none", color: "#0066cc", cursor: "pointer", textDecoration: "underline" }}
                  >
                    {p.name} @ {p.addr}
                  </button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>
    </div>
  );
}

export default Send;
