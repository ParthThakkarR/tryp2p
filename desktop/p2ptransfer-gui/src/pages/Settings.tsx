import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/dialog";
import type { Config, ContactEntry } from "../types";

/* ── Constants ────────────────────────────────────────────── */
const CHUNK_512KB = 512 * 1024;
const CHUNK_64MB  = 64 * 1024 * 1024;

/* ── Validation helpers ───────────────────────────────────── */
function validatePort(v: string): string | null {
  const n = Number(v);
  if (!v) return "Required";
  if (!Number.isInteger(n) || n < 1024 || n > 65535)
    return "Must be a port number between 1024 and 65535";
  return null;
}

function validateChunkSize(v: string): string | null {
  const n = Number(v);
  if (!v) return "Required";
  if (isNaN(n) || n < CHUNK_512KB || n > CHUNK_64MB)
    return `Must be between ${CHUNK_512KB / 1024} KB and ${CHUNK_64MB / (1024 * 1024)} MB`;
  return null;
}

function validateCompression(v: string): string | null {
  const n = Number(v);
  if (!v) return "Required";
  if (!Number.isInteger(n) || n < 1 || n > 22)
    return "Must be an integer between 1 and 22";
  return null;
}

function validateRequired(v: string): string | null {
  return v ? null : "Required";
}

/* ── Field component ─────────────────────────────────────── */
interface FieldProps {
  id: string;
  label: string;
  hint?: string;
  value: string;
  onChange: (v: string) => void;
  error?: string | null;
  type?: "text" | "number";
  mono?: boolean;
  disabled?: boolean;
  placeholder?: string;
  suffix?: string;
  isDir?: boolean;
  onBrowse?: () => void;
}

function Field({
  id, label, hint, value, onChange, error, type = "text",
  mono, disabled, placeholder, suffix, isDir, onBrowse,
}: FieldProps) {
  const FolderIcon = () => (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
    </svg>
  );

  return (
    <div className="form-group">
      <label className="form-label" htmlFor={id}>{label}</label>
      <div className={isDir ? "input-file-row" : undefined} style={suffix ? { display: "flex", gap: "var(--sp-2)", alignItems: "center" } : undefined}>
        <input
          id={id}
          type={type}
          className={`input-field${mono ? " mono" : ""}${error ? " error" : ""}`}
          value={value}
          onChange={e => onChange(e.target.value)}
          placeholder={placeholder}
          disabled={disabled}
          style={isDir || suffix ? { flex: 1 } : undefined}
        />
        {suffix && (
          <span style={{ color: "var(--mist)", fontSize: "var(--text-sm)", whiteSpace: "nowrap" }}>
            {suffix}
          </span>
        )}
        {isDir && onBrowse && (
          <button className="btn btn-ghost" onClick={onBrowse} disabled={disabled} type="button">
            <FolderIcon />
            Browse
          </button>
        )}
      </div>
      {hint  && !error && <span className="form-hint">{hint}</span>}
      {error && <span className="form-error" role="alert">{error}</span>}
    </div>
  );
}

/* ── Section icons ───────────────────────────────────────── */
const NetworkIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="2" y="2" width="6" height="6" rx="1" />
    <rect x="16" y="2" width="6" height="6" rx="1" />
    <rect x="9" y="16" width="6" height="6" rx="1" />
    <path d="M5 8v3a2 2 0 002 2h10a2 2 0 002-2V8" />
    <line x1="12" y1="13" x2="12" y2="16" />
  </svg>
);

const ZapIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
  </svg>
);

const FolderOpenIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
    <polyline points="8 14 12 18 16 14" />
    <line x1="12" y1="18" x2="12" y2="9" />
  </svg>
);

const GlobeIcon = () => (
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="10" />
    <line x1="2" y1="12" x2="22" y2="12" />
    <path d="M12 2a15.3 15.3 0 010 20M12 2a15.3 15.3 0 000 20" />
  </svg>
);

const CheckIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

/* ── Main component ──────────────────────────────────────── */
export default function Settings() {
  const [config,   setConfig]   = useState<Partial<Config>>({});
  const [dirty,    setDirty]    = useState<Partial<Config>>({});
  const [errors,   setErrors]   = useState<Partial<Record<keyof Config, string>>>({});
  const [saving,   setSaving]   = useState(false);
  const [saved,    setSaved]    = useState(false);
  const [loadErr,  setLoadErr]  = useState<string | null>(null);

  // Contacts state
  const [contacts,       setContacts]       = useState<ContactEntry[]>([]);
  const [newContactName, setNewContactName] = useState("");
  const [newContactId,   setNewContactId]   = useState("");
  const [contactError,   setContactError]   = useState<string | null>(null);
  const [contactSaved,   setContactSaved]   = useState(false);

  useEffect(() => {
    invoke<Config>("get_config")
      .then(cfg => { setConfig(cfg); setDirty(cfg); })
      .catch(() => setLoadErr("Couldn't load configuration. The backend may be unavailable."));
    invoke<ContactEntry[]>("list_contacts")
      .then(setContacts)
      .catch(() => {});
  }, []);

  const get = (key: keyof Config): string => dirty[key] ?? config[key] ?? "";
  const set = useCallback((key: keyof Config, value: string) => {
    setDirty(d => ({ ...d, [key]: value }));
    setErrors(e => ({ ...e, [key]: undefined }));
    setSaved(false);
  }, []);

  // Validate all fields and return whether valid
  const validate = (): boolean => {
    const errs: Partial<Record<keyof Config, string>> = {};
    const tcpErr  = validatePort(get("tcp_port"));
    const discErr = validatePort(get("discovery_port"));
    const chkErr  = validateChunkSize(get("chunk_size"));
    const cmpErr  = validateCompression(get("compression_level"));
    const datErr  = validateRequired(get("data_dir"));
    const outErr  = validateRequired(get("output_dir"));

    if (tcpErr)  errs.tcp_port  = tcpErr;
    if (discErr) errs.discovery_port = discErr;
    if (chkErr)  errs.chunk_size     = chkErr;
    if (cmpErr)  errs.compression_level = cmpErr;
    if (datErr)  errs.data_dir  = datErr;
    if (outErr)  errs.output_dir = outErr;

    setErrors(errs);
    return Object.keys(errs).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;
    setSaving(true);
    setSaved(false);
    try {
      // Only send keys that changed
      const changed = Object.entries(dirty).filter(
        ([k, v]) => v !== config[k as keyof Config]
      );
      for (const [key, value] of changed) {
        await invoke("set_config", { key, value });
      }
      const updated = await invoke<Config>("get_config");
      setConfig(updated);
      setDirty(updated);
      setSaved(true);
    } catch (e: unknown) {
      setErrors({ tcp_port: typeof e === "string" ? e : "Save failed — check values and try again." });
    } finally {
      setSaving(false);
    }
  };

  const browseDir = async (key: "data_dir" | "output_dir") => {
    const selected = await open({ directory: true, multiple: false, defaultPath: get(key) || undefined });
    if (selected && !Array.isArray(selected)) set(key, selected as string);
  };

  const chunkSizeKB = Math.round(Number(get("chunk_size")) / 1024);

  if (loadErr) {
    return (
      <div className="page">
        <div className="alert alert-error" role="alert">{loadErr}</div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page-header flex items-center justify-between">
        <div>
          <h1 className="page-title">Settings</h1>
          <p className="page-subtitle">
            Configuration is written to a TOML file — changes take effect on next connection
          </p>
        </div>
      </div>

      {/* ── Contacts ── */}
      <div className="settings-section">
        <div className="settings-section-title">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" />
            <circle cx="9" cy="7" r="4" />
            <path d="M23 21v-2a4 4 0 00-3-3.87M16 3.13a4 4 0 010 7.75" />
          </svg>
          Contacts
        </div>

        <p className="text-sm text-mist mb-4">
          Add people by their Device ID — they share it once from their Receive page. After adding, you can send them files directly from the Send page.
        </p>

        {/* Add contact form */}
        <div className="settings-grid mb-4">
          <div className="form-group">
            <label className="form-label" htmlFor="contact-name">Name</label>
            <input
              id="contact-name"
              className="input-field"
              type="text"
              value={newContactName}
              onChange={e => { setNewContactName(e.target.value); setContactError(null); setContactSaved(false); }}
              placeholder="Alice"
            />
          </div>
          <div className="form-group">
            <label className="form-label" htmlFor="contact-id">Device ID</label>
            <input
              id="contact-id"
              className="input-field mono"
              type="text"
              value={newContactId}
              onChange={e => { setNewContactId(e.target.value); setContactError(null); setContactSaved(false); }}
              placeholder="Paste their Device ID here"
            />
          </div>
        </div>

        <div className="flex items-center gap-3 mb-6">
          <button
            className="btn btn-primary btn-sm"
            onClick={async () => {
              if (!newContactName.trim() || !newContactId.trim()) {
                setContactError("Both name and Device ID are required.");
                return;
              }
              try {
                await invoke("add_contact", { name: newContactName.trim(), nodeId: newContactId.trim() });
                const updated = await invoke<ContactEntry[]>("list_contacts");
                setContacts(updated);
                setNewContactName("");
                setNewContactId("");
                setContactError(null);
                setContactSaved(true);
                setTimeout(() => setContactSaved(false), 2000);
              } catch (e: unknown) {
                setContactError(typeof e === "string" ? e : "Failed to add contact.");
              }
            }}
            id="add-contact-btn"
          >
            Add contact
          </button>
          {contactSaved && (
            <span className="flex items-center gap-1 text-signal text-sm" style={{ fontWeight: 600 }}>
              <CheckIcon /> Added
            </span>
          )}
          {contactError && (
            <span className="form-error">{contactError}</span>
          )}
        </div>

        {/* Contact list */}
        {contacts.length > 0 && (
          <div className="flex-col gap-2">
            {contacts.map(c => (
              <div key={c.name} className="peer-item" style={{ display: "flex", alignItems: "center", gap: "var(--sp-3)", padding: "var(--sp-3)", background: "var(--bg-raised)", borderRadius: "var(--r-md)", border: "1px solid var(--border)" }}>
                <div style={{ width: 32, height: 32, borderRadius: "50%", background: "var(--bg-panel)", border: "1px solid var(--border)", display: "flex", alignItems: "center", justifyContent: "center", flexShrink: 0 }}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, color: "var(--ink-bright)" }}>{c.name}</div>
                  <div className="font-mono text-xs text-mist" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {c.node_id.slice(0, 16)}…{c.node_id.slice(-8)}
                  </div>
                </div>
                <button
                  className="btn btn-ghost btn-sm"
                  style={{ color: "var(--ember)" }}
                  onClick={async () => {
                    try {
                      await invoke("remove_contact", { name: c.name });
                      const updated = await invoke<ContactEntry[]>("list_contacts");
                      setContacts(updated);
                    } catch { /* silent */ }
                  }}
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}

        {contacts.length === 0 && (
          <div className="alert alert-info">
            No contacts yet. Ask someone for their Device ID (they can copy it from their Receive page) and add it above.
          </div>
        )}
      </div>

      {/* ── Network ── */}
      <div className="settings-section">
        <div className="settings-section-title">
          <NetworkIcon />
          Network
        </div>
        <div className="settings-grid">
          <Field
            id="tcp-port"
            label="TCP transfer port"
            hint="The port your device listens on for incoming file transfers (1024–65535)"
            value={get("tcp_port")}
            onChange={v => set("tcp_port", v)}
            error={errors.tcp_port}
            type="number"
            mono
            placeholder="9877"
          />
          <Field
            id="discovery-port"
            label="Discovery port"
            hint="UDP multicast port used for LAN peer discovery (1024–65535)"
            value={get("discovery_port")}
            onChange={v => set("discovery_port", v)}
            error={errors.discovery_port}
            type="number"
            mono
            placeholder="9878"
          />
        </div>
      </div>

      {/* ── Transfer ── */}
      <div className="settings-section">
        <div className="settings-section-title">
          <ZapIcon />
          Transfer
        </div>
        <div className="settings-grid">
          <Field
            id="chunk-size"
            label="Chunk size"
            hint={`Each file is split into chunks of this size for encrypted transfer (512 KB – 64 MB). Current: ${isNaN(chunkSizeKB) ? "—" : chunkSizeKB + " KB"}`}
            value={get("chunk_size")}
            onChange={v => set("chunk_size", v)}
            error={errors.chunk_size}
            type="number"
            mono
            placeholder="2097152"
            suffix="bytes"
          />
          <div className="form-group">
            <label className="form-label" htmlFor="compression-level">
              Default compression level
            </label>
            <input
              id="compression-level"
              type="range"
              className={`range-slider w-full${errors.compression_level ? " error" : ""}`}
              min={1}
              max={22}
              value={Number(get("compression_level")) || 6}
              onChange={e => set("compression_level", e.target.value)}
              aria-label={`Compression level ${get("compression_level")}`}
            />
            <div className="flex justify-between mt-2">
              <span className="text-xs text-mist">1 — fastest</span>
              <span className="font-mono text-sm text-signal">Level {get("compression_level") || "—"}</span>
              <span className="text-xs text-mist">22 — smallest</span>
            </div>
            {errors.compression_level && (
              <span className="form-error">{errors.compression_level}</span>
            )}
            <span className="form-hint">
              Zstd compression. Skip-compressed formats (video, archives) are detected automatically.
            </span>
          </div>
        </div>
      </div>

      {/* ── Storage ── */}
      <div className="settings-section">
        <div className="settings-section-title">
          <FolderOpenIcon />
          Storage
        </div>
        <div className="settings-grid">
          <Field
            id="data-dir"
            label="Data directory"
            hint="Where BLIP stores its resume database and internal state"
            value={get("data_dir")}
            onChange={v => set("data_dir", v)}
            error={errors.data_dir}
            mono
            placeholder="/home/user/.local/share/blip"
            isDir
            onBrowse={() => browseDir("data_dir")}
          />
          <Field
            id="output-dir"
            label="Default output directory"
            hint="Where received files are saved (can be overridden per session in Receive)"
            value={get("output_dir")}
            onChange={v => set("output_dir", v)}
            error={errors.output_dir}
            mono
            placeholder="/home/user/Downloads"
            isDir
            onBrowse={() => browseDir("output_dir")}
          />
        </div>
      </div>

      {/* ── Relay / NAT ── */}
      <div className="settings-section">
        <div className="settings-section-title">
          <GlobeIcon />
          Relay &amp; NAT traversal
        </div>
        <div className="settings-grid">
          <Field
            id="relay-server"
            label="Relay server"
            hint="Address of a BLIP relay server for cross-network NAT traversal fallback. Leave blank to disable."
            value={get("relay_server")}
            onChange={v => set("relay_server", v)}
            mono
            placeholder="relay.example.com:9900"
          />
          <Field
            id="directory-identifier"
            label="Directory identifier"
            hint="Your unique ID for the relay directory — peers use this to reach you by name instead of IP"
            value={get("directory_identifier")}
            onChange={v => set("directory_identifier", v)}
            mono
            placeholder="yourname@relay"
          />
        </div>
      </div>

      {/* ── Save ── */}
      <div className="flex items-center gap-4" style={{ marginTop: "var(--sp-4)", paddingTop: "var(--sp-6)", borderTop: "1px solid var(--border)" }}>
        <button
          className="btn btn-primary btn-lg"
          onClick={handleSave}
          disabled={saving}
          id="settings-save-btn"
        >
          {saving ? "Saving…" : "Save settings"}
        </button>
        {saved && (
          <div className="flex items-center gap-2 text-signal text-sm" style={{ fontWeight: 600 }}>
            <CheckIcon />
            Saved
          </div>
        )}
      </div>
    </div>
  );
}
