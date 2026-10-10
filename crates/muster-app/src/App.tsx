import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Khop voi `VaultInfo` ben Rust (`src-tauri/src/lib.rs`). Buoc 0.5 se sinh tu dong bang tauri-specta. */
interface VaultInfo {
  path: string;
  size_bytes: number;
  branch: string;
}

export function App() {
  const [path, setPath] = useState("");
  const [result, setResult] = useState<VaultInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function inspect() {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      setResult(await invoke<VaultInfo>("vault_inspect", { path }));
    } catch (cause) {
      setError(typeof cause === "string" ? cause : String(cause));
    } finally {
      setBusy(false);
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
          <h2 className="card__title">Khung dự án đã dựng</h2>
          <p className="card__text">
            Đây là bộ xương của M0. Chưa có tính năng nào — phần này sẽ được thay bằng giao diện
            thật ở M1.
          </p>
          <ol className="steps">
            <li>
              <code>muster-core</code> — logic thuần, không phụ thuộc Tauri
            </li>
            <li>
              <code>muster-api</code> — local HTTP API (22 route, M2)
            </li>
            <li>
              <code>muster-cli</code> — chạy headless, không cần GUI
            </li>
            <li>
              <code>muster-app</code> — lớp vỏ Tauri 2, chỉ chứa command wrapper
            </li>
          </ol>
        </section>

        <section className="card">
          <h2 className="card__title">Kiểm tra đường IPC</h2>
          <p className="card__text">
            Gọi command <code>vault_inspect</code> bên Rust để chứng minh Rust → React thông suốt.
          </p>
          <p>
            <input
              type="text"
              value={path}
              onChange={(event) => setPath(event.target.value)}
              placeholder="đường dẫn tới AccountData.json"
              style={{
                width: "100%",
                padding: "8px 12px",
                borderRadius: 8,
                border: "1px solid var(--border)",
                background: "var(--bg)",
                color: "var(--text)",
                font: "inherit",
                fontFamily: "ui-monospace, Consolas, monospace",
                fontSize: 12,
              }}
            />
          </p>
          <button type="button" onClick={inspect} disabled={busy || path.length === 0}>
            {busy ? "Đang kiểm tra…" : "Kiểm tra"}
          </button>
        </section>

        {result !== null && (
          <section className="card">
            <h2 className="card__title">Kết quả</h2>
            <p className="result">
              {`branch : ${result.branch}\nsize   : ${result.size_bytes} byte\npath   : ${result.path}`}
            </p>
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
