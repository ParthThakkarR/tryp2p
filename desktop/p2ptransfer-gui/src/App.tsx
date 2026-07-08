import { BrowserRouter, Routes, Route, NavLink } from "react-router-dom";
import Home from "./pages/Home";
import Send from "./pages/Send";
import Receive from "./pages/Receive";
import History from "./pages/History";
import Settings from "./pages/Settings";

function App() {
  return (
    <BrowserRouter>
      <div style={{ display: "flex", minHeight: "100vh", fontFamily: "system-ui, sans-serif" }}>
        <nav style={{ width: 200, background: "#1a1a2e", color: "#fff", padding: "1rem" }}>
          <h2 style={{ marginBottom: "2rem" }}>p2ptransfer</h2>
          {["Home", "Send", "Receive", "History", "Settings"].map((page) => (
            <NavLink
              key={page}
              to={page === "Home" ? "/" : `/${page.toLowerCase()}`}
              style={({ isActive }) => ({
                display: "block",
                padding: "0.5rem 1rem",
                color: isActive ? "#00d4ff" : "#ccc",
                textDecoration: "none",
                borderRadius: 4,
                marginBottom: 4,
                background: isActive ? "rgba(0,212,255,0.1)" : "transparent",
              })}
            >
              {page}
            </NavLink>
          ))}
        </nav>
        <main style={{ flex: 1, padding: "2rem", background: "#f5f5f5" }}>
          <Routes>
            <Route path="/" element={<Home />} />
            <Route path="/send" element={<Send />} />
            <Route path="/receive" element={<Receive />} />
            <Route path="/history" element={<History />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
