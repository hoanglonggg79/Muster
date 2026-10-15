import { Download } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function SettingsTab() {
  const multiRoblox = useAppStore((s) => s.multiRoblox);
  const toggleMultiRoblox = useAppStore((s) => s.toggleMultiRoblox);
  const unlockFps = useAppStore((s) => s.unlockFps);
  const maxFps = useAppStore((s) => s.maxFps);
  const applyFps = useAppStore((s) => s.applyFps);
  const setShowImportModal = useAppStore((s) => s.setShowImportModal);

  return (
    <div className="flex flex-col gap-6">
      <div className="settings-section">
        <span className="settings-section__title">
          Roblox Client & Engine Tweaks
        </span>

        <div className="settings-row">
          <div className="settings-info">
            <span className="settings-label">Multi-Roblox Instance</span>
            <span className="settings-desc">
              Bypasses single-instance Windows mutex to allow running multiple
              Roblox clients concurrently.
            </span>
          </div>
          <button
            type="button"
            className={`btn ${multiRoblox ? "btn-primary" : "btn-secondary"}`}
            onClick={toggleMultiRoblox}
          >
            {multiRoblox ? "ENABLED" : "DISABLED"}
          </button>
        </div>

        <div className="settings-row">
          <div className="settings-info">
            <span className="settings-label">FPS Unlocker Cap</span>
            <span className="settings-desc">
              Patches ClientAppSettings.json to uncap Roblox framerate above
              standard 60 FPS.
            </span>
          </div>
          <div className="fps-chip-group">
            <button
              type="button"
              className={`fps-chip ${!unlockFps ? "active" : ""}`}
              onClick={() => applyFps(false, 60)}
            >
              60 (Default)
            </button>
            <button
              type="button"
              className={`fps-chip ${unlockFps && maxFps === 144 ? "active" : ""}`}
              onClick={() => applyFps(true, 144)}
            >
              144 FPS
            </button>
            <button
              type="button"
              className={`fps-chip ${unlockFps && maxFps === 240 ? "active" : ""}`}
              onClick={() => applyFps(true, 240)}
            >
              240 FPS
            </button>
            <button
              type="button"
              className={`fps-chip ${unlockFps && maxFps === 360 ? "active" : ""}`}
              onClick={() => applyFps(true, 360)}
            >
              360 FPS
            </button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <span className="settings-section__title">Vault & Data Migration</span>

        <div className="settings-row">
          <div className="settings-info">
            <span className="settings-label">
              Import from RAM (Roblox Account Manager)
            </span>
            <span className="settings-desc">
              Migrate accounts from legacy C# Roblox-Account-Manager
              AccountData.json directory.
            </span>
          </div>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => setShowImportModal(true)}
          >
            <Download size={13} strokeWidth={1.75} />
            <span>Migrate Accounts</span>
          </button>
        </div>
      </div>

      <div className="settings-section">
        <span className="settings-section__title">Application Info</span>
        <div className="settings-row">
          <div className="settings-info">
            <span className="settings-label">Engine Core</span>
            <span className="settings-desc">
              Rust 2024 / Tauri v2 / Argon2i Encryption
            </span>
          </div>
          <span
            style={{
              fontFamily: "monospace",
              color: "var(--text-muted)",
              fontSize: "11px",
            }}
          >
            v0.1.0-alpha
          </span>
        </div>
      </div>
    </div>
  );
}
