import {
  Download,
  LayoutGrid,
  List,
  Play,
  Plus,
  RefreshCw,
  Search,
  Shield,
  Square,
  SquareCheck,
  X,
} from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { AccountCard } from "./AccountCard";
import { AccountRow } from "./AccountRow";
import { GroupBar } from "./GroupBar";

export function AccountsTab() {
  const accounts = useAppStore((s) => s.accounts);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const viewMode = useAppStore((s) => s.viewMode);
  const setViewMode = useAppStore((s) => s.setViewMode);
  const selectedIds = useAppStore((s) => s.selectedIds);
  const toggleSelectAll = useAppStore((s) => s.toggleSelectAll);
  const launchSelected = useAppStore((s) => s.launchSelected);
  const batchRunning = useAppStore((s) => s.batchRunning);
  const refreshAccounts = useAppStore((s) => s.refreshAccounts);
  const setShowAddModal = useAppStore((s) => s.setShowAddModal);
  const setShowImportModal = useAppStore((s) => s.setShowImportModal);
  const groupFilter = useAppStore((s) => s.groupFilter);

  const filteredAccounts = accounts.filter((acc) => {
    if (groupFilter !== null && (acc.Group?.trim() || "Default") !== groupFilter) {
      return false;
    }

    const q = searchQuery.toLowerCase().trim();
    if (!q) return true;
    const name = (
      acc.Fields?.DisplayName ||
      acc.Alias ||
      acc.Username ||
      ""
    ).toLowerCase();
    const username = (acc.Username || "").toLowerCase();
    const uid = String(acc.UserID || "");
    return name.includes(q) || username.includes(q) || uid.includes(q);
  });

  const isAllSelected =
    filteredAccounts.length > 0 &&
    filteredAccounts.every((acc) => selectedIds.includes(acc.UserID ?? 0));

  return (
    <div className="flex flex-col gap-4">
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
            <button
              type="button"
              className="icon-btn"
              onClick={() => setSearchQuery("")}
            >
              <X size={12} strokeWidth={1.75} />
            </button>
          )}
        </div>

        <div className="view-controls">
          <button
            type="button"
            className={`icon-btn ${viewMode === "grid" ? "active" : ""}`}
            onClick={() => setViewMode("grid")}
            title="Grid view"
          >
            <LayoutGrid size={15} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            className={`icon-btn ${viewMode === "table" ? "active" : ""}`}
            onClick={() => setViewMode("table")}
            title="Table view"
          >
            <List size={15} strokeWidth={1.75} />
          </button>
          <div
            style={{
              width: "1px",
              height: "16px",
              background: "var(--border-medium)",
              margin: "0 4px",
            }}
          />
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={refreshAccounts}
            title="Reload accounts"
          >
            <RefreshCw size={12} strokeWidth={1.75} />
            <span>Refresh</span>
          </button>
        </div>
      </div>

      <GroupBar />

      <div className="batch-bar">
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={toggleSelectAll}
        >
          {isAllSelected ? (
            <SquareCheck size={13} strokeWidth={1.75} />
          ) : (
            <Square size={13} strokeWidth={1.75} />
          )}
          <span>
            {selectedIds.length > 0
              ? `Đã chọn ${selectedIds.length}`
              : "Chọn tất cả"}
          </span>
        </button>
        <button
          type="button"
          className="btn btn-primary btn-sm"
          onClick={launchSelected}
          disabled={batchRunning || selectedIds.length === 0}
          title="Khởi chạy tuần tự các tài khoản đã chọn"
        >
          <Play size={12} fill="currentColor" />
          <span>
            {batchRunning
              ? "Đang chạy..."
              : `Launch ${selectedIds.length || ""}`.trim()}
          </span>
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
            <button
              type="button"
              className="btn btn-primary btn-sm"
              onClick={() => setShowAddModal(true)}
            >
              <Plus size={13} strokeWidth={2} />
              <span>Add New Account</span>
            </button>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => setShowImportModal(true)}
            >
              <Download size={13} strokeWidth={1.75} />
              <span>Import From RAM</span>
            </button>
          </div>
        </div>
      ) : viewMode === "grid" ? (
        <div className="accounts-grid">
          {filteredAccounts.map((acc) => (
            <AccountCard key={acc.UserID ?? 0} account={acc} />
          ))}
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
              {filteredAccounts.map((acc) => (
                <AccountRow key={acc.UserID ?? 0} account={acc} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
