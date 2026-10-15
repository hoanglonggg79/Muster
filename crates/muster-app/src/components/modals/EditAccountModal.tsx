import { Loader2, ShieldCheck, SquarePen, UserCog, X } from "lucide-react";
import { useEffect, useState } from "react";
import { commands } from "../../bindings";
import type { PinInfo } from "../../bindings";
import { useAppStore } from "../../store/useAppStore";

type EditTab = "info" | "account" | "privacy";

const PRIVACY_OPTIONS = [
  { value: 0, label: "Mọi người (All)" },
  { value: 1, label: "Người theo dõi (Followers)" },
  { value: 2, label: "Đang theo dõi (Following)" },
  { value: 3, label: "Bạn bè (Friends)" },
  { value: 4, label: "Không ai (NoOne)" },
];

export function EditAccountModal() {
  const editAccount = useAppStore((s) => s.editAccount);
  const setEditAccount = useAppStore((s) => s.setEditAccount);
  const saveEdit = useAppStore((s) => s.saveEdit);
  const refreshAccounts = useAppStore((s) => s.refreshAccounts);
  const notify = useAppStore((s) => s.notify);

  const [tab, setTab] = useState<EditTab>("info");
  const [submitting, setSubmitting] = useState(false);

  const [editAlias, setEditAlias] = useState("");
  const [editGroup, setEditGroup] = useState("");
  const [editDescription, setEditDescription] = useState("");
  const [editPlaceId, setEditPlaceId] = useState("");
  const [editJobId, setEditJobId] = useState("");

  const [displayName, setDisplayName] = useState("");
  const [avatarJson, setAvatarJson] = useState("");
  const [pin, setPin] = useState("");
  const [pinInfo, setPinInfo] = useState<PinInfo | null>(null);
  const [pinBusy, setPinBusy] = useState(false);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [emailPassword, setEmailPassword] = useState("");
  const [newEmail, setNewEmail] = useState("");
  const [accountBusy, setAccountBusy] = useState(false);

  const [privacy, setPrivacy] = useState(0);
  const [blocked, setBlocked] = useState<number[]>([]);
  const [blockTarget, setBlockTarget] = useState("");
  const [privacyBusy, setPrivacyBusy] = useState(false);

  const uid = editAccount?.UserID ?? 0;

  useEffect(() => {
    if (editAccount) {
      setEditAlias(editAccount.Alias ?? "");
      setEditGroup(editAccount.Group ?? "");
      setEditDescription(editAccount.Description ?? "");
      setEditPlaceId(editAccount.Fields?.SavedPlaceId ?? "");
      setEditJobId(editAccount.Fields?.SavedJobId ?? "");
      setDisplayName(editAccount.Fields?.DisplayName ?? editAccount.Username ?? "");
      setAvatarJson("");
      setPin("");
      setPinInfo(null);
      setBlocked([]);
      setBlockTarget("");
      setTab("info");
    }
  }, [editAccount]);

  if (!editAccount) return null;

  const busy = submitting || accountBusy || privacyBusy || pinBusy;

  function handleClose() {
    if (!busy) {
      setEditAccount(null);
    }
  }

  async function handleSave() {
    setSubmitting(true);
    try {
      await saveEdit({
        alias: editAlias,
        group: editGroup,
        description: editDescription,
        placeId: editPlaceId,
        jobId: editJobId,
      });
    } finally {
      setSubmitting(false);
    }
  }

  async function handleCheckPin() {
    setPinBusy(true);
    try {
      const res = await commands.accountCheckPin(uid);
      if (res.status === "ok") {
        setPinInfo(res.data);
        notify(res.data.enabled ? "Tài khoản có bật PIN" : "Tài khoản không bật PIN");
      } else {
        notify(`Lỗi kiểm tra PIN: ${res.error}`);
      }
    } finally {
      setPinBusy(false);
    }
  }

  async function handleUnlockPin() {
    if (pin.length !== 4) {
      notify("PIN phải gồm đúng 4 chữ số");
      return;
    }
    setPinBusy(true);
    try {
      const res = await commands.accountUnlockPin(uid, pin);
      if (res.status === "ok") {
        setPinInfo(res.data);
        notify(res.data.is_unlocked ? "Đã mở khoá PIN" : "Mở khoá thất bại");
        setPin("");
      } else {
        notify(`Lỗi mở khoá PIN: ${res.error}`);
      }
    } finally {
      setPinBusy(false);
    }
  }

  async function handleSetDisplayName() {
    if (!displayName.trim()) {
      notify("Display name không được để trống");
      return;
    }
    setAccountBusy(true);
    try {
      const res = await commands.accountSetDisplayName(uid, displayName.trim());
      if (res.status === "ok") {
        notify("Đã đổi display name");
        await refreshAccounts();
      } else {
        notify(`Lỗi đổi display name: ${res.error}`);
      }
    } finally {
      setAccountBusy(false);
    }
  }

  async function handleSetAvatar() {
    if (!avatarJson.trim()) {
      notify("Dán JSON avatar (playerAvatarType/scales/bodyColors/assets)");
      return;
    }
    setAccountBusy(true);
    try {
      const res = await commands.accountSetAvatar(uid, avatarJson.trim());
      if (res.status === "ok") {
        const invalid = res.data.filter((id): id is number => id !== null);
        notify(
          invalid.length > 0
            ? `Đã gửi avatar, ${invalid.length} asset không hợp lệ: ${invalid.join(", ")}`
            : "Đã áp dụng avatar"
        );
      } else {
        notify(`Lỗi avatar: ${res.error}`);
      }
    } finally {
      setAccountBusy(false);
    }
  }

  async function handleChangePassword() {
    if (!currentPassword || !newPassword) {
      notify("Nhập mật khẩu hiện tại và mật khẩu mới");
      return;
    }
    setAccountBusy(true);
    try {
      const res = await commands.accountChangePassword(uid, currentPassword, newPassword);
      if (res.status === "ok") {
        notify("Đã đổi mật khẩu (cookie đã được làm mới)");
        setCurrentPassword("");
        setNewPassword("");
      } else {
        notify(`Lỗi đổi mật khẩu: ${res.error}`);
      }
    } finally {
      setAccountBusy(false);
    }
  }

  async function handleChangeEmail() {
    if (!emailPassword || !newEmail) {
      notify("Nhập mật khẩu và email mới");
      return;
    }
    setAccountBusy(true);
    try {
      const res = await commands.accountChangeEmail(uid, emailPassword, newEmail);
      if (res.status === "ok") {
        notify("Đã đổi email");
        setEmailPassword("");
        setNewEmail("");
      } else {
        notify(`Lỗi đổi email: ${res.error}`);
      }
    } finally {
      setAccountBusy(false);
    }
  }

  async function handleLogoutSessions() {
    setAccountBusy(true);
    try {
      const res = await commands.accountLogoutOtherSessions(uid);
      if (res.status === "ok") {
        notify("Đã đăng xuất khỏi mọi phiên khác");
        await refreshAccounts();
      } else {
        notify(`Lỗi đăng xuất phiên khác: ${res.error}`);
      }
    } finally {
      setAccountBusy(false);
    }
  }

  async function handleSavePrivacy() {
    setPrivacyBusy(true);
    try {
      const res = await commands.accountSetFollowPrivacy(uid, privacy);
      if (res.status === "ok") {
        notify("Đã lưu quyền riêng tư theo dõi");
      } else {
        notify(`Lỗi quyền riêng tư: ${res.error}`);
      }
    } finally {
      setPrivacyBusy(false);
    }
  }

  async function handleLoadBlocked() {
    setPrivacyBusy(true);
    try {
      const res = await commands.accountBlockedList(uid);
      if (res.status === "ok") {
        setBlocked(res.data.filter((id): id is number => id !== null));
      } else {
        notify(`Lỗi tải danh sách chặn: ${res.error}`);
      }
    } finally {
      setPrivacyBusy(false);
    }
  }

  async function handleBlockToggle(unblock: boolean) {
    const target = Number(blockTarget.replace(/[^0-9]/g, ""));
    if (!target) {
      notify("Nhập UserID cần chặn/bỏ chặn");
      return;
    }
    setPrivacyBusy(true);
    try {
      const res = unblock
        ? await commands.accountUnblockUser(uid, target)
        : await commands.accountBlockUser(uid, target);
      if (res.status === "ok") {
        notify(res.data ? (unblock ? "Đã bỏ chặn" : "Đã chặn") : "Roblox từ chối thao tác");
        await handleLoadBlocked();
      } else {
        notify(`Lỗi: ${res.error}`);
      }
    } finally {
      setPrivacyBusy(false);
    }
  }

  async function handleUnblockEveryone() {
    setPrivacyBusy(true);
    try {
      const res = await commands.accountUnblockEveryone(uid);
      if (res.status === "ok") {
        notify(`Đã bỏ chặn ${res.data} tài khoản`);
        await handleLoadBlocked();
      } else {
        notify(`Lỗi bỏ chặn tất cả: ${res.error}`);
      }
    } finally {
      setPrivacyBusy(false);
    }
  }

  const tabs: { id: EditTab; label: string; icon: typeof SquarePen }[] = [
    { id: "info", label: "Thông tin", icon: SquarePen },
    { id: "account", label: "Tài khoản", icon: UserCog },
    { id: "privacy", label: "Quyền riêng tư", icon: ShieldCheck },
  ];

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
        aria-labelledby="edit-account-title"
      >
        <div className="modal-header">
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <SquarePen size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
            <h3 id="edit-account-title">Edit Account</h3>
          </div>
          <button
            type="button"
            className="icon-btn"
            onClick={handleClose}
            disabled={busy}
            aria-label="Close"
          >
            <X size={15} strokeWidth={1.75} />
          </button>
        </div>

        <div className="modal-tabs" role="tablist">
          {tabs.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              className={`modal-tab ${tab === id ? "active" : ""}`}
              onClick={() => setTab(id)}
              disabled={busy}
              style={{ display: "flex", alignItems: "center", gap: "5px" }}
            >
              <Icon size={12} strokeWidth={1.75} />
              {label}
            </button>
          ))}
        </div>

        {tab === "info" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            <div className="input-group">
              <label className="input-label" htmlFor="edit-account-alias">
                Alias / Label
              </label>
              <input
                id="edit-account-alias"
                className="input-field"
                type="text"
                placeholder="Để trống sẽ dùng username"
                value={editAlias}
                onChange={(e) => setEditAlias(e.target.value)}
              />
            </div>

            <div className="input-group">
              <label className="input-label" htmlFor="edit-account-group">
                Group (tiền tố số để sắp xếp, ví dụ: 001 Main)
              </label>
              <input
                id="edit-account-group"
                className="input-field"
                type="text"
                placeholder="Để trống = Default"
                value={editGroup}
                onChange={(e) => setEditGroup(e.target.value)}
              />
            </div>

            <div className="input-group">
              <label className="input-label" htmlFor="edit-account-description">
                Description
              </label>
              <input
                id="edit-account-description"
                className="input-field"
                type="text"
                value={editDescription}
                onChange={(e) => setEditDescription(e.target.value)}
              />
            </div>

            <div className="launch-grid">
              <div className="input-group">
                <label className="input-label" htmlFor="edit-account-saved-place">
                  Saved Place ID (tùy chọn)
                </label>
                <input
                  id="edit-account-saved-place"
                  className="input-field"
                  type="text"
                  placeholder="Dùng chung nếu để trống"
                  value={editPlaceId}
                  onChange={(e) => setEditPlaceId(e.target.value)}
                />
              </div>
              <div className="input-group">
                <label className="input-label" htmlFor="edit-account-saved-job">
                  Saved Job ID / VIP Link (tùy chọn)
                </label>
                <input
                  id="edit-account-saved-job"
                  className="input-field"
                  type="text"
                  placeholder="Dùng chung nếu để trống"
                  value={editJobId}
                  onChange={(e) => setEditJobId(e.target.value)}
                />
              </div>
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
              <button type="button" className="btn btn-secondary" onClick={handleClose} disabled={busy}>
                Cancel
              </button>
              <button type="button" className="btn btn-primary" onClick={handleSave} disabled={busy}>
                {submitting ? "Saving..." : "Save Changes"}
              </button>
            </div>
          </div>
        )}

        {tab === "account" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <div className="input-group">
              <label className="input-label" htmlFor="edit-display-name">
                Display Name (tên hiển thị Roblox)
              </label>
              <div style={{ display: "flex", gap: "8px" }}>
                <input
                  id="edit-display-name"
                  className="input-field"
                  type="text"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                />
                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={handleSetDisplayName}
                  disabled={accountBusy}
                >
                  Đổi
                </button>
              </div>
            </div>

            <div className="input-group">
              <label className="input-label" htmlFor="edit-avatar-json">
                Avatar JSON (playerAvatarType / scales / bodyColors / assets)
              </label>
              <textarea
                id="edit-avatar-json"
                className="input-field"
                rows={3}
                placeholder='{"playerAvatarType":"R15","scales":{"height":1.0}}'
                value={avatarJson}
                onChange={(e) => setAvatarJson(e.target.value)}
                style={{ fontFamily: "monospace", fontSize: "12px" }}
              />
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleSetAvatar}
                disabled={accountBusy}
                style={{ alignSelf: "flex-start", marginTop: "6px" }}
              >
                Áp dụng avatar
              </button>
            </div>

            <div className="settings-section" style={{ padding: "12px 14px" }}>
              <span className="input-label" style={{ display: "block", marginBottom: "8px" }}>
                PIN bảo vệ tài khoản
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={handleCheckPin}
                  disabled={pinBusy}
                >
                  {pinBusy ? <Loader2 size={12} style={{ animation: "spin 1s linear infinite" }} /> : null}
                  Kiểm tra PIN
                </button>
                <input
                  className="input-field"
                  style={{ width: "90px", height: "28px", fontFamily: "monospace" }}
                  type="text"
                  inputMode="numeric"
                  maxLength={4}
                  placeholder="PIN"
                  value={pin}
                  onChange={(e) => setPin(e.target.value.replace(/[^0-9]/g, ""))}
                  aria-label="Mã PIN 4 chữ số"
                />
                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={handleUnlockPin}
                  disabled={pinBusy}
                >
                  Mở khoá PIN
                </button>
                {pinInfo && (
                  <span style={{ fontSize: "12px", color: "var(--text-muted)" }}>
                    {pinInfo.enabled
                      ? pinInfo.is_unlocked
                        ? "Đang mở khoá"
                        : "Đang bị khoá"
                      : "Không bật PIN"}
                  </span>
                )}
              </div>
            </div>

            <div className="launch-grid">
              <div className="input-group">
                <label className="input-label" htmlFor="edit-current-password">
                  Mật khẩu hiện tại
                </label>
                <input
                  id="edit-current-password"
                  className="input-field"
                  type="password"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                />
              </div>
              <div className="input-group">
                <label className="input-label" htmlFor="edit-new-password">
                  Mật khẩu mới
                </label>
                <input
                  id="edit-new-password"
                  className="input-field"
                  type="password"
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                />
              </div>
            </div>
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={handleChangePassword}
              disabled={accountBusy}
              style={{ alignSelf: "flex-start" }}
            >
              Đổi mật khẩu
            </button>

            <div className="launch-grid">
              <div className="input-group">
                <label className="input-label" htmlFor="edit-email-password">
                  Mật khẩu (để xác nhận)
                </label>
                <input
                  id="edit-email-password"
                  className="input-field"
                  type="password"
                  value={emailPassword}
                  onChange={(e) => setEmailPassword(e.target.value)}
                />
              </div>
              <div className="input-group">
                <label className="input-label" htmlFor="edit-new-email">
                  Email mới
                </label>
                <input
                  id="edit-new-email"
                  className="input-field"
                  type="email"
                  value={newEmail}
                  onChange={(e) => setNewEmail(e.target.value)}
                />
              </div>
            </div>
            <div style={{ display: "flex", gap: "8px", flexWrap: "wrap" }}>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleChangeEmail}
                disabled={accountBusy}
              >
                Đổi email
              </button>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={handleLogoutSessions}
                disabled={accountBusy}
              >
                Đăng xuất mọi phiên khác
              </button>
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "4px" }}>
              <button type="button" className="btn btn-secondary" onClick={handleClose} disabled={busy}>
                Đóng
              </button>
            </div>
          </div>
        )}

        {tab === "privacy" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <div className="input-group">
              <label className="input-label" htmlFor="edit-follow-privacy">
                Ai được phép theo dõi (follow) tài khoản này
              </label>
              <div style={{ display: "flex", gap: "8px" }}>
                <select
                  id="edit-follow-privacy"
                  className="input-field"
                  value={privacy}
                  onChange={(e) => setPrivacy(Number(e.target.value))}
                >
                  {PRIVACY_OPTIONS.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={handleSavePrivacy}
                  disabled={privacyBusy}
                >
                  Lưu
                </button>
              </div>
            </div>

            <div className="settings-section" style={{ padding: "12px 14px" }}>
              <span className="input-label" style={{ display: "block", marginBottom: "8px" }}>
                Danh sách chặn
              </span>
              <div style={{ display: "flex", alignItems: "center", gap: "8px", flexWrap: "wrap" }}>
                <input
                  className="input-field"
                  style={{ width: "130px", height: "28px" }}
                  type="text"
                  inputMode="numeric"
                  placeholder="UserID"
                  value={blockTarget}
                  onChange={(e) => setBlockTarget(e.target.value.replace(/[^0-9]/g, ""))}
                  aria-label="UserID cần chặn"
                />
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => handleBlockToggle(false)}
                  disabled={privacyBusy}
                >
                  Chặn
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={() => handleBlockToggle(true)}
                  disabled={privacyBusy}
                >
                  Bỏ chặn
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={handleLoadBlocked}
                  disabled={privacyBusy}
                >
                  Tải danh sách
                </button>
                <button
                  type="button"
                  className="btn btn-secondary btn-sm"
                  onClick={handleUnblockEveryone}
                  disabled={privacyBusy}
                >
                  Bỏ chặn tất cả
                </button>
              </div>
              {blocked.length > 0 && (
                <div
                  style={{
                    marginTop: "8px",
                    fontSize: "12px",
                    color: "var(--text-muted)",
                    fontFamily: "monospace",
                  }}
                >
                  Đang chặn: {blocked.join(", ")}
                </div>
              )}
            </div>

            <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "4px" }}>
              <button type="button" className="btn btn-secondary" onClick={handleClose} disabled={busy}>
                Đóng
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
