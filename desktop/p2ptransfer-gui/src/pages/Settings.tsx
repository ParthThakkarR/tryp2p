import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";

function Settings() {
  const [config, setConfig] = useState<Record<string, string>>({});
  const [keyInput, setKeyInput] = useState("");
  const [valueInput, setValueInput] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => {
    invoke<Record<string, string>>("get_config")
      .then(setConfig)
      .catch(() => {});
  }, []);

  const handleSet = async () => {
    if (!keyInput || !valueInput) {
      setMessage("Enter key and value");
      return;
    }
    try {
      await invoke("set_config", { key: keyInput, value: valueInput });
      setMessage(`Set ${keyInput}=${valueInput}`);
      setKeyInput("");
      setValueInput("");
      const updated = await invoke<Record<string, string>>("get_config");
      setConfig(updated);
    } catch (e) {
      setMessage(`Error: ${e}`);
    }
  };

  return (
    <div>
      <h1>Settings</h1>
      <div style={{ display: "flex", gap: "2rem" }}>
        <div style={{ flex: 1, background: "#fff", padding: "1.5rem", borderRadius: 8 }}>
          <h2>Current Config</h2>
          {Object.keys(config).length === 0 ? (
            <p style={{ color: "#888" }}>No config loaded.</p>
          ) : (
            <table style={{ width: "100%", borderCollapse: "collapse" }}>
              <tbody>
                {Object.entries(config).map(([k, v]) => (
                  <tr key={k} style={{ borderBottom: "1px solid #eee" }}>
                    <td style={{ padding: "0.5rem", fontWeight: 500 }}>{k}</td>
                    <td style={{ padding: "0.5rem", fontFamily: "monospace" }}>{v}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div style={{ flex: 1, background: "#fff", padding: "1.5rem", borderRadius: 8 }}>
          <h2>Set Config</h2>
          <div style={{ marginBottom: "0.5rem" }}>
            <label>Key:</label>
            <input
              type="text"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              placeholder="tcp_port"
              style={{ width: "100%", padding: "0.5rem", marginTop: 4 }}
            />
          </div>
          <div style={{ marginBottom: "0.5rem" }}>
            <label>Value:</label>
            <input
              type="text"
              value={valueInput}
              onChange={(e) => setValueInput(e.target.value)}
              placeholder="9877"
              style={{ width: "100%", padding: "0.5rem", marginTop: 4 }}
            />
          </div>
          <button
            onClick={handleSet}
            style={{
              padding: "0.5rem 1.5rem",
              background: "#0066cc",
              color: "#fff",
              border: "none",
              borderRadius: 4,
              cursor: "pointer",
            }}
          >
            Apply
          </button>
          {message && (
            <p style={{ marginTop: "0.5rem", color: message.startsWith("Error") ? "red" : "green" }}>
              {message}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

export default Settings;
