import {
  Gamepad2,
  LayoutDashboard,
  Settings,
  Users,
} from "lucide-react";
import type { NavigationTab } from "../../lib/accountUtils";
import { useAppStore } from "../../store/useAppStore";

export function Sidebar() {
  const activeTab = useAppStore((s) => s.activeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const accountsCount = useAppStore((s) => s.accounts.length);
  const multiRoblox = useAppStore((s) => s.multiRoblox);
  const toggleMultiRoblox = useAppStore((s) => s.toggleMultiRoblox);
  const unlockFps = useAppStore((s) => s.unlockFps);
  const maxFps = useAppStore((s) => s.maxFps);

  const navItems: { id: NavigationTab; label: string; icon: typeof LayoutDashboard; badge?: number }[] = [
    { id: "overview", label: "Overview", icon: LayoutDashboard },
    { id: "accounts", label: "Accounts", icon: Users, badge: accountsCount },
    { id: "games", label: "Game Search", icon: Gamepad2 },
    { id: "settings", label: "Settings", icon: Settings },
  ];

  return (
    <aside className="sidebar">
      <div>
        <div className="sidebar__header">
          <div className="brand-monogram">M</div>
          <div className="brand-info">
            <span className="brand-title">MUSTER</span>
            <span className="brand-subtitle">ROBLOX MANAGER</span>
          </div>
        </div>

        <nav aria-label="Main Navigation" className="sidebar__nav">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                type="button"
                className={`nav-item ${isActive ? "active" : ""}`}
                onClick={() => setActiveTab(item.id)}
              >
                <div className="nav-item__left">
                  <Icon size={16} strokeWidth={1.75} />
                  <span>{item.label}</span>
                </div>
                {item.badge !== undefined && (
                  <span className="nav-badge">{item.badge}</span>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      <div className="sidebar__footer">
        <div className="system-status-card">
          <div className="system-status-row">
            <div style={{ display: "flex", alignItems: "center" }}>
              <span className={`status-dot ${multiRoblox ? "active" : ""}`} />
              <span style={{ color: "var(--text-secondary)" }}>Multi-Roblox</span>
            </div>
            <button
              type="button"
              className="quick-toggle-btn"
              onClick={toggleMultiRoblox}
            >
              {multiRoblox ? "ENABLED" : "OFF"}
            </button>
          </div>
          <div className="system-status-row">
            <div style={{ display: "flex", alignItems: "center" }}>
              <span className={`status-dot ${unlockFps ? "active" : ""}`} />
              <span style={{ color: "var(--text-secondary)" }}>FPS Cap</span>
            </div>
            <span
              style={{
                fontFamily: "monospace",
                color: unlockFps ? "var(--accent-text)" : "var(--text-muted)",
              }}
            >
              {unlockFps ? `${maxFps} FPS` : "60 FPS"}
            </span>
          </div>
        </div>
      </div>
    </aside>
  );
}
