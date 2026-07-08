import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";

function Receive() {
  const [listening, setListening] = useState(false);
  const [outputDir, setOutputDir] = useState(".");
  const [status, setStatus] = useState("");

  const startListening = async () => {
    setListening(true);
    setStatus("Listening for incoming transfers...");
    try {
      const result = await invoke<string>("start_listening", {
        outputDir,
      });
      setStatus(`Done: ${result}`);
      setListening(false);
    } catch (e) {
      setStatus(`Error: ${e}`);
      setListening(false);
    }
  };

  const stopListening = () => {
    invoke("stop_listening")
      .then(() => {
        setStatus("Listener stopped");
        setListening(false);
      })
      .catch((e) => setStatus(`Error: ${e}`));
  };

  return (
    <div>
      <h1>Receive Files</h1>
      <div style={{ background: "#fff", padding: "1.5rem", borderRadius: 8, maxWidth: 500 }}>
        <div style={{ marginBottom: "1rem" }}>
          <label>Output Directory:</label>
          <div style={{ display: "flex", gap: "0.5rem", marginTop: 4 }}>
            <input
              type="text"
              value={outputDir}
              readOnly
              placeholder="Select folder..."
              style={{ flex: 1, padding: "0.5rem" }}
            />
            <button
              onClick={async () => {
                const selected = await open({
                  directory: true,
                  multiple: false,
                  defaultPath: outputDir !== "." ? outputDir : undefined,
                });
                if (selected && !Array.isArray(selected)) {
                  setOutputDir(selected);
                }
              }}
              style={{ padding: "0.5rem 1rem", cursor: "pointer" }}
            >
              Browse
            </button>
          </div>
        </div>

        <div style={{ display: "flex", gap: "0.5rem" }}>
          <button
            onClick={startListening}
            disabled={listening}
            style={{
              padding: "0.5rem 1.5rem",
              background: listening ? "#888" : "#0066cc",
              color: "#fff",
              border: "none",
              borderRadius: 4,
              cursor: listening ? "not-allowed" : "pointer",
            }}
          >
            {listening ? "Listening..." : "Start Listening"}
          </button>

          {listening && (
            <button
              onClick={stopListening}
              style={{
                padding: "0.5rem 1.5rem",
                background: "#cc3300",
                color: "#fff",
                border: "none",
                borderRadius: 4,
                cursor: "pointer",
              }}
            >
              Stop
            </button>
          )}
        </div>

        {status && (
          <p style={{ marginTop: "1rem", color: status.startsWith("Error") ? "red" : "green" }}>
            {status}
          </p>
        )}
      </div>
    </div>
  );
}

export default Receive;
