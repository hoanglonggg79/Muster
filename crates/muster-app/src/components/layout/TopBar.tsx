import {
  Download,
  Gamepad2,
  LayoutDashboard,
  Plus,
  Settings,
  Users,
} from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function TopBar() {
  const activeTab = useAppStore((s) => s.activeTab);
  const accountsCount = useAppStore((s) => s.accounts.length);
  const setShowImportModal = useAppStore((s) => s.setShowImportModal);
  const setShowAddModal = useAppStore((s) => s.setShowAddModal);

  return (
    <header className="workspace__topbar">
      <div className="topbar-title">
        {activeTab === "overview" && (
          <>
            <LayoutDashboard
              size={18}
              strokeWidth={1.75}
              style={{ color: "var(--accent)" }}
            />
            <span>Overview Dashboard</span>
          </>
        )}
        {activeTab === "accounts" && (
          <>
            <Users
              size={18}
              strokeWidth={1.75}
              style={{ color: "var(--accent)" }}
            />
            <span>Account Manager ({accountsCount})</span>
          </>
        )}
        {activeTab === "games" && (
          <>
            <Gamepad2
              size={18}
              strokeWidth={1.75}
              style={{ color: "var(--accent)" }}
            />
            <span>Game Search & Fast Launch</span>
          </>
        )}
        {activeTab === "settings" && (
          <>
            <Settings
              size={18}
              strokeWidth={1.75}
              style={{ color: "var(--accent)" }}
            />
            <span>System Configuration</span>
          </>
        )}
      </div>

      <div className="topbar-actions">
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={() => setShowImportModal(true)}
        >
          <Download size={13} strokeWidth={1.75} />
          <span>Import RAM</span>
        </button>
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={() => setShowAddModal(true)}
        >
          <Plus size={13} strokeWidth={2} />
          <span>Add Account</span>
        </button>
      </div>
    </header>
  );
}
