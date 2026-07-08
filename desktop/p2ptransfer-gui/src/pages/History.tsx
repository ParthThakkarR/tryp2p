import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

interface Transfer {
  id: string;
  file_path: string;
  peer_addr: string;
  file_size: number;
  bytes_transferred: number;
  status: string;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function History() {
  const [transfers, setTransfers] = useState<Transfer[]>([]);
  const [filter, setFilter] = useState("all");

  useEffect(() => {
    invoke<Transfer[]>("list_transfers")
      .then(setTransfers)
      .catch(() => {});
  }, []);

  const filtered = transfers.filter((t) => {
    if (filter === "active") return t.status !== "Completed" && t.status !== "Failed";
    if (filter === "completed") return t.status === "Completed";
    if (filter === "failed") return t.status === "Failed";
    return true;
  });

  return (
    <div>
      <h1>Transfer History</h1>

      <div style={{ marginBottom: "1rem", display: "flex", gap: "0.5rem" }}>
        {["all", "active", "completed", "failed"].map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              padding: "0.25rem 0.75rem",
              background: filter === f ? "#0066cc" : "#e0e0e0",
              color: filter === f ? "#fff" : "#333",
              border: "none",
              borderRadius: 4,
              cursor: "pointer",
            }}
          >
            {f.charAt(0).toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {filtered.length === 0 ? (
        <p style={{ color: "#888" }}>No transfers found.</p>
      ) : (
        <table style={{ width: "100%", background: "#fff", borderRadius: 8, borderCollapse: "collapse", overflow: "hidden" }}>
          <thead>
            <tr style={{ background: "#1a1a2e", color: "#fff" }}>
              <th style={{ padding: "0.75rem", textAlign: "left" }}>ID</th>
              <th style={{ padding: "0.75rem", textAlign: "left" }}>File</th>
              <th style={{ padding: "0.75rem", textAlign: "left" }}>Peer</th>
              <th style={{ padding: "0.75rem", textAlign: "right" }}>Size</th>
              <th style={{ padding: "0.75rem", textAlign: "right" }}>Progress</th>
              <th style={{ padding: "0.75rem", textAlign: "left" }}>Status</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((t) => (
              <tr key={t.id} style={{ borderBottom: "1px solid #eee" }}>
                <td style={{ padding: "0.75rem", fontFamily: "monospace", fontSize: "0.85em" }}>
                  {t.id.substring(0, 8)}
                </td>
                <td style={{ padding: "0.75rem" }}>{t.file_path.split(/[/\\]/).pop()}</td>
                <td style={{ padding: "0.75rem" }}>{t.peer_addr}</td>
                <td style={{ padding: "0.75rem", textAlign: "right" }}>{formatBytes(t.file_size)}</td>
                <td style={{ padding: "0.75rem", textAlign: "right" }}>
                  {t.file_size > 0 ? `${Math.round((t.bytes_transferred / t.file_size) * 100)}%` : "-"}
                </td>
                <td style={{ padding: "0.75rem" }}>{t.status}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

export default History;
