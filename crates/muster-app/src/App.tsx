import { useState, useEffect, useCallback } from "react";
import { commands } from "./bindings";
import type { Account } from "./bindings";
import {
  LayoutDashboard,
  Users,
  Gamepad2,
  Settings,
  Play,
  RefreshCw,
  Trash2,
  Plus,
  Download,
  Layers,
  Zap,
  Search,
  LayoutGrid,
  List,
  Coins,
  Shield,
  X,
  KeyRound,
  Compass,
  Square,
  SquareCheck,
  SquarePen,
} from "lucide-react";

type NavigationTab = "overview" | "accounts" | "games" | "settings";

type LaunchPhase = "launching" | "done" | "error";

function stripGroupPrefix(group: string): string {
  return group.replace(/^\d{1,3}\s?/, "");
}

function groupLabel(acc: Account): string {
  return stripGroupPrefix(acc.Group && acc.Group.trim() ? acc.Group : "Default");
}

function presenceLabel(type: number): string {
  switch (type) {
    case 1:
      return "Online";
    case 2:
      return "In Game";
    case 3:
      return "In Studio";
    default:
      return "Offline";
  }
}

function presenceClass(type: number): string {
  switch (type) {
    case 1:
      return "presence-badge presence-online";
    case 2:
      return "presence-badge presence-ingame";
    case 3:
      return "presence-badge presence-instudio";
    default:
      return "presence-badge presence-offline";
  }
}

// Preset game bookmarked for quick launch
const PRESET_GAMES = [
  { name: "Blox Fruits", placeId: "4483381587" },
  { name: "Blade Ball", placeId: "13772394625" },
  { name: "Deepwoken", placeId: "4111023553" },
  { name: "Pet Simulator 99", placeId: "1537690962" },
  { name: "Arsenal", placeId: "286090429" },
  { name: "Murder Mystery 2", placeId: "142823291" },
];

export function App() {
  const [activeTab, setActiveTab] = useState<NavigationTab>("accounts");
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [viewMode, setViewMode] = useState<"grid" | "table">("grid");

  // Launch state
  const [placeId, setPlaceId] = useState("4483381587");
  const [jobId, setJobId] = useState("");
  const [selectedUserId, setSelectedUserId] = useState<number | null>(null);

  // System status
  const [multiRoblox, setMultiRoblox] = useState(false);
  const [unlockFps, setUnlockFps] = useState(true);
  const [maxFps, setMaxFps] = useState(240);
  const [toast, setToast] = useState<string | null>(null);

  // Modals
  const [showAddModal, setShowAddModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [addTab, setAddTab] = useState<"cookie" | "credentials">("cookie");

  const [cookieInput, setCookieInput] = useState("");
  const [aliasInput, setAliasInput] = useState("");
  const [userInput, setUserInput] = useState("");
  const [passInput, setPassInput] = useState("");
  const [ramPathInput, setRamPathInput] = useState("");
  const [ramPassInput, setRamPassInput] = useState("");

  const [submitting, setSubmitting] = useState(false);

  const [selectedIds, setSelectedIds] = useState<number[]>([]);
  const [launchPhases, setLaunchPhases] = useState<Record<number, LaunchPhase>>({});
  const [presence, setPresence] = useState<Record<number, number>>({});
  const [batchRunning, setBatchRunning] = useState(false);

  const [editAccount, setEditAccount] = useState<Account | null>(null);
  const [editAlias, setEditAlias] = useState("");
  const [editGroup, setEditGroup] = useState("");
  const [editDescription, setEditDescription] = useState("");
  const [editPlaceId, setEditPlaceId] = useState("");
  const [editJobId, setEditJobId] = useState("");

  function notify(msg: string) {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  }

  const refreshPresence = useCallback(async (ids: number[]) => {
    if (ids.length === 0) {
      setPresence({});
      return;
    }
    try {
      const res = await commands.presenceBatch(ids);
      if (res.status === "ok") {
        const map: Record<number, number> = {};
        for (const item of res.data) {
          if (item.user_id !== null && item.user_id !== undefined) {
            map[item.user_id] = item.presence_type;
          }
        }
        setPresence(map);
      }
    } catch {
      setPresence({});
    }
  }, []);

  async function refreshAccounts() {
    try {
      const res = await commands.accountList();
      if (res.status === "ok") {
        setAccounts(res.data);
        if (res.data.length > 0 && selectedUserId === null) {
          const firstUid = res.data[0]?.UserID;
          if (firstUid) setSelectedUserId(firstUid);
        }
        refreshPresence(res.data.map((acc) => acc.UserID ?? 0).filter((id) => id > 0));
      }
    } catch (e) {
      notify(`Lỗi tải danh sách: ${String(e)}`);
    }
  }

  useEffect(() => {
    refreshAccounts();
  }, []);

  useEffect(() => {
    const timer = setInterval(() => {
      refreshPresence(accounts.map((acc) => acc.UserID ?? 0).filter((id) => id > 0));
    }, 120000);
    return () => clearInterval(timer);
  }, [accounts, refreshPresence]);

  function savedPlaceId(acc: Account): string {
    const saved = acc.Fields?.SavedPlaceId;
    return saved && saved.trim() ? saved.trim() : placeId;
  }

  function savedJobId(acc: Account): string | null {
    const saved = acc.Fields?.SavedJobId;
    if (saved && saved.trim()) return saved.trim();
    return jobId.trim() || null;
  }

  async function toggleMultiRoblox() {
    try {
      const res = await commands.multiRobloxToggle(!multiRoblox);
      if (res.status === "ok") {
        setMultiRoblox(res.data);
        notify(res.data ? "Đã kích hoạt Multi-Roblox" : "Đã tắt Multi-Roblox");
      }
    } catch (e) {
      notify(`Lỗi Multi-Roblox: ${String(e)}`);
    }
  }

  async function applyFps(newUnlock: boolean, fpsVal: number) {
    try {
      const res = await commands.patchApplyFps(newUnlock, fpsVal);
      if (res.status === "ok") {
        setUnlockFps(newUnlock);
        setMaxFps(fpsVal);
        notify(res.data);
      } else {
        notify(`Lỗi FPS: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi FPS: ${String(e)}`);
    }
  }

  async function launchAccount(userId: number, acc?: Account): Promise<boolean> {
    const rawPlace = acc ? savedPlaceId(acc) : placeId;
    const pid = Number(String(rawPlace).replace(/[^0-9]/g, ""));
    if (!pid || Number.isNaN(pid)) {
      notify("Vui lòng nhập Place ID hợp lệ");
      return false;
    }
    const job = acc ? savedJobId(acc) : (jobId.trim() || null);
    notify("Đang khởi chạy Roblox client...");
    try {
      const res = await commands.accountLaunch(userId, pid, job);
      if (res.status === "ok") {
        notify(res.data);
        return true;
      }
      notify(`Lỗi khởi chạy: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Lỗi launch: ${String(e)}`);
      return false;
    }
  }

  function toggleSelect(userId: number) {
    setSelectedIds((prev) =>
      prev.includes(userId) ? prev.filter((id) => id !== userId) : [...prev, userId]
    );
  }

  function toggleSelectAll() {
    const visible = filteredAccounts.map((acc) => acc.UserID ?? 0);
    const allSelected = visible.every((id) => selectedIds.includes(id));
    setSelectedIds(allSelected ? [] : visible);
  }

  async function launchSelected() {
    const chosen = filteredAccounts.filter((acc) => selectedIds.includes(acc.UserID ?? 0));
    if (chosen.length === 0) {
      notify("Chưa chọn tài khoản nào để khởi chạy");
      return;
    }

    setBatchRunning(true);
    let okCount = 0;
    const failed: string[] = [];

    for (let index = 0; index < chosen.length; index++) {
      const acc = chosen[index];
      if (!acc) continue;
      const uid = acc.UserID ?? 0;
      const name = acc.Fields?.DisplayName || acc.Username || String(uid);

      setLaunchPhases((prev) => ({ ...prev, [uid]: "launching" }));
      const ok = await launchAccount(uid, acc);
      setLaunchPhases((prev) => ({ ...prev, [uid]: ok ? "done" : "error" }));

      if (ok) {
        okCount++;
      } else {
        failed.push(name);
      }

      if (index < chosen.length - 1) {
        await new Promise((resolve) => setTimeout(resolve, 2000));
      }
    }

    setBatchRunning(false);
    setSelectedIds([]);
    notify(
      failed.length === 0
        ? `Đã khởi chạy ${okCount} tài khoản`
        : `Hoàn tất: ${okCount} thành công, lỗi ${failed.length} (${failed.join(", ")})`
    );
    setTimeout(() => setLaunchPhases({}), 6000);
  }

  function openEdit(acc: Account) {
    setEditAccount(acc);
    setEditAlias(acc.Alias ?? "");
    setEditGroup(acc.Group ?? "");
    setEditDescription(acc.Description ?? "");
    setEditPlaceId(acc.Fields?.SavedPlaceId ?? "");
    setEditJobId(acc.Fields?.SavedJobId ?? "");
  }

  async function saveEdit() {
    if (!editAccount) return;
    setSubmitting(true);
    try {
      const res = await commands.accountUpdate(
        editAccount.UserID ?? 0,
        editAlias,
        editGroup.trim() || null,
        editDescription,
        editPlaceId.trim() || null,
        editJobId.trim() || null
      );
      if (res.status === "ok") {
        notify(`Đã cập nhật ${res.data.Username ?? "tài khoản"}`);
        setEditAccount(null);
        refreshAccounts();
      } else {
        notify(`Lỗi cập nhật: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi: ${String(e)}`);
    } finally {
      setSubmitting(false);
    }
  }

  async function refreshAccount(userId: number) {
    try {
      const res = await commands.accountRefresh(userId);
      if (res.status === "ok") {
        notify(`Đã cập nhật: ${res.data.Username ?? "Tài khoản"}`);
        refreshAccounts();
      } else {
        notify(`Lỗi làm mới: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi: ${String(e)}`);
    }
  }

  async function removeAccount(userId: number, name: string) {
    if (!confirm(`Xác nhận xoá tài khoản ${name}?`)) return;
    try {
      const res = await commands.accountRemove(userId);
      if (res.status === "ok") {
        notify(`Đã xoá tài khoản ${name}`);
        refreshAccounts();
      } else {
        notify(`Lỗi: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi: ${String(e)}`);
    }
  }

  async function handleAddAccount() {
    setSubmitting(true);
    try {
      if (addTab === "cookie") {
        if (!cookieInput.trim()) {
          notify("Vui lòng dán cookie .ROBLOSECURITY");
          setSubmitting(false);
          return;
        }
        const res = await commands.accountAddCookie(cookieInput.trim(), aliasInput.trim() || null);
        if (res.status === "ok") {
          notify(`Thêm thành công ${res.data.Username ?? "tài khoản"}`);
          setShowAddModal(false);
          setCookieInput("");
          setAliasInput("");
          refreshAccounts();
        } else {
          notify(`Lỗi thêm: ${res.error}`);
        }
      } else {
        if (!userInput.trim() || !passInput) {
          notify("Vui lòng nhập tên tài khoản và mật khẩu");
          setSubmitting(false);
          return;
        }
        const res = await commands.accountAddCredentials(
          userInput.trim(),
          passInput,
          aliasInput.trim() || null
        );
        if (res.status === "ok") {
          notify(`Đăng nhập thành công: ${res.data.Username ?? ""}`);
          setShowAddModal(false);
          setUserInput("");
          setPassInput("");
          setAliasInput("");
          refreshAccounts();
        } else {
          notify(`Đăng nhập thất bại: ${res.error}`);
        }
      }
    } catch (e) {
      notify(`Lỗi: ${String(e)}`);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleImportRAM() {
    if (!ramPathInput.trim()) {
      notify("Vui lòng nhập đường dẫn thư mục RAM cũ");
      return;
    }
    setSubmitting(true);
    try {
      const res = await commands.vaultImportRam(ramPathInput.trim(), ramPassInput || null);
      if (res.status === "ok") {
        notify(`Đã nhập thành công ${res.data} tài khoản từ RAM!`);
        setShowImportModal(false);
        setRamPathInput("");
        setRamPassInput("");
        refreshAccounts();
      } else {
        notify(`Lỗi nhập: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi import: ${String(e)}`);
    } finally {
      setSubmitting(false);
    }
  }

  // Filtered accounts
  const filteredAccounts = accounts.filter((acc) => {
    const q = searchQuery.toLowerCase().trim();
    if (!q) return true;
    const name = (acc.Fields?.DisplayName || acc.Alias || acc.Username || "").toLowerCase();
    const username = (acc.Username || "").toLowerCase();
    const uid = String(acc.UserID || "");
    return name.includes(q) || username.includes(q) || uid.includes(q);
  });

  // Calculate total Robux
  const totalRobux = accounts.reduce((acc, curr) => {
    const r = parseInt(curr.Fields?.Robux?.replace(/[^0-9]/g, "") || "0", 10);
    return acc + (Number.isNaN(r) ? 0 : r);
  }, 0);

  return (
    <div className="app-shell">
      {/* 20% SIDEBAR */}
      <aside className="sidebar">
        <div>
          <div className="sidebar__header">
            <div className="brand-monogram">M</div>
            <div className="brand-info">
              <span className="brand-title">MUSTER</span>
              <span className="brand-subtitle">ROBLOX MANAGER</span>
            </div>
          </div>

          <nav className="sidebar__nav">
            <button type="button"
              className={`nav-item ${activeTab === "overview" ? "active" : ""}`}
              onClick={() => setActiveTab("overview")}
            >
              <div className="nav-item__left">
                <LayoutDashboard size={16} strokeWidth={1.75} />
                <span>Overview</span>
              </div>
            </button>

            <button type="button"
              className={`nav-item ${activeTab === "accounts" ? "active" : ""}`}
              onClick={() => setActiveTab("accounts")}
            >
              <div className="nav-item__left">
                <Users size={16} strokeWidth={1.75} />
                <span>Accounts</span>
              </div>
              <span className="nav-badge">{accounts.length}</span>
            </button>

            <button type="button"
              className={`nav-item ${activeTab === "games" ? "active" : ""}`}
              onClick={() => setActiveTab("games")}
            >
              <div className="nav-item__left">
                <Gamepad2 size={16} strokeWidth={1.75} />
                <span>Game Search</span>
              </div>
            </button>

            <button type="button"
              className={`nav-item ${activeTab === "settings" ? "active" : ""}`}
              onClick={() => setActiveTab("settings")}
            >
              <div className="nav-item__left">
                <Settings size={16} strokeWidth={1.75} />
                <span>Settings</span>
              </div>
            </button>
          </nav>
        </div>

        {/* System Quick Status Footer */}
        <div className="sidebar__footer">
          <div className="system-status-card">
            <div className="system-status-row">
              <div style={{ display: "flex", alignItems: "center" }}>
                <span className={`status-dot ${multiRoblox ? "active" : ""}`}></span>
                <span style={{ color: "var(--text-secondary)" }}>Multi-Roblox</span>
              </div>
              <button type="button" className="quick-toggle-btn" onClick={toggleMultiRoblox}>
                {multiRoblox ? "ENABLED" : "OFF"}
              </button>
            </div>
            <div className="system-status-row">
              <div style={{ display: "flex", alignItems: "center" }}>
                <span className={`status-dot ${unlockFps ? "active" : ""}`}></span>
                <span style={{ color: "var(--text-secondary)" }}>FPS Cap</span>
              </div>
              <span style={{ fontFamily: "monospace", color: unlockFps ? "var(--accent-text)" : "var(--text-muted)" }}>
                {unlockFps ? `${maxFps} FPS` : "60 FPS"}
              </span>
            </div>
          </div>
        </div>
      </aside>

      {/* 80% WORKSPACE */}
      <main className="workspace">
        <header className="workspace__topbar">
          <div className="topbar-title">
            {activeTab === "overview" && (
              <>
                <LayoutDashboard size={18} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <span>Overview Dashboard</span>
              </>
            )}
            {activeTab === "accounts" && (
              <>
                <Users size={18} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <span>Account Manager ({accounts.length})</span>
              </>
            )}
            {activeTab === "games" && (
              <>
                <Gamepad2 size={18} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <span>Game Search & Fast Launch</span>
              </>
            )}
            {activeTab === "settings" && (
              <>
                <Settings size={18} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <span>System Configuration</span>
              </>
            )}
          </div>

          <div className="topbar-actions">
            <button type="button" className="btn btn-secondary btn-sm" onClick={() => setShowImportModal(true)}>
              <Download size={13} strokeWidth={1.75} />
              <span>Import RAM</span>
            </button>
            <button type="button" className="btn btn-primary btn-sm" onClick={() => setShowAddModal(true)}>
              <Plus size={13} strokeWidth={2} />
              <span>Add Account</span>
            </button>
          </div>
        </header>

        <section className="workspace__content">
          {/* TAB 1: OVERVIEW */}
          {activeTab === "overview" && (
            <>
              <div className="overview-metrics">
                <div className="metric-card">
                  <div className="metric-card__header">
                    <span>Total Accounts</span>
                    <Users size={15} strokeWidth={1.75} />
                  </div>
                  <div className="metric-card__val">{accounts.length}</div>
                  <div className="metric-card__sub">Stored in encrypted vault</div>
                </div>

                <div className="metric-card">
                  <div className="metric-card__header">
                    <span>Total Robux</span>
                    <Coins size={15} strokeWidth={1.75} style={{ color: "var(--gold-robux)" }} />
                  </div>
                  <div className="metric-card__val" style={{ color: "var(--gold-robux)" }}>
                    {totalRobux.toLocaleString()}
                  </div>
                  <div className="metric-card__sub">Aggregate balance</div>
                </div>

                <div className="metric-card">
                  <div className="metric-card__header">
                    <span>Multi-Instance</span>
                    <Layers size={15} strokeWidth={1.75} />
                  </div>
                  <div className="metric-card__val" style={{ color: multiRoblox ? "var(--accent-text)" : "var(--text-muted)" }}>
                    {multiRoblox ? "ACTIVE" : "STANDBY"}
                  </div>
                  <div className="metric-card__sub">Mutex patcher status</div>
                </div>

                <div className="metric-card">
                  <div className="metric-card__header">
                    <span>Target FPS</span>
                    <Zap size={15} strokeWidth={1.75} />
                  </div>
                  <div className="metric-card__val">
                    {unlockFps ? `${maxFps}` : "60"}
                  </div>
                  <div className="metric-card__sub">{unlockFps ? "Unlocked cap" : "Default engine limit"}</div>
                </div>
              </div>

              {/* Quick Launch Panel */}
              <div className="overview-banner">
                <div className="overview-banner__title">
                  <Compass size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                  <span>Quick Launcher</span>
                </div>

                <div className="launch-grid">
                  <div className="input-group">
                    <label className="input-label">Place ID (Game ID)</label>
                    <input
                      className="input-field"
                      type="text"
                      value={placeId}
                      onChange={(e) => setPlaceId(e.target.value)}
                      placeholder="e.g. 4483381587"
                    />
                  </div>
                  <div className="input-group">
                    <label className="input-label">Job ID (Server ID - Optional)</label>
                    <input
                      className="input-field"
                      type="text"
                      value={jobId}
                      onChange={(e) => setJobId(e.target.value)}
                      placeholder="Leave empty for public matchmaking"
                    />
                  </div>
                </div>

                {accounts.length > 0 && (
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: "10px", flex: 1 }}>
                      <label className="input-label" style={{ whiteSpace: "nowrap" }}>Launch As:</label>
                      <select
                        className="input-field"
                        style={{ flex: 1, padding: "6px 10px" }}
                        value={selectedUserId ?? accounts[0]?.UserID ?? ""}
                        onChange={(e) => setSelectedUserId(Number(e.target.value))}
                      >
                        {accounts.map((acc) => (
                          <option key={acc.UserID ?? 0} value={acc.UserID ?? 0}>
                            {acc.Fields?.DisplayName || acc.Alias || acc.Username || `ID: ${acc.UserID}`} (@{acc.Username})
                          </option>
                        ))}
                      </select>
                    </div>

                    <button type="button"
                      className="btn btn-primary"
                      onClick={() => {
                        const targetId = selectedUserId ?? accounts[0]?.UserID;
                        if (targetId) launchAccount(targetId);
                      }}
                    >
                      <Play size={13} fill="currentColor" />
                      <span>Launch Instance</span>
                    </button>
                  </div>
                )}
              </div>
            </>
          )}

          {/* TAB 2: ACCOUNTS */}
          {activeTab === "accounts" && (
            <>
              <div className="accounts-toolbar">
                <div className="search-box">
                  <Search size={14} strokeWidth={1.75} color="var(--text-muted)" />
                  <input
                    type="text"
                    placeholder="Search accounts by username, alias, id..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                  />
                  {searchQuery && (
                    <button type="button" className="icon-btn" onClick={() => setSearchQuery("")}>
                      <X size={12} strokeWidth={1.75} />
                    </button>
                  )}
                </div>

                <div className="view-controls">
                  <button type="button"
                    className={`icon-btn ${viewMode === "grid" ? "active" : ""}`}
                    onClick={() => setViewMode("grid")}
                    title="Grid view"
                  >
                    <LayoutGrid size={15} strokeWidth={1.75} />
                  </button>
                  <button type="button"
                    className={`icon-btn ${viewMode === "table" ? "active" : ""}`}
                    onClick={() => setViewMode("table")}
                    title="Table view"
                  >
                    <List size={15} strokeWidth={1.75} />
                  </button>
                  <div style={{ width: "1px", height: "16px", background: "var(--border-medium)", margin: "0 4px" }} />
                  <button type="button" className="btn btn-secondary btn-sm" onClick={refreshAccounts} title="Reload accounts">
                    <RefreshCw size={12} strokeWidth={1.75} />
                    <span>Refresh</span>
                  </button>
                </div>
              </div>

              <div className="batch-bar">
                <button type="button" className="btn btn-secondary btn-sm" onClick={toggleSelectAll}>
                  {filteredAccounts.length > 0 && filteredAccounts.every((acc) => selectedIds.includes(acc.UserID ?? 0)) ? (
                    <SquareCheck size={13} strokeWidth={1.75} />
                  ) : (
                    <Square size={13} strokeWidth={1.75} />
                  )}
                  <span>{selectedIds.length > 0 ? `Đã chọn ${selectedIds.length}` : "Chọn tất cả"}</span>
                </button>
                <button type="button"
                  className="btn btn-primary btn-sm"
                  onClick={launchSelected}
                  disabled={batchRunning || selectedIds.length === 0}
                  title="Khởi chạy tuần tự các tài khoản đã chọn"
                >
                  <Play size={12} fill="currentColor" />
                  <span>{batchRunning ? "Đang chạy..." : `Launch ${selectedIds.length || ""}`.trim()}</span>
                </button>
              </div>

              {filteredAccounts.length === 0 ? (
                <div className="empty-state">
                  <Shield size={32} strokeWidth={1.5} color="var(--text-muted)" />
                  <h3>No accounts found</h3>
                  <p>
                    {searchQuery
                      ? "No account matches your search query."
                      : "Add accounts via .ROBLOSECURITY cookie or import existing accounts from RAM."}
                  </p>
                  <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
                    <button type="button" className="btn btn-primary btn-sm" onClick={() => setShowAddModal(true)}>
                      <Plus size={13} strokeWidth={2} />
                      <span>Add New Account</span>
                    </button>
                    <button type="button" className="btn btn-secondary btn-sm" onClick={() => setShowImportModal(true)}>
                      <Download size={13} strokeWidth={1.75} />
                      <span>Import From RAM</span>
                    </button>
                  </div>
                </div>
              ) : viewMode === "grid" ? (
                <div className="accounts-grid">
                  {filteredAccounts.map((acc) => {
                    const uid = acc.UserID ?? 0;
                    const displayName = acc.Fields?.DisplayName || acc.Alias || acc.Username || "Roblox Player";
                    const username = acc.Username || `ID: ${uid}`;
                    const robux = acc.Fields?.Robux || "0";
                    const avatarUrl = acc.Fields?.AvatarUrl;
                    const selected = selectedIds.includes(uid);
                    const phase = launchPhases[uid];

                    return (
                      <div key={uid} className={`account-card ${selected ? "selected" : ""}`}>
                        <div className="account-card__header">
                          <button type="button"
                            className="acct-check"
                            onClick={() => toggleSelect(uid)}
                            title={selected ? "Bỏ chọn" : "Chọn để launch hàng loạt"}
                          >
                            {selected ? <SquareCheck size={16} strokeWidth={1.75} /> : <Square size={16} strokeWidth={1.75} />}
                          </button>
                          <div className="avatar-frame">
                            {avatarUrl ? (
                              <img src={avatarUrl} alt={username} />
                            ) : (
                              <span className="avatar-fallback-text">
                                {displayName.charAt(0).toUpperCase()}
                              </span>
                            )}
                          </div>
                          <div className="account-details">
                            <span className="account-displayname" title={displayName}>
                              {displayName}
                            </span>
                            <span className="account-username">@{username}</span>
                            <span className="account-robux">
                              <Coins size={11} strokeWidth={1.75} />
                              <span>{robux}</span>
                            </span>
                          </div>
                        </div>

                        <div className="account-card__meta">
                          <span className="group-chip" title={`Nhóm: ${acc.Group ?? "Default"}`}>
                            {groupLabel(acc)}
                          </span>
                          <span className={presenceClass(presence[uid] ?? 0)}>
                            <span className="presence-dot" />
                            {presenceLabel(presence[uid] ?? 0)}
                          </span>
                        </div>

                        <div className="account-card__footer">
                          <div className="card-action-group">
                            <button type="button"
                              className="icon-btn"
                              onClick={() => openEdit(acc)}
                              title="Sửa tài khoản"
                            >
                              <SquarePen size={13} strokeWidth={1.75} />
                            </button>
                            <button type="button"
                              className="icon-btn"
                              onClick={() => refreshAccount(uid)}
                              title="Refresh profile & Robux"
                            >
                              <RefreshCw size={13} strokeWidth={1.75} />
                            </button>
                            <button type="button"
                              className="icon-btn danger"
                              onClick={() => removeAccount(uid, displayName)}
                              title="Remove account"
                            >
                              <Trash2 size={13} strokeWidth={1.75} />
                            </button>
                          </div>

                          <button
                            type="button"
                            className={`btn btn-primary btn-sm ${phase === "launching" ? "is-busy" : ""}`}
                            onClick={() => launchAccount(uid, acc)}
                            disabled={phase === "launching"}
                          >
                            <Play size={11} fill="currentColor" />
                            <span>{phase === "launching" ? "Đang chạy" : "Launch"}</span>
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <div className="table-container">
                  <table className="data-table">
                    <thead>
                      <tr>
                        <th style={{ width: "36px" }} />
                        <th>Avatar</th>
                        <th>Display Name</th>
                        <th>Username</th>
                        <th>User ID</th>
                        <th>Robux</th>
                        <th>Group</th>
                        <th>Presence</th>
                        <th style={{ textAlign: "right" }}>Actions</th>
                      </tr>
                    </thead>
                    <tbody>
                      {filteredAccounts.map((acc) => {
                        const uid = acc.UserID ?? 0;
                        const displayName = acc.Fields?.DisplayName || acc.Alias || acc.Username || "Roblox Player";
                        const username = acc.Username || `ID: ${uid}`;
                        const robux = acc.Fields?.Robux || "0";
                        const avatarUrl = acc.Fields?.AvatarUrl;
                        const selected = selectedIds.includes(uid);
                        const phase = launchPhases[uid];

                        return (
                          <tr key={uid} className={selected ? "selected" : ""}>
                            <td>
                              <button type="button" className="acct-check" onClick={() => toggleSelect(uid)}>
                                {selected ? <SquareCheck size={15} strokeWidth={1.75} /> : <Square size={15} strokeWidth={1.75} />}
                              </button>
                            </td>
                            <td style={{ width: "48px" }}>
                              <div className="avatar-frame" style={{ width: "32px", height: "32px" }}>
                                {avatarUrl ? (
                                  <img src={avatarUrl} alt={username} />
                                ) : (
                                  <span className="avatar-fallback-text" style={{ fontSize: "11px" }}>
                                    {displayName.charAt(0).toUpperCase()}
                                  </span>
                                )}
                              </div>
                            </td>
                            <td style={{ fontWeight: 600 }}>{displayName}</td>
                            <td style={{ color: "var(--text-secondary)", fontFamily: "monospace" }}>@{username}</td>
                            <td style={{ color: "var(--text-muted)", fontFamily: "monospace" }}>{uid}</td>
                            <td>
                              <span className="account-robux">
                                <Coins size={11} strokeWidth={1.75} />
                                <span>{robux}</span>
                              </span>
                            </td>
                            <td>
                              <span className="group-chip">{groupLabel(acc)}</span>
                            </td>
                            <td>
                              <span className={presenceClass(presence[uid] ?? 0)}>
                                <span className="presence-dot" />
                                {presenceLabel(presence[uid] ?? 0)}
                              </span>
                            </td>
                            <td style={{ textAlign: "right" }}>
                              <div style={{ display: "inline-flex", gap: "6px" }}>
                                <button type="button" className="icon-btn" onClick={() => openEdit(acc)} title="Edit">
                                  <SquarePen size={13} strokeWidth={1.75} />
                                </button>
                                <button type="button" className="icon-btn" onClick={() => refreshAccount(uid)} title="Refresh">
                                  <RefreshCw size={13} strokeWidth={1.75} />
                                </button>
                                <button type="button" className="icon-btn danger" onClick={() => removeAccount(uid, displayName)} title="Delete">
                                  <Trash2 size={13} strokeWidth={1.75} />
                                </button>
                                <button
                                  type="button"
                                  className="btn btn-primary btn-sm"
                                  onClick={() => launchAccount(uid, acc)}
                                  disabled={phase === "launching"}
                                >
                                  <Play size={10} fill="currentColor" />
                                  <span>{phase === "launching" ? "..." : "Launch"}</span>
                                </button>
                              </div>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
            </>
          )}

          {/* TAB 3: GAME SEARCH / PLACE EXPLORER */}
          {activeTab === "games" && (
            <div className="launch-bar">
              <div className="overview-banner__title">
                <Gamepad2 size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <span>Target Game Configuration</span>
              </div>

              <div className="launch-grid">
                <div className="input-group">
                  <label className="input-label">Place ID (Roblox Experience ID)</label>
                  <input
                    className="input-field"
                    type="text"
                    value={placeId}
                    onChange={(e) => setPlaceId(e.target.value)}
                    placeholder="e.g. 4483381587"
                  />
                </div>
                <div className="input-group">
                  <label className="input-label">Job ID (Specific Server Instance)</label>
                  <input
                    className="input-field"
                    type="text"
                    value={jobId}
                    onChange={(e) => setJobId(e.target.value)}
                    placeholder="Leave empty for public join"
                  />
                </div>
              </div>

              <div style={{ marginTop: "6px" }}>
                <label className="input-label" style={{ marginBottom: "8px", display: "block" }}>
                  Quick Bookmarks
                </label>
                <div className="game-bookmarks">
                  {PRESET_GAMES.map((game) => (
                    <div
                      key={game.placeId}
                      className={`game-bookmark-chip ${placeId === game.placeId ? "active" : ""}`}
                      onClick={() => setPlaceId(game.placeId)}
                    >
                      <span className="bookmark-title">{game.name}</span>
                      <span className="bookmark-id">{game.placeId}</span>
                    </div>
                  ))}
                </div>
              </div>

              {accounts.length > 0 && (
                <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "10px" }}>
                  <button type="button"
                    className="btn btn-primary"
                    onClick={() => {
                      const targetId = selectedUserId ?? accounts[0]?.UserID;
                      if (targetId) launchAccount(targetId);
                    }}
                  >
                    <Play size={13} fill="currentColor" />
                    <span>Launch Game Now</span>
                  </button>
                </div>
              )}
            </div>
          )}

          {/* TAB 4: SETTINGS */}
          {activeTab === "settings" && (
            <>
              <div className="settings-section">
                <span className="settings-section__title">Roblox Client & Engine Tweaks</span>

                <div className="settings-row">
                  <div className="settings-info">
                    <span className="settings-label">Multi-Roblox Instance</span>
                    <span className="settings-desc">
                      Bypasses single-instance Windows mutex to allow running multiple Roblox clients concurrently.
                    </span>
                  </div>
                  <button type="button"
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
                      Patches ClientAppSettings.json to uncap Roblox framerate above standard 60 FPS.
                    </span>
                  </div>
                  <div className="fps-chip-group">
                    <button type="button"
                      className={`fps-chip ${!unlockFps ? "active" : ""}`}
                      onClick={() => applyFps(false, 60)}
                    >
                      60 (Default)
                    </button>
                    <button type="button"
                      className={`fps-chip ${unlockFps && maxFps === 144 ? "active" : ""}`}
                      onClick={() => applyFps(true, 144)}
                    >
                      144 FPS
                    </button>
                    <button type="button"
                      className={`fps-chip ${unlockFps && maxFps === 240 ? "active" : ""}`}
                      onClick={() => applyFps(true, 240)}
                    >
                      240 FPS
                    </button>
                    <button type="button"
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
                    <span className="settings-label">Import from RAM (Roblox Account Manager)</span>
                    <span className="settings-desc">
                      Migrate accounts from legacy C# Roblox-Account-Manager AccountData.json directory.
                    </span>
                  </div>
                  <button type="button" className="btn btn-secondary" onClick={() => setShowImportModal(true)}>
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
                    <span className="settings-desc">Rust 2024 / Tauri v2 / Argon2i Encryption</span>
                  </div>
                  <span style={{ fontFamily: "monospace", color: "var(--text-muted)", fontSize: "11px" }}>
                    v0.1.0-alpha
                  </span>
                </div>
              </div>
            </>
          )}
        </section>
      </main>

      {/* ADD ACCOUNT MODAL */}
      {showAddModal && (
        <div className="modal-overlay" onClick={() => !submitting && setShowAddModal(false)}>
          <div className="modal-box" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <KeyRound size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <h3>Add Account</h3>
              </div>
              <button type="button"
                className="icon-btn"
                onClick={() => setShowAddModal(false)}
                disabled={submitting}
              >
                <X size={15} strokeWidth={1.75} />
              </button>
            </div>

            <div className="modal-tabs">
              <div
                className={`modal-tab ${addTab === "cookie" ? "active" : ""}`}
                onClick={() => setAddTab("cookie")}
              >
                Cookie (.ROBLOSECURITY)
              </div>
              <div
                className={`modal-tab ${addTab === "credentials" ? "active" : ""}`}
                onClick={() => setAddTab("credentials")}
              >
                Credentials (Username & Password)
              </div>
            </div>

            {addTab === "cookie" ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div className="input-group">
                  <label className="input-label">Cookie Value</label>
                  <textarea
                    className="input-field"
                    rows={4}
                    placeholder="_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you..."
                    value={cookieInput}
                    onChange={(e) => setCookieInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label className="input-label">Alias / Account Label (Optional)</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="e.g. Main Farm, Trading Alt..."
                    value={aliasInput}
                    onChange={(e) => setAliasInput(e.target.value)}
                  />
                </div>
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div className="input-group">
                  <label className="input-label">Username / Email / Phone</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="Roblox login identity"
                    value={userInput}
                    onChange={(e) => setUserInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label className="input-label">Password</label>
                  <input
                    className="input-field"
                    type="password"
                    placeholder="Account password"
                    value={passInput}
                    onChange={(e) => setPassInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label className="input-label">Alias / Label (Optional)</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="e.g. Alt 1"
                    value={aliasInput}
                    onChange={(e) => setAliasInput(e.target.value)}
                  />
                </div>
              </div>
            )}

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "8px" }}>
              <button type="button"
                className="btn btn-secondary"
                onClick={() => setShowAddModal(false)}
                disabled={submitting}
              >
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={handleAddAccount} disabled={submitting}>
                {submitting ? "Processing..." : "Confirm & Save"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* IMPORT RAM MODAL */}
      {showImportModal && (
        <div className="modal-overlay" onClick={() => !submitting && setShowImportModal(false)}>
          <div className="modal-box" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <Download size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <h3>Import from RAM Directory</h3>
              </div>
              <button type="button"
                className="icon-btn"
                onClick={() => setShowImportModal(false)}
                disabled={submitting}
              >
                <X size={15} strokeWidth={1.75} />
              </button>
            </div>

            <p style={{ color: "var(--text-muted)", fontSize: "12px" }}>
              Specify the path to your legacy Roblox Account Manager folder where AccountData.json is stored.
            </p>

            <div className="input-group">
              <label className="input-label">RAM Directory Path</label>
              <input
                className="input-field"
                type="text"
                placeholder="C:\Users\Username\Roblox-Account-Manager"
                value={ramPathInput}
                onChange={(e) => setRamPathInput(e.target.value)}
              />
            </div>

            <div className="input-group">
              <label className="input-label" htmlFor="ram-vault-password">Vault Password (chỉ cần nếu AccountData.json được khoá mật khẩu)</label>
              <input
                id="ram-vault-password"
                className="input-field"
                type="password"
                placeholder="Để trống nếu file không có mật khẩu"
                value={ramPassInput}
                onChange={(e) => setRamPassInput(e.target.value)}
              />
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "8px" }}>
              <button type="button"
                className="btn btn-secondary"
                onClick={() => setShowImportModal(false)}
                disabled={submitting}
              >
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={handleImportRAM} disabled={submitting}>
                {submitting ? "Importing..." : "Start Import"}
              </button>
            </div>
          </div>
        </div>
      )}

      {editAccount && (
        <div className="modal-overlay" onClick={() => !submitting && setEditAccount(null)}>
          <div className="modal-box" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <SquarePen size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
                <h3>Edit Account</h3>
              </div>
              <button type="button"
                className="icon-btn"
                onClick={() => setEditAccount(null)}
                disabled={submitting}
              >
                <X size={15} strokeWidth={1.75} />
              </button>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
              <div className="input-group">
                <label className="input-label" htmlFor="edit-alias">Alias / Label</label>
                <input
                  id="edit-alias"
                  className="input-field"
                  type="text"
                  placeholder="Để trống sẽ dùng username"
                  value={editAlias}
                  onChange={(e) => setEditAlias(e.target.value)}
                />
              </div>

              <div className="input-group">
                <label className="input-label" htmlFor="edit-group">Group (tiền tố số để sắp xếp, ví dụ: 001 Main)</label>
                <input
                  id="edit-group"
                  className="input-field"
                  type="text"
                  placeholder="Để trống = Default"
                  value={editGroup}
                  onChange={(e) => setEditGroup(e.target.value)}
                />
              </div>

              <div className="input-group">
                <label className="input-label" htmlFor="edit-description">Description</label>
                <input
                  id="edit-description"
                  className="input-field"
                  type="text"
                  value={editDescription}
                  onChange={(e) => setEditDescription(e.target.value)}
                />
              </div>

              <div className="launch-grid">
                <div className="input-group">
                  <label className="input-label" htmlFor="edit-saved-place">Saved Place ID (tùy chọn)</label>
                  <input
                    id="edit-saved-place"
                    className="input-field"
                    type="text"
                    placeholder="Dùng chung nếu để trống"
                    value={editPlaceId}
                    onChange={(e) => setEditPlaceId(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label className="input-label" htmlFor="edit-saved-job">Saved Job ID / VIP Link (tùy chọn)</label>
                  <input
                    id="edit-saved-job"
                    className="input-field"
                    type="text"
                    placeholder="Dùng chung nếu để trống"
                    value={editJobId}
                    onChange={(e) => setEditJobId(e.target.value)}
                  />
                </div>
              </div>
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "8px" }}>
              <button type="button"
                className="btn btn-secondary"
                onClick={() => setEditAccount(null)}
                disabled={submitting}
              >
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={saveEdit} disabled={submitting}>
                {submitting ? "Saving..." : "Save Changes"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* TOAST NOTIFICATION */}
      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
export default App;
