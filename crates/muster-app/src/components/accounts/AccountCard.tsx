import {
  Coins,
  Play,
  RefreshCw,
  Square,
  SquareCheck,
  SquarePen,
  Trash2,
} from "lucide-react";
import type { Account } from "../../bindings";
import {
  groupLabel,
  presenceClass,
  presenceLabel,
} from "../../lib/accountUtils";
import { useAppStore } from "../../store/useAppStore";

interface AccountCardProps {
  account: Account;
}

export function AccountCard({ account }: AccountCardProps) {
  const selectedIds = useAppStore((s) => s.selectedIds);
  const toggleSelect = useAppStore((s) => s.toggleSelect);
  const launchPhases = useAppStore((s) => s.launchPhases);
  const presence = useAppStore((s) => s.presence);
  const setEditAccount = useAppStore((s) => s.setEditAccount);
  const refreshAccount = useAppStore((s) => s.refreshAccount);
  const removeAccount = useAppStore((s) => s.removeAccount);
  const launchAccount = useAppStore((s) => s.launchAccount);

  const uid = account.UserID ?? 0;
  const displayName =
    account.Fields?.DisplayName ||
    account.Alias ||
    account.Username ||
    "Roblox Player";
  const username = account.Username || `ID: ${uid}`;
  const robux = account.Fields?.Robux || "0";
  const avatarUrl = account.Fields?.AvatarUrl;
  const selected = selectedIds.includes(uid);
  const phase = launchPhases[uid];
  const userPresence = presence[uid] ?? 0;

  return (
    <div className={`account-card ${selected ? "selected" : ""}`}>
      <div className="account-card__header">
        <button
          type="button"
          className="acct-check"
          onClick={() => toggleSelect(uid)}
          title={selected ? "Bỏ chọn" : "Chọn để launch hàng loạt"}
        >
          {selected ? (
            <SquareCheck size={16} strokeWidth={1.75} />
          ) : (
            <Square size={16} strokeWidth={1.75} />
          )}
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
        <span
          className="group-chip"
          title={`Nhóm: ${account.Group ?? "Default"}`}
        >
          {groupLabel(account)}
        </span>
        <span className={presenceClass(userPresence)}>
          <span className="presence-dot" />
          {presenceLabel(userPresence)}
        </span>
      </div>

      <div className="account-card__footer">
        <div className="card-action-group">
          <button
            type="button"
            className="icon-btn"
            onClick={() => setEditAccount(account)}
            title="Sửa tài khoản"
          >
            <SquarePen size={13} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={() => refreshAccount(uid)}
            title="Refresh profile & Robux"
          >
            <RefreshCw size={13} strokeWidth={1.75} />
          </button>
          <button
            type="button"
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
          onClick={() => launchAccount(uid, account)}
          disabled={phase === "launching"}
        >
          <Play size={11} fill="currentColor" />
          <span>{phase === "launching" ? "Đang chạy" : "Launch"}</span>
        </button>
      </div>
    </div>
  );
}
