import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import type { ContactEntry } from "../types";

const CheckIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const UserIcon = () => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
    <circle cx="12" cy="7" r="4" />
  </svg>
);

export default function Contacts() {
  const [contacts,       setContacts]       = useState<ContactEntry[]>([]);
  const [newContactName, setNewContactName] = useState("");
  const [newContactId,   setNewContactId]   = useState("");
  const [contactError,   setContactError]   = useState<string | null>(null);
  const [contactSaved,   setContactSaved]   = useState(false);

  useEffect(() => {
    invoke<ContactEntry[]>("list_contacts")
      .then(setContacts)
      .catch(() => {});
  }, []);

  const handleAdd = async () => {
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
  };

  const handleRemove = async (name: string) => {
    try {
      await invoke("remove_contact", { name });
      const updated = await invoke<ContactEntry[]>("list_contacts");
      setContacts(updated);
    } catch { /* silent */ }
  };

  return (
    <div className="page">
      <div className="page-header flex items-center justify-between">
        <div>
          <h1 className="page-title">Contacts</h1>
          <p className="page-subtitle">
            Manage your peer list for quick access when sending files.
          </p>
        </div>
      </div>

      <div style={{ maxWidth: 640 }}>
        <p className="text-sm text-mist mb-4">
          Add people by their Device ID (e.g. A1B2-C3D4) — they share it once from their Receive page. 
          After adding, you can send them files directly from the Send page.
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
              onChange={e => {
                let val = e.target.value.toUpperCase().replace(/[^0-9A-F]/g, "");
                if (val.length > 4) val = val.slice(0, 4) + "-" + val.slice(4, 8);
                setNewContactId(val);
                setContactError(null);
                setContactSaved(false);
              }}
              placeholder="XXXX-XXXX"
              maxLength={9}
            />
          </div>
        </div>

        <div className="flex items-center gap-3 mb-6">
          <button
            className="btn btn-primary btn-sm"
            onClick={handleAdd}
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
                  <UserIcon />
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 600, color: "var(--ink-bright)" }}>{c.name}</div>
                  <div className="font-mono text-xs text-mist" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {c.node_id.length === 8 ? `${c.node_id.slice(0, 4)}-${c.node_id.slice(4, 8)}` : c.node_id}
                  </div>
                </div>
                <button
                  className="btn btn-ghost btn-sm"
                  style={{ color: "var(--ember)" }}
                  onClick={() => handleRemove(c.name)}
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        )}

        {contacts.length === 0 && (
          <div className="alert alert-info">
            No contacts yet. Ask someone for their Device ID and add it above.
          </div>
        )}
      </div>
    </div>
  );
}
