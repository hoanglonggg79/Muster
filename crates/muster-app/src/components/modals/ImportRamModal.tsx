import { Download, X } from "lucide-react";
import { useState } from "react";
import { useAppStore } from "../../store/useAppStore";

export function ImportRamModal() {
  const showImportModal = useAppStore((s) => s.showImportModal);
  const setShowImportModal = useAppStore((s) => s.setShowImportModal);
  const importRamVault = useAppStore((s) => s.importRamVault);
  const notify = useAppStore((s) => s.notify);

  const [ramPathInput, setRamPathInput] = useState("");
  const [ramPassInput, setRamPassInput] = useState("");
  const [submitting, setSubmitting] = useState(false);

  if (!showImportModal) return null;

  async function handleImport() {
    if (!ramPathInput.trim()) {
      notify("Vui lòng nhập đường dẫn thư mục RAM cũ");
      return;
    }
    setSubmitting(true);
    try {
      const ok = await importRamVault(
        ramPathInput.trim(),
        ramPassInput || null
      );
      if (ok) {
        setRamPathInput("");
        setRamPassInput("");
      }
    } finally {
      setSubmitting(false);
    }
  }

  function handleClose() {
    if (!submitting) {
      setShowImportModal(false);
    }
  }

  return (
    <div className="modal-overlay">
      <button
        type="button"
        className="fixed inset-0 border-0 bg-transparent cursor-default"
        aria-label="Close modal overlay"
        onClick={handleClose}
      />
      <div
        className="modal-box relative z-10"
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-ram-title"
      >
        <div className="modal-header">
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <Download
              size={16}
              strokeWidth={1.75}
              style={{ color: "var(--accent)" }}
            />
            <h3 id="import-ram-title">Import from RAM Directory</h3>
          </div>
          <button
            type="button"
            className="icon-btn"
            onClick={handleClose}
            disabled={submitting}
            aria-label="Close"
          >
            <X size={15} strokeWidth={1.75} />
          </button>
        </div>

        <p style={{ color: "var(--text-muted)", fontSize: "12px" }}>
          Specify the path to your legacy Roblox Account Manager folder where
          AccountData.json is stored.
        </p>

        <div className="input-group">
          <label className="input-label" htmlFor="import-ram-path">
            RAM Directory Path
          </label>
          <input
            id="import-ram-path"
            className="input-field"
            type="text"
            placeholder="C:\Users\Username\Roblox-Account-Manager"
            value={ramPathInput}
            onChange={(e) => setRamPathInput(e.target.value)}
          />
        </div>

        <div className="input-group">
          <label className="input-label" htmlFor="import-ram-pass">
            Vault Password (chỉ cần nếu AccountData.json được khoá mật khẩu)
          </label>
          <input
            id="import-ram-pass"
            className="input-field"
            type="password"
            placeholder="Để trống nếu file không có mật khẩu"
            value={ramPassInput}
            onChange={(e) => setRamPassInput(e.target.value)}
          />
        </div>

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: "8px",
            marginTop: "8px",
          }}
        >
          <button
            type="button"
            className="btn btn-secondary"
            onClick={handleClose}
            disabled={submitting}
          >
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            onClick={handleImport}
            disabled={submitting}
          >
            {submitting ? "Importing..." : "Start Import"}
          </button>
        </div>
      </div>
    </div>
  );
}
