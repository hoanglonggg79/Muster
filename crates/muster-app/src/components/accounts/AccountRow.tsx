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

interface AccountRowProps {
  account: Account;
}

export function AccountRow({ account }: AccountRowProps) {
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
    <tr className={selected ? "selected" : ""}>
      <td>
        <button
          type="button"
          className="acct-check"
          onClick={() => toggleSelect(uid)}
        >
          {selected ? (
            <SquareCheck size={15} strokeWidth={1.75} />
          ) : (
            <Square size={15} strokeWidth={1.75} />
          )}
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
      <td style={{ color: "var(--text-secondary)", fontFamily: "monospace" }}>
        @{username}
      </td>
      <td style={{ color: "var(--text-muted)", fontFamily: "monospace" }}>
        {uid}
      </td>
      <td>
        <span className="account-robux">
          <Coins size={11} strokeWidth={1.75} />
          <span>{robux}</span>
        </span>
      </td>
      <td>
        <span className="group-chip">{groupLabel(account)}</span>
      </td>
      <td>
        <span className={presenceClass(userPresence)}>
          <span className="presence-dot" />
          {presenceLabel(userPresence)}
        </span>
      </td>
      <td style={{ textAlign: "right" }}>
        <div style={{ display: "inline-flex", gap: "6px" }}>
          <button
            type="button"
            className="icon-btn"
            onClick={() => setEditAccount(account)}
            title="Edit"
          >
            <SquarePen size={13} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={() => refreshAccount(uid)}
            title="Refresh"
          >
            <RefreshCw size={13} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            className="icon-btn danger"
            onClick={() => removeAccount(uid, displayName)}
            title="Delete"
          >
            <Trash2 size={13} strokeWidth={1.75} />
          </button>
          <button
            type="button"
            className="btn btn-primary btn-sm"
            onClick={() => launchAccount(uid, account)}
            disabled={phase === "launching"}
          >
            <Play size={10} fill="currentColor" />
            <span>{phase === "launching" ? "..." : "Launch"}</span>
          </button>
        </div>
      </td>
    </tr>
  );
}
