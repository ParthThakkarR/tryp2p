import React, { useEffect } from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// ── Prevent browser-style page refresh shortcuts ─────────────
// Tauri wraps a webview that passes F5 / Ctrl+R through by default.
// We intercept them in the capture phase (before the browser handles them)
// so the app never reloads and loses in-memory state.
function preventRefresh(e: KeyboardEvent) {
  const isF5 = e.key === "F5";
  const isCtrlR = (e.ctrlKey || e.metaKey) && e.key === "r";
  const isCtrlShiftR = (e.ctrlKey || e.metaKey) && e.shiftKey && e.key === "R";
  if (isF5 || isCtrlR || isCtrlShiftR) {
    e.preventDefault();
    e.stopPropagation();
  }
}

// Register in the capture phase so we intercept before any child handler.
window.addEventListener("keydown", preventRefresh, true);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
