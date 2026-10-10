import { useState } from "react";

import { commands } from "./bindings";
import type { VaultInspection, VaultOpenSummary } from "./bindings";

/**
 * Khung giao diện của M0.
 *
 * Đây chưa phải giao diện thật — nó tồn tại để chứng minh đường IPC có kiểu thông suốt từ Rust
 * sang React. Giao diện thật thuộc nhánh UI (ROADMAP mục 6.0-UI).
 *
 * Toàn bộ type dữ liệu ở đây đến từ `bindings.ts`, file do `tauri-specta` sinh ra từ code Rust.
 * **Không** khai lại type bằng tay: mục 6.0-UI nói rõ UI không được tự định nghĩa hình dạng dữ
 * liệu, vì chỉ cần backend trả về hình dạng khác là toàn bộ phần ghép phải viết lại.
 *
 * Sinh lại bindings: `npm run codegen`.
 */

type Busy = "idle" | "inspect" | "open";

export function App() {
  const [path, setPath] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState<Busy>("idle");
  const [inspection, setInspection] = useState<VaultInspection | null>(null);
  const [summary, setSummary] = useState<VaultOpenSummary | null>(null);
  const [error, setError] = useState<string | null>(null);

  const disabled = busy !== "idle" || path.length === 0;

  function reset() {
    setError(null);
    setInspection(null);
    setSummary(null);
  }

  async function inspect() {
    setBusy("inspect");
    reset();
    try {
      const result = await commands.vaultInspect(path);
      if (result.status === "ok") {
        setInspection(result.data);
      } else {
        setError(result.error);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy("idle");
    }
  }

  async function open() {
    setBusy("open");
    reset();
    try {
      const result = await commands.vaultOpen(path, password.length === 0 ? null : password);
      if (result.status === "ok") {
        setSummary(result.data);
      } else {
        setError(result.error);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy("idle");
    }
  }

  return (
    <div className="app">
      <header className="app__header">
        <h1 className="app__title">Muster</h1>
        <span className="app__subtitle">account manager for Roblox</span>
      </header>

      <main className="app__main">
        <section className="card">
          <h2 className="card__title">M0 — đọc file của Roblox Account Manager</h2>
          <p className="card__text">
            M0 đã xong: <code>core::vault</code> đọc được cả bốn nhánh định dạng,{" "}
            <code>core::settings</code> chuyển được <code>RAMSettings.ini</code> sang TOML. Phần
            dưới chỉ để chứng minh đường IPC có kiểu đã thông.
          </p>
        </section>

        <section className="card">
          <h2 className="card__title">Kiểm tra một file AccountData.json</h2>

          <label className="field">
            <span className="field__label">Đường dẫn file</span>
            <input
              type="text"
              value={path}
              onChange={(event) => setPath(event.target.value)}
              placeholder="đường dẫn tới AccountData.json"
            />
          </label>

          <label className="field">
            <span className="field__label">
              Mật khẩu{" "}
              <span className="field__hint">— chỉ cần nếu file ở nhánh password-locked</span>
            </span>
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="để trống nếu file không khoá bằng mật khẩu"
            />
          </label>

          <div className="actions">
            <button type="button" onClick={inspect} disabled={disabled}>
              {busy === "inspect" ? "Đang kiểm tra…" : "Đoán nhánh"}
            </button>
            <button type="button" className="button--ghost" onClick={open} disabled={disabled}>
              {busy === "open" ? "Đang mở…" : "Mở và đọc"}
            </button>
          </div>
        </section>

        {inspection !== null && (
          <section className="card">
            <h2 className="card__title">Nhánh định dạng</h2>
            <dl className="facts">
              <dt>nhánh</dt>
              <dd>{inspection.branch}</dd>
              <dt>kích thước</dt>
              <dd>{inspection.size_bytes} byte</dd>
            </dl>
          </section>
        )}

        {summary !== null && (
          <section className="card">
            <h2 className="card__title">Đã mở được</h2>
            <dl className="facts">
              <dt>nhánh đã dùng</dt>
              <dd>{summary.format}</dd>
              <dt>số tài khoản</dt>
              <dd>{summary.account_count}</dd>
            </dl>
            {summary.usernames.length > 0 && (
              <ol className="steps">
                {summary.usernames.map((name) => (
                  <li key={name}>{name}</li>
                ))}
              </ol>
            )}
          </section>
        )}

        {error !== null && (
          <section className="card">
            <h2 className="card__title">Lỗi</h2>
            <p className="result result--error">{error}</p>
          </section>
        )}
      </main>
    </div>
  );
}
