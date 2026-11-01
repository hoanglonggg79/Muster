import {
  Cookie,
  FileText,
  Globe,
  KeyRound,
  Loader2,
  ShieldCheck,
  Users,
  X,
} from "lucide-react";
import { type DragEvent, useState } from "react";
import { commands } from "../../bindings";
import { useAppStore } from "../../store/useAppStore";

type AddTab = "cookie" | "browser" | "tkpass";
type TkPassMode = "api" | "browser";

interface LineResult {
  id: number;
  label: string;
  status: "pending" | "ok" | "fail";
  msg?: string;
}


function readDroppedFile(event: DragEvent<HTMLElement>, apply: (text: string) => void) {
  event.preventDefault();
  event.stopPropagation();
  const file = event.dataTransfer.files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => apply(String(reader.result ?? ""));
  reader.readAsText(file);
}

function ResultList({ rows }: { rows: LineResult[] }) {
  return (
    <div
      style={{
        maxHeight: "200px",
        overflowY: "auto",
        display: "flex",
        flexDirection: "column",
        gap: "4px",
      }}
    >
      {rows.map((row) => (
        <div
          key={row.id}
          style={{
            display: "flex",
            alignItems: "center",
            gap: "8px",
            padding: "6px 10px",
            background: "var(--bg-tertiary)",
            borderRadius: "6px",
            fontSize: "12px",
          }}
        >
          {row.status === "pending" ? (
            <Loader2
              size={12}
              strokeWidth={1.75}
              style={{
                color: "var(--text-muted)",
                animation: "spin 1s linear infinite",
                flexShrink: 0,
              }}
            />
          ) : (
            <span
              style={{
                width: "8px",
                height: "8px",
                borderRadius: "50%",
                background: row.status === "ok" ? "#4ade80" : "#f87171",
                flexShrink: 0,
              }}
            />
          )}
          <span
            style={{
              color:
                row.status === "ok"
                  ? "#4ade80"
                  : row.status === "fail"
                    ? "#f87171"
                    : "var(--text-muted)",
              fontFamily: "monospace",
              flex: 1,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {row.label}
          </span>
          {row.msg && (
            <span style={{ color: "var(--text-muted)", fontSize: "11px", flexShrink: 0 }}>
              {row.msg}
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

export function AddAccountModal() {
  const showAddModal = useAppStore((s) => s.showAddModal);
  const setShowAddModal = useAppStore((s) => s.setShowAddModal);
  const addAccountCookie = useAppStore((s) => s.addAccountCookie);
  const refreshAccounts = useAppStore((s) => s.refreshAccounts);
  const refreshGroups = useAppStore((s) => s.refreshGroups);
  const notify = useAppStore((s) => s.notify);

  const browserBusy = useAppStore((s) => s.browserBusy);
  const openBrowserLogin = useAppStore((s) => s.openBrowserLogin);

  const [addTab, setAddTab] = useState<AddTab>("cookie");
  const [cookieInput, setCookieInput] = useState("");
  const [aliasInput, setAliasInput] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [cookieResults, setCookieResults] = useState<LineResult[]>([]);

  const [tkpassInput, setTkpassInput] = useState("");
  const [tkpassMode, setTkpassMode] = useState<TkPassMode>("api");
  const [tkpassProcessing, setTkpassProcessing] = useState(false);
  const [tkpassResults, setTkpassResults] = useState<LineResult[]>([]);

  if (!showAddModal) return null;

  const isBusy = submitting || tkpassProcessing || browserBusy;

  function handleClose() {
    if (!isBusy) {
      setShowAddModal(false);
      resetAll();
    }
  }

  function resetAll() {
    setCookieInput("");
    setAliasInput("");
    setCookieResults([]);
    setTkpassInput("");
    setTkpassResults([]);
  }

  async function handleCookieConfirm() {
    const lines = cookieInput
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean);

    if (lines.length === 0) {
      notify("Vui lòng dán cookie .ROBLOSECURITY");
      return;
    }

    const alias = aliasInput.trim() || null;

    if (lines.length === 1) {
      setSubmitting(true);
      try {
        const ok = await addAccountCookie(lines[0] ?? "", alias);
        if (ok) {
          await refreshGroups();
          resetAll();
        }
      } finally {
        setSubmitting(false);
      }
      return;
    }

    setSubmitting(true);
    setCookieResults(
      lines.map((_line, index) => ({
        id: index,
        label: `Cookie #${index + 1}`,
        status: "pending",
      }))
    );

    let okCount = 0;
    for (let index = 0; index < lines.length; index++) {
      const cookie = lines[index] ?? "";
      const res = await commands.accountAddCookie(cookie, alias);
      const success = res.status === "ok";
      if (success) okCount++;

      setCookieResults((prev) =>
        prev.map((row, at) =>
          at === index
            ? {
                ...row,
                status: success ? "ok" : "fail",
                msg: success ? res.data.Username ?? "OK" : String(res.error),
              }
            : row
        )
      );

      if (index < lines.length - 1) {
        await new Promise((resolve) => setTimeout(resolve, 400));
      }
    }

    await refreshAccounts();
    await refreshGroups();
    setSubmitting(false);
    notify(`Nhập cookie hàng loạt: ${okCount}/${lines.length} thành công`);
  }

  function parseLines(): { user: string; pass: string; line: string }[] {
    return tkpassInput
      .split("\n")
      .map((line) => line.trim())
      .filter((line) => line.includes(":"))
      .map((line) => {
        const colon = line.indexOf(":");
        return {
          line,
          user: line.slice(0, colon).trim(),
          pass: line.slice(colon + 1).trim(),
        };
      });
  }

  async function handleTkpassSubmit() {
    const parsed = parseLines();
    if (parsed.length === 0) {
      notify("Nhập ít nhất một tài khoản theo định dạng username:password");
      return;
    }

    setTkpassProcessing(true);
    setTkpassResults(
      parsed.map((item, index) => ({
        id: index,
        label: item.user || item.line,
        status: "pending",
      }))
    );

    for (let index = 0; index < parsed.length; index++) {
      const item = parsed[index];
      if (!item) continue;

      if (!item.user || !item.pass) {
        setTkpassResults((prev) =>
          prev.map((row, at) =>
            at === index ? { ...row, status: "fail", msg: "Định dạng sai" } : row
          )
        );
        continue;
      }

      try {
        if (tkpassMode === "browser") {
          const ok = await openBrowserLogin(item.user, item.pass);
          setTkpassResults((prev) =>
            prev.map((row, at) =>
              at === index
                ? { ...row, status: ok ? "ok" : "fail", msg: ok ? "qua trình duyệt" : "thất bại" }
                : row
            )
          );
        } else {
          const res = await commands.accountAddCredentials(item.user, item.pass, null);
          setTkpassResults((prev) =>
            prev.map((row, at) =>
              at === index
                ? res.status === "ok"
                  ? { ...row, status: "ok", msg: res.data.Username ?? "OK" }
                  : { ...row, status: "fail", msg: String(res.error) }
                : row
            )
          );
        }
      } catch (error) {
        setTkpassResults((prev) =>
          prev.map((row, at) => (at === index ? { ...row, status: "fail", msg: String(error) } : row))
        );
      }

      if (index < parsed.length - 1) {
        await new Promise((resolve) => setTimeout(resolve, tkpassMode === "api" ? 1500 : 300));
      }
    }

    await refreshAccounts();
    await refreshGroups();
    setTkpassProcessing(false);
    notify("Hoàn tất đăng nhập hàng loạt");
  }

  const tabDefs: { id: AddTab; label: string; icon: typeof Cookie }[] = [
    { id: "cookie", label: "Cookie", icon: Cookie },
    { id: "browser", label: "Open Browser", icon: Globe },
    { id: "tkpass", label: "TK:PASS", icon: Users },
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
        aria-labelledby="add-account-title"
      >
        <div className="modal-header">
          <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
            <KeyRound size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
            <h3 id="add-account-title">Add Account</h3>
          </div>
          <button
            type="button"
            className="icon-btn"
            onClick={handleClose}
            disabled={isBusy}
            aria-label="Close"
          >
            <X size={15} strokeWidth={1.75} />
          </button>
        </div>

        <div className="modal-tabs" role="tablist">
          {tabDefs.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={addTab === id}
              className={`modal-tab ${addTab === id ? "active" : ""}`}
              onClick={() => setAddTab(id)}
              disabled={isBusy}
              style={{ display: "flex", alignItems: "center", gap: "5px" }}
            >
              <Icon size={12} strokeWidth={1.75} />
              {label}
            </button>
          ))}
        </div>

        {addTab === "cookie" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            {cookieResults.length === 0 ? (
              <>
                <div className="input-group">
                  <label className="input-label" htmlFor="modal-cookie-val">
                    Cookie Value (.ROBLOSECURITY) — mỗi cookie một dòng
                  </label>
                  <textarea
                    id="modal-cookie-val"
                    className="input-field"
                    rows={5}
                    placeholder={"_|WARNING:-DO-NOT-SHARE-THIS.--Sharing-this-will-allow-someone-to-log-in-as-you...\n(dán nhiều dòng để nhập hàng loạt)"}
                    value={cookieInput}
                    onChange={(e) => setCookieInput(e.target.value)}
                    onDragOver={(e) => e.preventDefault()}
                    onDrop={(e) => readDroppedFile(e, (text) => setCookieInput(text.trim()))}
                    disabled={submitting}
                    style={{ fontFamily: "monospace", fontSize: "12px" }}
                  />
                </div>

                <div className="drop-hint">
                  <FileText size={12} strokeWidth={1.75} />
                  <span>Hoặc kéo-thả file .txt (mỗi dòng một cookie) vào ô trên</span>
                </div>

                <div className="input-group">
                  <label className="input-label" htmlFor="modal-cookie-alias">
                    Alias / Account Label (Optional)
                  </label>
                  <input
                    id="modal-cookie-alias"
                    className="input-field"
                    type="text"
                    placeholder="e.g. Main Farm, Trading Alt..."
                    value={aliasInput}
                    onChange={(e) => setAliasInput(e.target.value)}
                    disabled={submitting}
                  />
                </div>
              </>
            ) : (
              <ResultList rows={cookieResults} />
            )}

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleClose}
                disabled={submitting}
              >
                {submitting ? "Đang xử lý..." : "Đóng"}
              </button>
              {cookieResults.length === 0 && (
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleCookieConfirm}
                  disabled={submitting}
                >
                  {submitting ? "Processing..." : "Confirm & Save"}
                </button>
              )}
            </div>
          </div>
        )}

        {addTab === "browser" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "14px" }}>
            <div
              style={{
                background: "var(--bg-tertiary)",
                borderRadius: "8px",
                padding: "12px 14px",
                fontSize: "13px",
                color: "var(--text-secondary)",
                lineHeight: 1.6,
                display: "flex",
                gap: "10px",
                alignItems: "flex-start",
              }}
            >
              <ShieldCheck size={18} strokeWidth={1.75} style={{ color: "#4ade80", flexShrink: 0, marginTop: "2px" }} />
              <div>
                Đăng nhập qua <strong>Microsoft Edge WebView2</strong> (trình duyệt hệ thống Windows).
                Phương thức này mang fingerprint tự nhiên của máy, <strong>không bị Roblox gắn cờ tự động hoá</strong>,
                giải captcha Arkose và xác thực 2FA mượt mà không bị lặp.
              </div>
            </div>

            <div style={{ fontSize: "12px", color: "var(--text-muted)", lineHeight: 1.5 }}>
              * Sau khi đăng nhập thành công trong cửa sổ vừa mở, Muster sẽ tự động phát hiện cookie .ROBLOSECURITY, lưu tài khoản và đóng cửa sổ.
            </div>

            {browserBusy ? (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: "10px",
                  padding: "10px 14px",
                  background: "var(--bg-surface)",
                  borderRadius: "8px",
                  color: "var(--accent-text)",
                  fontSize: "13px",
                  border: "1px solid var(--accent)",
                }}
              >
                <Loader2 size={16} strokeWidth={1.75} style={{ animation: "spin 1s linear infinite" }} />
                <span>Cửa sổ đăng nhập Roblox đang mở. Hãy đăng nhập và giải Captcha (nếu có)...</span>
              </div>
            ) : (
              <button
                type="button"
                className="btn btn-primary"
                onClick={async () => {
                  const ok = await openBrowserLogin(null, null);
                  if (ok) handleClose();
                }}
                disabled={browserBusy}
                style={{ alignSelf: "flex-start", display: "flex", alignItems: "center", gap: "6px" }}
              >
                <Globe size={15} strokeWidth={1.75} />
                Mở trang đăng nhập Roblox (WebView2)
              </button>
            )}

            <div style={{ display: "flex", justifyContent: "flex-end" }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleClose}
                disabled={isBusy}
              >
                Đóng
              </button>
            </div>
          </div>
        )}

        {addTab === "tkpass" && (
          <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
            <div
              style={{
                background: "var(--bg-tertiary)",
                borderRadius: "8px",
                padding: "10px 14px",
                fontSize: "12px",
                color: "var(--text-secondary)",
                lineHeight: 1.6,
              }}
            >
              Nhập theo định dạng{" "}
              <code
                style={{
                  background: "var(--bg-surface)",
                  padding: "1px 5px",
                  borderRadius: "4px",
                  fontFamily: "monospace",
                  color: "var(--accent-text)",
                }}
              >
                username:password
              </code>
              , mỗi tài khoản một dòng. Có thể kéo-thả file .txt vào ô nhập.
            </div>

            <div className="modal-tabs" role="tablist">
              <button
                type="button"
                className={`modal-tab ${tkpassMode === "api" ? "active" : ""}`}
                onClick={() => setTkpassMode("api")}
                disabled={tkpassProcessing}
              >
                API (nhanh)
              </button>
              <button
                type="button"
                className={`modal-tab ${tkpassMode === "browser" ? "active" : ""}`}
                onClick={() => setTkpassMode("browser")}
                disabled={tkpassProcessing}
              >
                Trình duyệt (WebView2)
              </button>
            </div>

            <div style={{ fontSize: "11px", color: "var(--text-muted)", lineHeight: 1.6 }}>
              {tkpassMode === "api"
                ? "Đăng nhập trực tiếp qua auth.roblox.com — cực nhanh, không cần mở trình duyệt. Nếu gặp captcha/2FA, chuyển sang chế độ Trình duyệt."
                : "Mở cửa sổ WebView2 tự điền username/password lần lượt cho từng tài khoản. Giải captcha và 2FA an toàn không bị loop."}
            </div>

            {tkpassResults.length === 0 ? (
              <div className="input-group">
                <label className="input-label" htmlFor="modal-tkpass-input">
                  Danh sách tài khoản
                </label>
                <textarea
                  id="modal-tkpass-input"
                  className="input-field"
                  rows={6}
                  placeholder={"myuser1:password123\nmyuser2:pass456\nmyalt:securepass789"}
                  value={tkpassInput}
                  onChange={(e) => setTkpassInput(e.target.value)}
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => readDroppedFile(e, (text) => setTkpassInput(text.trim()))}
                  disabled={tkpassProcessing}
                  style={{ fontFamily: "monospace", fontSize: "12px" }}
                />
              </div>
            ) : (
              <ResultList rows={tkpassResults} />
            )}

            <div style={{ display: "flex", justifyContent: "flex-end", gap: "8px", marginTop: "4px" }}>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={handleClose}
                disabled={tkpassProcessing}
              >
                {tkpassProcessing ? "Đang xử lý..." : "Đóng"}
              </button>
              {tkpassResults.length === 0 && (
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={handleTkpassSubmit}
                  disabled={tkpassProcessing}
                >
                  {tkpassProcessing ? (
                    <>
                      <Loader2
                        size={13}
                        strokeWidth={1.75}
                        style={{ animation: "spin 1s linear infinite" }}
                      />
                      Đang đăng nhập...
                    </>
                  ) : (
                    "Bắt đầu đăng nhập"
                  )}
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
