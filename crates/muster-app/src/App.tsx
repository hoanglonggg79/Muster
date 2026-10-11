import { useState, useEffect } from "react";
import { commands } from "./bindings";
import type { Account } from "./bindings";

export function App() {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [placeId, setPlaceId] = useState("4483381587");
  const [jobId, setJobId] = useState("");
  const [multiRoblox, setMultiRoblox] = useState(false);
  const [unlockFps, setUnlockFps] = useState(true);
  const [maxFps, setMaxFps] = useState(240);
  const [toast, setToast] = useState<string | null>(null);

  const [showAddModal, setShowAddModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [addTab, setAddTab] = useState<"cookie" | "credentials">("cookie");

  const [cookieInput, setCookieInput] = useState("");
  const [aliasInput, setAliasInput] = useState("");
  const [userInput, setUserInput] = useState("");
  const [passInput, setPassInput] = useState("");
  const [ramPathInput, setRamPathInput] = useState("");

  const [submitting, setSubmitting] = useState(false);

  function notify(msg: string) {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  }

  async function refreshAccounts() {
    try {
      const res = await commands.accountList();
      if (res.status === "ok") {
        setAccounts(res.data);
      }
    } catch (e) {
      notify(`Lỗi tải danh sách: ${String(e)}`);
    }
  }

  useEffect(() => {
    refreshAccounts();
  }, []);

  async function toggleMultiRoblox() {
    try {
      const res = await commands.multiRobloxToggle(!multiRoblox);
      if (res.status === "ok") {
        setMultiRoblox(res.data);
        notify(res.data ? "Đã BẬT Multi-Roblox instance" : "Đã TẮT Multi-Roblox");
      }
    } catch (e) {
      notify(`Lỗi Multi-Roblox: ${String(e)}`);
    }
  }

  async function applyFps(newUnlock: boolean, fpsVal: number) {
    try {
      const res = await commands.patchApplyFps(newUnlock, fpsVal);
      if (res.status === "ok") {
        notify(res.data);
      } else {
        notify(`Lỗi FPS: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi FPS: ${String(e)}`);
    }
  }

  async function launchAccount(userId: number) {
    const pid = Number(placeId);
    if (!pid || isNaN(pid)) {
      notify("Vui lòng nhập Place ID hợp lệ");
      return;
    }
    notify("Đang khởi chạy Roblox client...");
    try {
      const res = await commands.accountLaunch(userId, pid, jobId.trim() || null);
      if (res.status === "ok") {
        notify(res.data);
      } else {
        notify(`Lỗi khởi chạy: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi launch: ${String(e)}`);
    }
  }

  async function refreshAccount(userId: number) {
    try {
      const res = await commands.accountRefresh(userId);
      if (res.status === "ok") {
        notify(`Đã cập nhật: ${res.data.Username ?? "User"}`);
        refreshAccounts();
      } else {
        notify(`Lỗi làm mới: ${res.error}`);
      }
    } catch (e) {
      notify(`Lỗi: ${String(e)}`);
    }
  }

  async function removeAccount(userId: number, name: string) {
    if (!confirm(`Bạn có chắc muốn xoá tài khoản ${name}?`)) return;
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
          notify(`Thêm thành công tài khoản ${res.data.Username ?? ""}`);
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
      notify("Vui lòng nhập đường dẫn thư mục RAM");
      return;
    }
    setSubmitting(true);
    try {
      const res = await commands.vaultImportRam(ramPathInput.trim());
      if (res.status === "ok") {
        notify(`Đã nhập thành công ${res.data} tài khoản từ RAM!`);
        setShowImportModal(false);
        setRamPathInput("");
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

  return (
    <div className="app">
      <header className="navbar">
        <div className="navbar__brand">
          <div className="brand-icon">M</div>
          <div className="brand-text">
            <h1>MUSTER</h1>
            <p>Roblox Account Manager & Launcher</p>
          </div>
        </div>

        <div className="navbar__actions">
          <div
            className={`toggle-badge ${multiRoblox ? "active" : ""}`}
            onClick={toggleMultiRoblox}
            title="Cho phép mở nhiều tài khoản Roblox cùng lúc"
          >
            <span className="status-dot"></span>
            Multi-Roblox: {multiRoblox ? "BẬT" : "TẮT"}
          </div>

          <div
            className={`toggle-badge ${unlockFps ? "active" : ""}`}
            onClick={() => {
              let nextUnlock = unlockFps;
              let nextFps = maxFps;
              if (!unlockFps) {
                nextUnlock = true;
                nextFps = 144;
              } else if (maxFps === 144) {
                nextFps = 240;
              } else if (maxFps === 240) {
                nextFps = 360;
              } else {
                nextUnlock = false;
                nextFps = 60;
              }
              setUnlockFps(nextUnlock);
              setMaxFps(nextFps);
              applyFps(nextUnlock, nextFps);
            }}
            title="Nhấp để chuyển đổi: 144 -> 240 -> 360 -> Tắt"
          >
            <span className="status-dot"></span>
            FPS: {unlockFps ? `${maxFps} FPS` : "Mặc định"}
          </div>

          <button className="btn btn-secondary btn-sm" onClick={() => setShowImportModal(true)}>
            Import RAM
          </button>
          <button className="btn btn-primary btn-sm" onClick={() => setShowAddModal(true)}>
            + Thêm Tài Khoản
          </button>
        </div>
      </header>

      <main className="main-container">
        <section className="launch-bar">
          <div className="launch-bar__inputs">
            <div className="input-group">
              <label>Place ID (Mã Game)</label>
              <input
                className="input-field"
                type="text"
                value={placeId}
                onChange={(e) => setPlaceId(e.target.value)}
                placeholder="Ví dụ: 4483381587"
              />
            </div>
            <div className="input-group">
              <label>Job ID (Server ID - Tuỳ chọn)</label>
              <input
                className="input-field"
                type="text"
                value={jobId}
                onChange={(e) => setJobId(e.target.value)}
                placeholder="Để trống để vào server ngẫu nhiên"
              />
            </div>
          </div>
        </section>

        <section className="accounts-header">
          <h2>
            Tài Khoản <span className="count-badge">{accounts.length}</span>
          </h2>
          <button className="btn btn-secondary btn-sm" onClick={refreshAccounts}>
            ↻ Làm Mới Danh Sách
          </button>
        </section>

        {accounts.length === 0 ? (
          <div className="empty-state">
            <h3>Chưa có tài khoản nào</h3>
            <p style={{ color: "var(--text-muted)", fontSize: "13px" }}>
              Hãy thêm tài khoản bằng cookie/mật khẩu hoặc nhập dữ liệu từ RAM cũ.
            </p>
            <div style={{ display: "flex", gap: "10px", marginTop: "10px" }}>
              <button className="btn btn-primary" onClick={() => setShowAddModal(true)}>
                + Thêm Tài Khoản Mới
              </button>
              <button className="btn btn-secondary" onClick={() => setShowImportModal(true)}>
                Nhập từ RAM cũ
              </button>
            </div>
          </div>
        ) : (
          <div className="accounts-grid">
            {accounts.map((acc) => {
              const uid = acc.UserID ?? 0;
              const displayName = acc.Fields?.DisplayName || acc.Alias || acc.Username || "Roblox Player";
              const username = acc.Username || `ID: ${uid}`;
              const robux = acc.Fields?.Robux || "0";
              const avatarUrl = acc.Fields?.AvatarUrl;

              return (
                <div key={uid} className="account-card">
                  <div className="account-card__top">
                    <div className="avatar-wrapper">
                      {avatarUrl ? (
                        <img className="avatar-img" src={avatarUrl} alt={username} />
                      ) : (
                        <span className="avatar-fallback">{displayName.charAt(0).toUpperCase()}</span>
                      )}
                    </div>
                    <div className="account-info">
                      <div className="account-name" title={displayName}>
                        {displayName}
                      </div>
                      <div className="account-username">@{username}</div>
                      <div className="robux-badge">
                        <span>R$</span> {robux}
                      </div>
                    </div>
                  </div>

                  <div className="account-card__bottom">
                    <div style={{ display: "flex", gap: "6px" }}>
                      <button
                        className="btn btn-secondary btn-sm"
                        onClick={() => refreshAccount(uid)}
                        title="Cập nhật lại Robux và Avatar"
                      >
                        ↻ Làm mới
                      </button>
                      <button
                        className="btn btn-danger btn-sm"
                        onClick={() => removeAccount(uid, displayName)}
                        title="Xoá khỏi danh sách"
                      >
                        Xoá
                      </button>
                    </div>

                    <div className="account-card__actions">
                      <button className="btn btn-primary btn-sm" onClick={() => launchAccount(uid)}>
                        ▶ Launch
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </main>

      {showAddModal && (
        <div className="modal-overlay" onClick={() => !submitting && setShowAddModal(false)}>
          <div className="modal-box" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Thêm Tài Khoản</h3>
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setShowAddModal(false)}
                disabled={submitting}
              >
                ✕
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
                Tên & Mật khẩu (API)
              </div>
            </div>

            {addTab === "cookie" ? (
              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div className="input-group">
                  <label>Cookie .ROBLOSECURITY</label>
                  <textarea
                    className="input-field"
                    rows={4}
                    placeholder="_|WARNING:-DO-NOT-SHARE-THIS..."
                    value={cookieInput}
                    onChange={(e) => setCookieInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label>Tên gợi nhớ (Alias - Tuỳ chọn)</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="Ví dụ: Acc chính, Farm Acc..."
                    value={aliasInput}
                    onChange={(e) => setAliasInput(e.target.value)}
                  />
                </div>
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
                <div className="input-group">
                  <label>Username / Email / Số điện thoại</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="Tên đăng nhập Roblox"
                    value={userInput}
                    onChange={(e) => setUserInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label>Mật khẩu</label>
                  <input
                    className="input-field"
                    type="password"
                    placeholder="Mật khẩu"
                    value={passInput}
                    onChange={(e) => setPassInput(e.target.value)}
                  />
                </div>
                <div className="input-group">
                  <label>Tên gợi nhớ (Alias - Tuỳ chọn)</label>
                  <input
                    className="input-field"
                    type="text"
                    placeholder="Ví dụ: Alt 1"
                    value={aliasInput}
                    onChange={(e) => setAliasInput(e.target.value)}
                  />
                </div>
              </div>
            )}

            <button className="btn btn-primary" onClick={handleAddAccount} disabled={submitting}>
              {submitting ? "Đang xử lý..." : "Xác nhận & Thêm"}
            </button>
          </div>
        </div>
      )}

      {showImportModal && (
        <div className="modal-overlay" onClick={() => !submitting && setShowImportModal(false)}>
          <div className="modal-box" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Nhập dữ liệu từ RAM (Roblox Account Manager)</h3>
              <button
                className="btn btn-secondary btn-sm"
                onClick={() => setShowImportModal(false)}
                disabled={submitting}
              >
                ✕
              </button>
            </div>

            <p style={{ color: "var(--text-muted)", fontSize: "12px" }}>
              Nhập đường dẫn tới thư mục cài đặt của RAM cũ (nơi chứa file AccountData.json).
            </p>

            <div className="input-group">
              <label>Đường dẫn thư mục RAM</label>
              <input
                className="input-field"
                type="text"
                placeholder="C:\Users\...\Roblox-Account-Manager"
                value={ramPathInput}
                onChange={(e) => setRamPathInput(e.target.value)}
              />
            </div>

            <button className="btn btn-primary" onClick={handleImportRAM} disabled={submitting}>
              {submitting ? "Đang nhập..." : "Nhập Dữ Liệu"}
            </button>
          </div>
        </div>
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
