import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

interface Peer {
  name: string;
  addr: string;
  last_seen: number;
}

interface Transfer {
  id: string;
  file_path: string;
  peer_addr: string;
  file_size: number;
  bytes_transferred: number;
  status: string;
}

function Home() {
  const [peers, setPeers] = useState<Peer[]>([]);
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [backendOk, setBackendOk] = useState(false);

  useEffect(() => {
    invoke<string>("ping")
      .then(() => setBackendOk(true))
      .catch(() => setBackendOk(false));

    invoke<Peer[]>("list_peers")
      .then(setPeers)
      .catch(() => {});

    invoke<Transfer[]>("list_transfers")
      .then(setTransfers)
      .catch(() => {});
  }, []);

  const activeTransfers = transfers.filter(
    (t) => t.status !== "Completed" && t.status !== "Failed"
  );

  return (
    <div>
      <h1>Dashboard</h1>
      <p style={{ color: backendOk ? "green" : "red" }}>
        Backend: {backendOk ? "Connected" : "Disconnected"}
      </p>

      <div style={{ display: "flex", gap: "2rem", marginTop: "1rem" }}>
        <div style={{ flex: 1, background: "#fff", padding: "1rem", borderRadius: 8, boxShadow: "0 1px 3px rgba(0,0,0,0.1)" }}>
          <h2>Discovered Peers</h2>
          {peers.length === 0 ? (
            <p style={{ color: "#888" }}>No peers discovered. Run discovery on the Send page.</p>
          ) : (
            <ul>
              {peers.map((p) => (
                <li key={p.addr}>
                  {p.name} @ {p.addr}
                </li>
              ))}
            </ul>
          )}
        </div>

        <div style={{ flex: 1, background: "#fff", padding: "1rem", borderRadius: 8, boxShadow: "0 1px 3px rgba(0,0,0,0.1)" }}>
          <h2>Active Transfers</h2>
          {activeTransfers.length === 0 ? (
            <p style={{ color: "#888" }}>No active transfers.</p>
          ) : (
            <ul>
              {activeTransfers.map((t) => (
                <li key={t.id}>
                  {t.file_path} → {t.peer_addr} ({t.status})
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}

export default Home;
