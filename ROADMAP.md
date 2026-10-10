# ROADMAP — `<PROJECT>`

> Ứng dụng quản lý nhiều tài khoản Roblox và mở game, kế thừa Roblox Account Manager (RAM).
>
> **Trạng thái:** Đang ở **M0 — Nền móng**, bước **0.4**. Workspace 4 crate đã dựng, build xanh.
> Nguồn tham chiếu để port là `RAM_Source/` (C# WinForms, .NET 4.7.2, ~16.000 dòng, license GPL-3.0).
>
> **Tên dự án:** CHƯA CHỐT. Toàn bộ tài liệu này dùng placeholder — xem mục 1.

---

## 1. Quy ước placeholder

Tên dự án chưa được chốt. Khi chốt, thay các placeholder sau bằng giá trị thật (tìm & thay toàn repo):

| Placeholder | Ý nghĩa | Quy tắc | Ví dụ nếu chọn "Muster" |
| :--- | :--- | :--- | :--- |
| `<PROJECT>` | Tên hiển thị, dùng trong UI, tiêu đề cửa sổ, tài liệu | PascalCase, không chứa chữ "Roblox" | `Muster` |
| `<project>` | Tên crate / package | kebab-case, chữ thường | `muster` |
| `<slug>` | Tên repo, domain, bundle identifier | lowercase, không dấu | `muster` |
| `<BINARY>` | Tên file thực thi | PascalCase | `Muster.exe` |

**Quy tắc đặt tên bắt buộc:**

- KHÔNG đưa "Roblox" vào tên sản phẩm, tên crate, bundle id, hay domain. Chỉ dùng dạng phụ đề mô tả:
  `<PROJECT> — account manager for Roblox`.
- Bundle identifier Tauri: `dev.<slug>.app` (không dùng `com.roblox.*`).
- Tên crate gợi ý: `<project>-core`, `<project>-cli`, `<project>-app`, `<project>-api`.

---

## 2. Nguyên tắc bất biến (invariants)

Những điều sau KHÔNG được vi phạm trong bất kỳ milestone nào. Nếu một tính năng đòi phá vỡ chúng → từ chối tính năng đó.

1. **Out-of-process tuyệt đối.** Không inject DLL, không patch memory, không hook API của
   `RobloxPlayerBeta.exe`. Chỉ được: spawn process, đọc file log, đọc registry, đọc command line,
   tạo named mutex, đọc/ghi `ClientSettings`. Lý do: Byfron/Hyperion và rủi ro khoá tài khoản.
2. **Tương thích ngược là ưu tiên cao hơn tính đẹp của code.** Người dùng RAM phải chuyển sang được
   mà không mất dữ liệu và không phải cấu hình lại. Xem mục 4 — Hợp đồng tương thích ngược.
3. **Logic không nằm trong `src-tauri/`.** Mọi thứ phải dùng được từ `<project>-cli` và test được
   bằng `cargo test` mà không cần khởi động Tauri.
4. **Secret không bao giờ chạm đĩa dạng plaintext.** Cookie, password, auth ticket phải nằm trong
   vùng nhớ được zeroize sau khi dùng. Log không bao giờ ghi cookie/ticket/password.
5. **Không tự cập nhật từ URL thô.** Mọi bản cập nhật phải qua chữ ký số.
6. **Giữ GPL-3.0.** Toàn bộ repo là GPL-3.0 (xem mục 9).
7. **Mọi tính năng nguy hiểm phải bị gate.** Hành vi có thể lộ quyền tài khoản (tạo link
   `rbx-player`, xuất cookie, `Execute` script trên client) mặc định TẮT, bật qua Developer Mode
   kèm cảnh báo rõ ràng — giữ đúng tinh thần của RAM.
8. **Local API mặc định chỉ nghe `localhost`.** Không bind `0.0.0.0` trừ khi người dùng bật rõ ràng
   `AllowExternalConnections`.

---

## 3. Kiến trúc & cấu trúc repo

### 3.1 Sơ đồ tầng

```
┌─ WebView2 ─────────────────────────────────────────────┐
│  React + TypeScript + Vite                             │
│  Accounts · Server browser · Settings · Nexus panel    │
└──────────────────────┬─────────────────────────────────┘
                       │  IPC có kiểu (tauri-specta sinh type TS)
┌──────────────────────┴─────────────────────────────────┐
│  <project>-app  (binary Tauri 2, chỉ là lớp vỏ)        │
└──────────────────────┬─────────────────────────────────┘
                       │
┌──────────────────────┴─────────────────────────────────┐
│  <project>-core  (thư viện thuần Rust, không Tauri)    │
│  vault · roblox::* · launcher · watcher · patch        │
└───┬────────────────┬─────────────────┬─────────────────┘
    │                │                 │
┌───┴──────────┐ ┌───┴──────────┐ ┌────┴───────────────┐
│ <project>-api│ │ <project>-cli│ │ crate `windows`    │
│ axum + WS    │ │ headless     │ │ Win32 · registry   │
└──────────────┘ └──────────────┘ └────────────────────┘
```

### 3.2 Cây thư mục mục tiêu

```
<slug>/
├─ Cargo.toml                  # workspace root
├─ ROADMAP.md                  # tài liệu này
├─ LICENSE                     # GPL-3.0
├─ crates/
│  ├─ <project>-core/          # logic thuần, phần lớn giá trị nằm ở đây
│  │  └─ src/
│  │     ├─ vault/             # mã hoá, migration AccountData.json
│  │     ├─ roblox/            # auth, account, games, servers, presence, batch, models
│  │     ├─ launch/            # ticket → join script → spawn
│  │     ├─ watcher/           # quét process, tail log, regex
│  │     ├─ patch/             # ClientSettings / FPS unlocker
│  │     ├─ nexus/             # protocol WS điều khiển client
│  │     └─ settings/          # config + migration RAMSettings.ini
│  ├─ <project>-api/           # HTTP API + WS server
│  ├─ <project>-cli/           # binary headless
│  └─ <project>-app/           # binary Tauri 2
│     ├─ src-tauri/            # chỉ chứa command wrapper + setup
│     └─ src/                  # frontend React
├─ assets/
│  ├─ lua/                     # Nexus.lua, RAMAccount.lua (port nguyên)
│  └─ icons/
└─ docs/
   ├─ COMPATIBILITY.md         # đặc tả định dạng file & API
   ├─ API.md                   # tài liệu local HTTP API
   ├─ BROWSER.md               # chiến lược đăng nhập: API trước, browser sau (ĐÃ CHỐT)
   └─ NEXUS.md                 # protocol Nexus
```

### 3.3 Tech stack

**Rust (backend)**

| Nhóm | Crate |
| :--- | :--- |
| Async runtime | `tokio` |
| HTTP client | `reqwest` (+ `rustls`), `serde`, `serde_json` |
| HTTP server | `axum`, `tower-http` |
| WebSocket | `tokio-tungstenite` |
| Crypto vault | `argon2` hoặc `libsodium-sys`, `crypto_secretbox`, `zeroize`, `secrecy` |
| Windows | `windows`, `windows-registry` (hoặc `winreg`) |
| Process | `sysinfo`, `windows` (command line, window rect) |
| File watch | `notify` |
| Regex | `regex`, `once_cell` |
| Rate limit | `governor` |
| Config | `serde`, `toml` |
| Logging | `tracing`, `tracing-subscriber`, `tracing-appender` |
| Error | `thiserror`, `anyhow` |
| Secret store | `keyring` |
| Browser automation (tuỳ chọn) | `chromiumoxide` |
| Tauri | `tauri` 2, `tauri-plugin-updater`, `tauri-plugin-store`, `tauri-plugin-single-instance`, `tauri-plugin-log`, `tauri-plugin-shell`, `tauri-specta` |

**Frontend**

| Nhóm | Package |
| :--- | :--- |
| Core | `react` 19, `typescript`, `vite` |
| State / data | `@tanstack/react-query`, `zustand` |
| UI | `tailwindcss`, `shadcn/ui`, `lucide-react` |
| Danh sách lớn | `@tanstack/react-virtual`, `@tanstack/react-table` |
| Drag & drop | `dnd-kit` |
| i18n | `i18next`, `react-i18next` |
| Chart | `recharts` |

---

## 4. Hợp đồng tương thích ngược

Đây là phần đặc tả chi tiết nhất và quan trọng nhất của tài liệu. Mọi con số ở đây đã được
kiểm chứng trực tiếp từ `RAM_Source/`.

### 4.1 Định dạng `AccountData.json` cần đọc được

RAM có **4 nhánh định dạng**. Trình nhập của `<PROJECT>` phải nhận diện và xử lý cả 4:

| # | Nhánh | Nhận diện | Cách giải mã |
| :--- | :--- | :--- | :--- |
| 1 | **Password-locked** | 64 byte đầu khớp magic `RAMHeader` | `Argon2(SHA512(password), salt, 32 byte)` → `secretbox_open` |
| 2 | **DPAPI — mặc định** | Không khớp magic, giải mã DPAPI bằng entropy đúng thì ra UTF-8 JSON | `CryptUnprotectData` scope **CurrentUser**, entropy 56 byte (mục 4.2) |
| 3 | **DPAPI — LocalMachine** | Như trên nhưng scope khác | `CryptUnprotectData` scope **LocalMachine**, cùng entropy |
| 4 | **Plain JSON** | Byte đầu là `[` | Đọc trực tiếp UTF-8 |

> ⚠️ **ĐÃ ĐO 2026-10-10 — nhánh 2 và 3 KHÔNG tách được khi đọc.** Chiến lược "thử CurrentUser
> trước, thất bại thì thử LocalMachine" vẫn đúng và vẫn phải làm, vì nó tối đa hoá khả năng mở
> được file. Nhưng **kết quả thử không cho biết phạm vi thật**: đo trên Windows cho thấy một blob
> được mã hoá bằng phạm vi **LocalMachine** vẫn giải mã thành công bằng đường **CurrentUser**
> (không kèm cờ `CRYPTPROTECT_LOCAL_MACHINE`). Cả hai fixture DPAPI trong `tests/fixtures/` đều
> mở được bằng cả hai chế độ cờ.
>
> Hệ quả cho code: `core::vault::Format` chỉ có **một** biến thể `Dpapi` thay vì hai, và báo cáo
> nhập ghi `dpapi`. Thà báo ít thông tin còn hơn khẳng định một phạm vi mà mình không biết chắc.
> Việc **ghi** thì vẫn chọn được phạm vi — `vault::seal_dpapi(plaintext, scope)`.
>
> Trường hợp ngoại lệ duy nhất đáng ghi nhận: nếu đường không cờ thất bại mà đường có cờ thành
> công thì chắc chắn là blob phạm vi máy — nhưng trường hợp đó hiếm và không đủ để đổi hình dạng
> của kiểu công khai.

**Chi tiết nhánh 1 — layout nhị phân:**

```
offset 0    : 64 byte   magic RAMHeader (ASCII, xem 4.2)
offset 64   : 16 byte   salt        (ArgonGenerateSalt)
offset 80   : 24 byte   nonce       (SecretBox.GenerateNonce)
offset 104  : 16 byte   MAC         (crypto_secretbox_MACBYTES)
offset 120  : phần còn lại  ciphertext
```

> ⚠️ **ĐÃ SỬA 2026-10-10 — bản đầu của tài liệu ghi sai vị trí MAC.** Bản đầu viết
> "offset 104 : ciphertext (secretbox, gồm 16 byte MAC ở cuối)". **Sai.** `Sodium.SecretBox.Encrypt`
> gọi `crypto_secretbox_easy`, và hàm này đặt 16 byte MAC ở **đầu** phần sau nonce, không phải ở
> cuối. Đã kiểm chứng lại trên chính fixture `accountdata.password-locked.bin`: mở được khi coi
> `blob[..16]` là MAC, và không mở được khi coi `blob[-16..]` là MAC. Kích thước tối thiểu của
> một file nhánh 1 vì vậy là **120 byte**, không phải 104.

- Hàm dẫn xuất khoá: `PasswordHash.ArgonHashBinary(SHA512(password), salt, StrengthArgon.Moderate, 32)`
- ⚠️ **Lưu ý quan trọng:** đầu vào của Argon2 KHÔNG phải password thô mà là `CryptoHash.Hash(password)`
  — trong libsodium, `crypto_hash` là **SHA-512**, tức 64 byte. Bỏ qua bước này sẽ giải mã sai.
- ✅ **ĐÃ XÁC MINH TOÀN BỘ tham số Argon2 (2026-10-10) — rủi ro #6 ĐÓNG HẲN.**
  Chuỗi bằng chứng:
  1. `packages.config` ghim **`libsodium-net 0.10.0`**; `Resources/libsodium.dll` (488.888 byte)
     là **libsodium 1.0.11** và **trùng kích thước từng byte** với `output/libsodium.dll` trong gói
     `libsodium-net.0.10.0.nupkg` → RAM dùng đúng native lib đó.
  2. DLL chỉ export `crypto_pwhash_alg_argon2i13` / `crypto_pwhash_argon2i*`; không có symbol
     `argon2id` nào (argon2id chỉ có từ libsodium 1.0.15).
  3. Dịch ngược IL của `Sodium.PasswordHash` trong `lib/Net40/Sodium.dll` (gói nupkg): overload nhận
     `StrengthArgon` là một `switch`, nạp cặp `(opslimit, memlimit)` rồi gọi hàm lõi, hàm lõi gọi
     native `crypto_pwhash(..., alg = 1)` — **`alg = 1` chính là `ARGON2I13`**. Không có phép biến
     đổi tham số nào ở giữa.
  4. Chạy lại đúng lời gọi đó bằng native libsodium 1.0.11 (ctypes), rồi dò ngược bộ tham số tương
     đương của implementation tham chiếu → khớp tuyệt đối.
  - ⇒ **Bản Rust PHẢI dùng `Algorithm::Argon2i` + `Version::V0x13`.** Dùng Argon2id sẽ giải mã sai 100%.
  - Bảng tham số đã chốt (`opslimit` → `t_cost` là **1:1**; `memlimit / 1024` → `m_cost`):

    | `StrengthArgon` | `opslimit` | `memlimit` (byte) | **`t_cost`** | **`m_cost`** (KiB) | `p_cost` |
    | :--- | ---: | ---: | ---: | ---: | ---: |
    | `Interactive` | 4 | 33.554.432 (32 MiB) | 4 | 32768 | 1 |
    | **`Moderate` ← RAM dùng** | **6** | **134.217.728 (128 MiB)** | **6** | **131072** | **1** |
    | `Sensitive` | 8 | 536.870.912 (512 MiB) | 8 | 524288 | 1 |

  - Độ dài khoá ra = **32 byte**; salt = **16 byte**; nonce = **24 byte**.
  - ⚠️ Sửa lỗi so với bản đầu của tài liệu: `opslimit` **KHÔNG** phải 8388608 (đó là hằng số của
    *Scrypt*, không liên quan). Giá trị thật là **6**.
  - Tương đương Rust:
    ```rust
    // Params::new(m_cost KiB, t_cost, p_cost, output_len)
    let params = argon2::Params::new(131_072, 6, 1, Some(32))?;
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2i, argon2::Version::V0x13, params);
    let mut key = [0u8; 32];
    a.hash_password_into(&sha512(password), &salt16, &mut key)?;  // đầu vào là SHA-512(password)
    ```
- 📌 Bằng chứng phụ trợ cho tính bất biến: `RAMHeader` thực tế là 64 byte ASCII
  `"Roblox Account Manager created by ic3w0lf22 @ github.com ......."` (đúng như mô tả ở bảng nhánh 1).
  `Cryptography.Decrypt` đọc salt **16 byte** rồi nonce **24 byte** — khớp layout ở trên.

**Chi tiết nhánh 2/3 — entropy DPAPI:**

Chuỗi entropy dài **56 byte**, ASCII, giữ nguyên cả lỗi chính tả gốc:

```
ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf
```

(đúng, "BUY" chứ không phải "BY" — phải khớp từng byte, không được sửa)

- Nhánh 2 và 3 dùng **cùng entropy**, khác `DataProtectionScope`. Trình nhập nên thử CurrentUser
  trước, thất bại thì thử LocalMachine.
- Nhánh 1 còn có biến thể phụ: `PasswordHash` (khoá đã dẫn xuất) được RAM cache lại bằng
  `ProtectedData.Protect(hash, entropy_rỗng, CurrentUser)` — không cần đọc, chỉ cần biết để không nhầm.

**File phụ cần xử lý:**

| File | Vai trò |
| :--- | :--- |
| `AccountData.json` | File chính |
| `AccountData.json.backup` | Bản sao, RAM ghi tối đa mỗi 8 giờ. Dùng để khôi phục |
| `AccountData.json.old` | Bản cũ khi RAM đổi file |
| `AccountData.json.bak` | RAM tạo khi giải mã thất bại hoàn toàn |
| `RecentGames.json` | Danh sách game gần đây, JSON riêng |
| `RAMSettings.ini` | Toàn bộ cấu hình, INI |
| `NoEncryption.IUnderstandTheRisks.iautamor` | File cờ. **Đã kiểm chứng lại tại `AccountManager.cs:377-380`:** cờ tồn tại → ghi **plain UTF-8 JSON**; cờ không tồn tại → ghi **DPAPI `LocalMachine`** + entropy. Chiều đọc (`AccountManager.cs:294-298`) chỉ thử **CurrentUser** rồi fallback plain JSON — **không thử LocalMachine**. Trình nhập phải thử lần lượt CurrentUser → LocalMachine → plain JSON để không bỏ sót file do chính RAM ghi ra |
| `handle.bin` | Bản sao `handle.exe`, `<PROJECT>` sẽ không dùng nữa |

**Yêu cầu bắt buộc với trình nhập:**

- [ ] Nhập **không phá huỷ**: chỉ đọc, không ghi đè file gốc của RAM.
- [ ] Nếu người dùng chọn chuyển hẳn: tạo bản sao lưu `<project>-backup-<timestamp>.zip` chứa toàn bộ
      file trên TRƯỚC khi ghi bất cứ thứ gì.
- [ ] Hiển thị báo cáo nhập: số account đọc được, số bị lỗi, nhánh định dạng đã dùng.
- [ ] Giữ nguyên `Account.Fields` (dictionary tuỳ ý) — đây là nơi RAM lưu vị trí cửa sổ
      (`Window_Position_X/Y`, `Window_Width/Height`) và các field người dùng tự thêm.
- [ ] Giữ nguyên `Group`, `Alias`, `Description`, `Password`, `LastUse`, `BrowserTrackerID`, `UserID`.
- [ ] Có test vector: 1 file cho mỗi nhánh, nằm trong `crates/<project>-core/tests/fixtures/`
      (đã khử định danh, không chứa cookie thật).

### 4.2 Local HTTP API — 22 route phải giữ nguyên

Mặc định cổng **7963** (setting `WebServerPort`). Bản thân RAM bind `http://localhost:<port>/`,
và bind `http://*:<port>/` khi bật `AllowExternalConnections`.

Quy ước chung cần tái lập:

- `GET /Running` → trả `true` dạng text thô. `GET /favicon.ico` → trả rỗng.
- Nhánh `/v2/<Method>` trả JSON `{"Success": bool, "Message": string}`.
  Nhánh `/v1/<Method>` trả text thô, mã lỗi ghi vào header `ws-error`.
- HTTP status: thành công 200, lỗi 400, sai quyền 401.
- Tham số qua query string: `Account`, `Password`, `Group`, `IncludeCookies`, `Cookie`.
- Password gate: `EveryRequestRequiresPassword`, và nhóm lệnh nhạy cảm
  (`GetCookie`, `GetAccounts`, `LaunchAccount`, `FollowUser`) luôn yêu cầu password nếu
  `WSPassword` được đặt. Password phải dài ≥ 6 ký tự.
- `Account` nhận cả username lẫn UserID.
- Các cờ cho phép riêng: `AllowGetAccounts`, `AllowGetCookie`, `AllowLaunchAccount`.

Danh sách route (đúng 22, giữ nguyên tên và chữ hoa/thường):

| # | Route | Nhóm | Ghi chú |
| :--- | :--- | :--- | :--- |
| 1 | `GetAccounts` | đọc | Danh sách username, phân cách bằng dấu phẩy, lọc theo `Group` |
| 2 | `GetAccountsJson` | đọc | Mảng object; `IncludeCookies=true` cần password |
| 3 | `GetCookie` | đọc | Trả `.ROBLOSECURITY` |
| 4 | `GetCSRFToken` | đọc | |
| 5 | `GetAlias` | đọc | |
| 6 | `GetDescription` | đọc | |
| 7 | `GetField` | đọc | |
| 8 | `GetBlockedList` | đọc | |
| 9 | `SetAlias` | ghi | |
| 10 | `SetDescription` | ghi | |
| 11 | `AppendDescription` | ghi | Nối thêm thay vì ghi đè |
| 12 | `SetField` | ghi | |
| 13 | `RemoveField` | ghi | |
| 14 | `SetAvatar` | ghi | Nhận JSON avatar |
| 15 | `SetServer` | ghi | `v1/join-game-instance` |
| 16 | `SetRecommendedServer` | ghi | Chọn server ít người |
| 17 | `LaunchAccount` | launch | |
| 18 | `FollowUser` | launch | |
| 19 | `ImportCookie` | import | |
| 20 | `BlockUser` | quyền | Cần PIN |
| 21 | `UnblockUser` | quyền | Cần PIN |
| 22 | `UnblockEveryone` | quyền | Cần PIN, chạy nền, có delay 20s giữa các lần |

Ngoài 22 route: giữ thêm `/Running` và `/favicon.ico`.

- [ ] Viết bộ test tích hợp replay đúng request/response của RAM cho cả 22 route, ở cả 2 nhánh `/v1/` và `/v2/`.
- [ ] Xuất bản `docs/API.md` + một OpenAPI spec để hệ sinh thái tool bên thứ ba chuyển sang dễ dàng.

### 4.3 Protocol Nexus (WebSocket)

- Cổng mặc định **5242** (setting `NexusPort`), host `localhost`.
- Client Roblox kết nối kèm query string: `name` (username), `id` (userId), `jobId`. Thiếu `name` hoặc
  `id` → server đóng kết nối ngay. Username không khớp account nào → đóng kết nối.
- **`assets/lua/Nexus.lua` (419 dòng) giữ nguyên hoàn toàn**, chỉ đổi host/port mặc định.
- API phía Lua phải giữ đủ: `Nexus.Connected`, `Nexus.Disconnected`, `Nexus.MessageReceived`,
  `Nexus:Connect/Stop/Send/Log/Echo`, `Nexus:Create[Button|TextBox|Label|Numeric]`, `Nexus:NewLine`,
  `Nexus:WaitForMessage`, `Nexus:GetText`, `Nexus:SetRelaunch`, `Nexus:SetAutoRelaunch`,
  `Nexus:SetPlaceId`, `Nexus:SetJobId`, `Nexus:AddCommand`, `Nexus:RemoveCommand`,
  `Nexus:OnButtonClick`, `Nexus.ShutdownTime`.
- Lệnh mặc định phải giữ: `Execute`, `Teleport`, `Rejoin`, `Mute`, `Unmute`, `Performance`.
- Tài liệu: port `NexusDocs.md` sang `docs/NEXUS.md`.
- [ ] Test: một client giả lập WS gửi/nhận đúng chuỗi lệnh như Nexus.lua.

### 4.4 Định dạng cấu hình `RAMSettings.ini`

- [x] ~~Viết migration đọc INI cũ → config mới (`toml`), giữ nguyên giá trị, không hỏi lại người dùng.~~
      **XONG 2026-10-10** — `core::settings`, xem ghi chú bên dưới.
- [x] ~~Giữ nguyên tên khoá để người dùng có thể tự sửa file nếu muốn~~ **XONG 2026-10-10**.
      Danh sách dưới đây là 16 khoá mà bản đầu của tài liệu liệt kê:
      `WebServerPort`, `NexusPort`, `EnableMultiRbx`, `SavePasswords`, `AllowExternalConnections`,
      `EveryRequestRequiresPassword`, `AllowGetAccounts`, `AllowGetCookie`, `AllowLaunchAccount`,
      `ShuffleJobID`, `AutoCloseLastProcess`, `UnlockFPS`, `MaxFPSValue`, `CustomClientSettings`,
      `ScaleFonts`, `DevMode`.
      ⚠️ **Hai điểm đã sửa khi đối chiếu `RAM_Source/`:**
      1. **Khoá thật là `ShuffleJobId`, không phải `ShuffleJobID`.** `AccountManager.cs:2079` ghi ra
         chữ `d` thường. Bản port ghi ra `ShuffleJobId` và khi đọc chấp nhận cả hai cách viết.
      2. **Danh sách thật nhiều hơn 16 khoá.** `AccountManager.cs:121-165` đặt mặc định cho khoảng
         40 khoá trải trên 6 section (`General`, `Developer`, `WebServer`, `AccountControl`,
         `Watcher`, `Prompts`). Migration đọc hết, và mọi khoá/section không mô hình hoá được vẫn
         giữ nguyên trong catch-all nên không mất dữ liệu.
- [x] ~~File `RAMSettings.ini` gốc phải được sao lưu, không xoá.~~ **XONG 2026-10-10** —
      `settings::backup_file()` ghi bản sao `<tên>.backup-<unix_giây>` trước khi ghi bất cứ thứ gì.

---

## 5. Bảng tái sử dụng `RAM_Source`

| File trong RAM_Source | Nội dung | Đích trong `<PROJECT>` | Trạng thái |
| :--- | :--- | :--- | :--- |
| `Classes/Account.cs` | CSRF, auth ticket, join, block, avatar, quick login, fields | `core::roblox::account` | Port 1:1 |
| `Classes/Cryptography.cs` | Argon2 + secretbox, layout file | `core::vault` | Port 1:1 |
| `Classes/RobloxProcess.cs` | Tail log, regex DataModel / disconnect | `core::watcher::log` | Port 1:1 |
| `Classes/RobloxWatcher.cs` | Quét process, khớp cờ `-b`, vị trí cửa sổ | `core::watcher` | Port 1:1 |
| `Classes/ClientSettingsPatcher.cs` | FPS unlocker qua registry | `core::patch` | Port 1:1 |
| `Classes/Utilities.cs` | `GetRandomJobId`, MD5, SHA256 | `core::roblox::servers` | Port 1:1 |
| `Classes/Presence.cs` | Presence API theo batch | `core::roblox::presence` | Port 1:1 |
| `Classes/Batch.cs` | Gộp thumbnail + place details trong 50ms | `core::roblox::batch` | Port 1:1 |
| `GameClass`, `GameInstance`, `GamePlayer`, `Server` | DTO | `core::models` | Port 1:1 |
| `Forms/ServerList.cs` | games.roblox.com, player finder, favorites | `core::roblox::games` | Port logic, UI mới |
| `AccountManager.SendResponse` | Local HTTP API 22 route | `core::api` | Giữ route y nguyên |
| `Nexus/WebsocketServer.cs`, `Command.cs` | WS protocol | `core::nexus` | Port 1:1 |
| `Nexus/Nexus.lua` | Script phía client | `assets/lua/` | Dùng nguyên |
| `RAMAccount.lua` | Script phụ trợ | `assets/lua/` | Dùng nguyên |
| `Resources/WatcherRegexMatches.txt` | Cơ chế cập nhật regex từ xa | giữ cơ chế, tự host + ký | Dùng nguyên |
| `Resources/*.png`, `*.ico` | Icon | `assets/icons/` | Dùng nguyên |
| `Classes/IniFile.cs` | Parser INI | `serde` + `toml` + migration | Viết lại |
| `Classes/AccountBrowser.cs` | CefSharp + Puppeteer + proxy + captcha | `core::browser` | **Viết lại theo `docs/BROWSER.md`** — API trước, browser sau; dùng trình duyệt có sẵn (Tầng 0) hoặc Chrome for Testing (Tầng 1), KHÔNG nhúng sẵn |
| `Resources/handle.exe` | Tìm log path | native handle / glob log | Thay thế |
| `Forms/Updater.cs` | Tự cập nhật | `tauri-plugin-updater` | Thay thế |
| `Classes/WM.cs` | Helper Win32 | crate `windows` | Port một phần |
| `Forms/*.cs` + `*.Designer.cs`, `ObjectListView`, `ThemeEditor`, `NBTabControl`, `BorderedTextBox`, `AvatarControl` | Toàn bộ UI WinForms | React + Tailwind + shadcn/ui | Bỏ |

**Kiến thức nghiệp vụ phải port chính xác (đã kiểm chứng trong source):**

- Chuỗi xác thực: `POST /v1/authentication-ticket/` bị trả **403** → đọc header `x-csrf-token`
  → POST lại kèm `X-CSRF-TOKEN` + `Referer` → đọc header `rbx-authentication-ticket`.
- 🚨 **RAM KHÔNG có đường đăng nhập bằng API.** Đã kiểm chứng: trong toàn bộ `RAM_Source` chỉ có
  **một** chỗ nhắc `auth.roblox.com/v2/login`, và đó là `AccountBrowser.Page_RequestFinished` —
  tức chỉ *quan sát* request do chính Chromium gửi đi. Không có dòng C# nào POST username/password.
  - Hệ quả 1: `user:pass` trong RAM **không phải** một phương thức đăng nhập — nó chỉ được gõ sẵn
    vào form của Chromium. `bulkUserPassToolStripMenuItem_Click` (`AccountManager.cs:1396-1417`)
    mở **một `AccountBrowser` cho mỗi dòng** `user:pass`, hai cửa sổ một lượt.
  - Hệ quả 2: khi Puppeteer không khả dụng, `AccountManager_Shown` **gỡ luôn**
    `bulkUserPassToolStripMenuItem` và `customURLJSToolStripMenuItem` khỏi menu. Tức tính năng
    nhập hàng loạt user:pass **phụ thuộc hoàn toàn** vào browser.
  - ⇒ `<PROJECT>` có cơ hội làm tốt hơn: tự gọi API login để không phải tải ~150 MB Chromium
    cho những tài khoản chỉ cần user:pass. Xem task mới ở M1.
- Body của request login (đọc từ `AccountBrowser.cs:309-312`): có khoá `ctype` với giá trị `Username`
  và khoá `password`. Tên khoá chứa username cần xác minh lại khi code (khả năng cao là `cvalue`).
  Captcha/2FA không thể xử lý qua API — gặp hai thứ đó buộc phải có browser.
- Multi-instance cần **cả hai**: named mutex `ROBLOX_singletonMutex` VÀ cờ `-b <BrowserTrackerID>`
  riêng cho từng account. `BrowserTrackerID` được sinh ngẫu nhiên và lưu cùng account.
- Watcher phải **bỏ qua tiến trình thứ hai** của Roblox (command line rỗng, hoặc bắt đầu bằng `\??\`,
  hoặc không chứa `-t `/`-j `). Kill nhầm → kick lỗi 268.
- Regex log phụ thuộc format của Roblox và sẽ hỏng khi Roblox đổi format → giữ cơ chế danh sách regex
  tải từ xa, nhưng tự host và ký số thay vì trỏ vào raw GitHub.
- Trước khi launch, RAM patch `ClientAppSettings.json`; đường dẫn version lấy từ registry
  `HKEY_CLASSES_ROOT\roblox\DefaultIcon` → thư mục `version-*`.
- `AutoCloseLastProcess`: đóng process cũ của cùng `BrowserTrackerID` bằng `CloseMainWindow()` **hai lần**
  (cách nhau 250ms) rồi mới `Kill()`, để Roblox kịp ngắt kết nối và tránh lỗi "Same account launched".
- RAM dùng token bucket cho rate limit. Roblox rate limit endpoint auth ticket — cần giữ cơ chế này.
- `SaveAccounts` có rate limit nội bộ (2 giây) và bỏ qua khi danh sách rỗng. Cần tái lập để tránh
  ghi đĩa liên tục.

---

## 6. Lộ trình theo milestone

Kích cỡ là **tương đối** (S/M/L/XL), không phải ước lượng thời gian.

### 6.0 — Thứ tự thực thi M0 (chốt ngày 2026-10-10)

Thứ tự dưới đây **khác** thứ tự liệt kê ở M0 vì nó sắp theo *độ rủi ro giảm dần*, không theo độ tự nhiên
của code. Nguyên tắc: giết rủi ro chưa biết trước khi viết code dễ.

| Bước | Việc | Vì sao ở đúng vị trí này | Chặn bởi |
| :--- | :--- | :--- | :--- |
| ~~**0.1**~~ ✅ | ~~Sinh test vector vault thật (1 file mỗi nhánh, đã khử định danh)~~ **XONG 2026-10-10** | Là *hợp đồng* cho toàn bộ `core::vault`. Không có nó thì mọi dòng code vault chỉ là phỏng đoán | — |
| ~~**0.2**~~ ✅ | ~~Chốt ánh xạ `StrengthArgon.Moderate` → `(t, m, p)`~~ **XONG 2026-10-10** → `t=6, m=131072 KiB, p=1` | Rủi ro #6. Biết Argon2i rồi vẫn phải biết `t_cost` | 0.1 |
| ~~**0.3**~~ ✅ | ~~Scaffold workspace 4 crate + `tools/rename.sh`~~ **XONG 2026-10-10** | Sau 0.2 vì tên crate phụ thuộc tên dự án, và ta muốn rename được | 0.2 (mềm) |
| ~~**0.4**~~ ✅ | ~~`core::vault` + test vector chạy xanh~~ **XONG 2026-10-10** — 58 unit test + 11 test tích hợp, cả 4 fixture giải mã ra dữ liệu giống hệt nhau | Lúc này mới có cơ sở để viết code | 0.1, 0.2, 0.3 |
| ~~**0.5**~~ ✅ | ~~`tauri-specta` + command xuyên từ React~~ **XONG 2026-10-10** — bindings sinh tự động vào `crates/muster-app/src/bindings.ts`, `tsc --noEmit` xanh | Chứng minh đường IPC có kiểu thông suốt trước khi xây UI thật | 0.3 |
| ~~**0.6**~~ ✅ | ~~`core::settings` + migration `RAMSettings.ini` → TOML~~ **XONG 2026-10-10** — 6 section, ~40 khoá, giữ nguyên tên khoá của RAM | Độc lập, rủi ro thấp, làm khi rảnh | 0.3 |
| **0.7** | Wizard nhập liệu + backup ZIP + báo cáo | Cần 0.4 xong | 0.4 |
| **0.8** | `tracing` + redaction test + CI | Chốt chặn chất lượng, phải có trước khi có gì đáng giữ | 0.3 |

**Trạng thái 2026-10-10:** 0.1, 0.2, 0.3, **0.4**, **0.5**, **0.6** đã xong. Việc tiếp theo là
**0.7** (wizard nhập liệu) và **0.8** (tracing + CI).
Workspace build được cả 4 crate (`cargo check --workspace` xanh), `cargo test` xanh,
`tsc --noEmit` xanh, frontend build ra `dist/`. `tools/rename.sh` đã được kiểm chứng bằng một vòng
đổi tên thật trên bản sao (đổi sang tên khác rồi khôi phục, cả hai chiều đều xanh).

**Baseline git:** repo đã `git init` nhưng tới 2026-10-10 vẫn **chưa có commit nào**. Đã chốt
baseline ở lần làm việc này; từ đây mỗi bước xong phải có commit riêng.

**Hợp đồng hai nhánh đã có thật:** `crates/muster-app/src/bindings.ts` được sinh từ code Rust và
đã chứa `Account`, `Branch`, `Format`, cùng hai command `vaultInspect` / `vaultOpen`. Nhánh UI từ
đây **được phép** bắt đầu dựng component bằng dữ liệu giả dựa trên type đó — nhưng vẫn chưa được
ghép dữ liệu thật cho tới khi M1 có model `Account` đầy đủ.

---

### 6.0-UI — Nhánh UI/UX (chạy song song, KHÔNG nằm trên đường găng)

UI **không phải một bước tuần tự** trong lộ trình — nó là một nhánh riêng, chạy song song với
nhánh Rust. Đây là chỗ tài liệu từng thiếu: các task UI trước đây chỉ nằm rải rác trong M1–M3
nên không ai thấy được toàn cảnh.

| Việc | Bắt đầu được từ khi nào |
| :--- | :--- |
| Design system: token màu/chữ/khoảng cách, layout khung, dark/light | **Ngay** — không phụ thuộc gì |
| Wireframe các màn hình chính: Accounts, Server browser, Settings, Nexus | **Ngay** |
| Cài stack frontend (Tailwind, shadcn/ui, TanStack, dnd-kit, zustand, i18next) | **Ngay** |
| Component nội bộ dựng bằng **dữ liệu giả** (`AccountRow`, `AccountList`, `Toolbar`…) | **Ngay** |
| Ghép với **dữ liệu thật** | Chỉ sau khi có **type TS sinh từ 0.5** (`tauri-specta`) **và** model `Account` của M1 |

⚠️ **Điểm phụ thuộc cứng — đừng bỏ qua.** UI **không được tự định nghĩa type cho dữ liệu.**
Nếu UI dựng một `interface Account` theo trí tưởng tượng rồi backend trả về hình dạng khác,
toàn bộ phần ghép phải viết lại. Cách chặn: chốt trước hình dạng của `Account` (đặc biệt là
`Fields`, `Group`, `BrowserTrackerID`), `Server`, `GameInstance` — rồi để `tauri-specta` sinh
type cho UI dùng. Đây chính là lý do bước **0.5 có giá trị cao hơn vẻ ngoài của nó**: nó không
chỉ là "cho đẹp kiến trúc", nó là **hợp đồng giữa hai nhánh**.

**Trạng thái 2026-10-10 — hợp đồng đã có thật.** `crates/muster-app/src/bindings.ts` được sinh từ
code Rust và hiện chứa: `Account` (kèm `Account_Serialize` / `Account_Deserialize`), `Branch`,
`Format`, `VaultInspection`, `VaultOpenSummary`, và hai command `vaultInspect` / `vaultOpen`.
Sinh lại bằng `npm run codegen`.

Hệ quả cho nhánh UI:

- **Được phép** bắt đầu dựng component bằng dữ liệu giả dựa trên `Account` trong bindings — đây
  chính là thứ mà trước 0.5 chưa có nên nhánh UI bị chặn.
- **Chưa được** ghép dữ liệu thật cho tới khi M1 có CRUD account; hiện chỉ có hai command đọc file.
- Cần thêm field vào `Account` hoặc thêm command mới → báo nhánh Rust trước, rồi chạy
  `npm run codegen`. **Đừng sửa `bindings.ts` bằng tay** — file đó bị ghi đè mỗi lần chạy codegen.
- `Account::extra` (túi field lạ) cố tình **không** xuất sang TypeScript: UI không thể và không nên
  dùng dữ liệu chưa biết hình dạng. Nó vẫn được giữ nguyên khi ghi lại xuống đĩa.

**Bối cảnh đã chốt 2026-10-10:** UI dev làm **trong repo này**, tại `crates/muster-app/src/`,
dùng **stack theo mục 3.3**.

**Phân chia sở hữu:**

| Nhánh | Sở hữu | Không đụng vào |
| :--- | :--- | :--- |
| UI | `crates/muster-app/src/`, `index.html`, `vite.config.ts`, `tsconfig.json` | `crates/*/src/`, `src-tauri/` |
| Rust | `muster-core`, `muster-api`, `muster-cli`, `src-tauri` (command wrapper) | `crates/muster-app/src/` |

Thay đổi vượt ranh giới thì báo nhau trước. Cụ thể có đúng hai loại việc: **thêm field vào
`Account`** (Rust đổi, UI phải biết) và **thêm command Tauri mới** (Rust đổi, UI phải biết).

**Các task UI trong M1–M3** vẫn giữ nguyên chỗ cũ vì chúng gắn với tính năng tương ứng. Mục này
chỉ bổ sung phần *thiết kế và hạ tầng*, thứ mà M1–M3 không mô tả:

- [ ] Design system + token (S)
- [ ] Wireframe 4 màn hình chính (M)
- [ ] Cài và cấu hình stack frontend theo mục 3.3 (S)
- [ ] Layout khung app: sidebar + vùng nội dung + thanh trạng thái (M)
- [ ] Component nội bộ chạy bằng dữ liệu giả (L)
- [ ] Ghép dữ liệu thật — **chặn bởi 0.5 và M1** (L)

---

### M0 — Nền móng

> Mục tiêu: có khung repo chạy được, vault đọc được dữ liệu RAM cũ, IPC có kiểu hoạt động.

- [x] ~~Scaffold Cargo workspace 4 crate + frontend Vite/React/TS (M)~~ — **XONG 2026-10-10**:
      `cargo check --workspace` xanh, `cargo test` xanh, `tsc --noEmit` xanh, `tools/rename.sh` xong.
- [x] ~~Cấu hình `tauri-specta` sinh type TypeScript từ command Rust (M)~~
      — **XONG 2026-10-10**: `muster-app/src-tauri/src/lib.rs` dựng `Builder`, hai command
      `vault_inspect` / `vault_open`, bindings ghi ra `crates/muster-app/src/bindings.ts`.
      Sinh lại bằng `npm run codegen`. Xem rủi ro #17 về việc crate này không chạy được `cargo test`.
- [x] ~~`core::settings`: model config + đọc `RAMSettings.ini` cũ, migration sang TOML (M)~~
      — **XONG 2026-10-10**: `core::settings` + `core::settings::ini`, giữ nguyên tên section và
      tên khoá của RAM, catch-all nên không mất khoá lạ, `backup_file()` sao lưu file gốc.
- [x] ~~`core::vault`: 4 nhánh giải mã ở mục 4.1 + API ghi lại (L)~~ — **XONG 2026-10-10**:
      `derive_key` (SHA-512 → Argon2i), `open_password_locked`/`seal_password_locked`,
      `vault::dpapi`, `open()` tự dò nhánh, `read_accounts`/`accounts_to_bytes`.
      Kèm `core::models::Account` khớp từng field với `Classes/Account.cs`.
- [x] ~~Test vector cho cả 4 nhánh vault, kèm fixture đã khử định danh (M)~~ — **XONG 2026-10-10**:
      `crates/muster-core/tests/fixtures/` (4 file + `manifest.json`), sinh bằng
      `tools/fixtures-gen/generate.py`, cả 4 nhánh qua self-check giải mã lại.
- [ ] Wizard nhập dữ liệu từ RAM: chọn thư mục → phát hiện file → báo cáo → sao lưu → nhập (L)
- [ ] `tracing` + log file xoay vòng, redaction tự động cho cookie/ticket (S)
- [ ] CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `tsc --noEmit` (S)
- [ ] **Nhánh UI/UX** — design system, wireframe, cài stack frontend (xem mục 6.0-UI). Chạy song song,
      không chặn đường găng; riêng việc ghép dữ liệu thật thì bị chặn bởi 0.5 và M1.

**Nghiệm thu M0:** chạy `<project>-cli import --from <thư_mục_RAM>` trên một bản cài RAM thật, nhập
đúng số account, không sửa file gốc, và một test tự động chứng minh giải mã được file
password-locked.

---

### M1 — Parity cơ bản (đã dùng được thật)

> Mục tiêu: launch được nhiều account cùng lúc, thay thế được RAM cho nhu cầu cơ bản.

- [ ] `core::roblox::account`: model `Account` + CRUD + `Fields` (M)
- [ ] `core::roblox::auth`: CSRF → auth ticket, có retry và rate limit (M)
- [ ] ⭐ `core::roblox::auth`: **đăng nhập bằng API từ `user:pass`** → cookie `.ROBLOSECURITY`
      (`POST https://auth.roblox.com/v2/login`, body `{ctype:"Username", cvalue:<user>, password:<pass>}`,
      kèm header CSRF). **RAM KHÔNG có đường này** — đây là cải tiến, cho phép nhập hàng loạt
      user:pass mà không phải tải trình duyệt ~198 MiB. Xem `docs/BROWSER.md` mục 3 (M)
- [ ] ⭐ `core::roblox::auth`: **phân loại lỗi đăng nhập** — sai mật khẩu / captcha / 2FA /
      rate limit / tài khoản bị khoá. Đây là điều kiện tiên quyết để biết khi nào được phép
      đề nghị người dùng tải trình duyệt (S)
- [ ] `core::roblox::account`: `GetRobux`, `GetUserInfo`, `GetMobileInfo`, presence (M)
- [ ] `core::launch`: dựng launch URL `roblox-player:1+...` + `HttpUtility.UrlEncode` tương đương (L)
- [ ] `core::launch`: chế độ legacy (`RobloxPlayerBeta.exe -t ... -j ...`) — giữ để dự phòng (M)
- [ ] `core::launch`: multi-instance — mutex `ROBLOX_singletonMutex` + sinh `BrowserTrackerID` (M)
- [ ] `core::launch`: `AutoCloseLastProcess` đúng hành vi 2 lần `CloseMainWindow` (S)
- [ ] `core::patch`: phát hiện version folder qua registry, ghi `ClientAppSettings.json`, FPS unlocker (M)
- [ ] UI: danh sách account (virtual hoá), thêm/sửa/xoá, kéo thả sắp xếp (L)
- [ ] UI: ô PlaceId / JobId, nút Join, lưu PlaceId+JobId theo account (M)
- [ ] UI: chọn nhiều account → launch hàng loạt, có tiến trình và xử lý lỗi từng account (M)
- [ ] `core::launch`: join VIP server từ link đầy đủ, kể cả nhánh redirect (L)

**Nghiệm thu M1:** chọn 5 account, join cùng một PlaceId, cả 5 client chạy song song không bị kick
"Same account launched"; không cần `handle.exe`.

---

### M2 — Parity đầy đủ (ngang tầm RAM)

- [ ] `core::roblox::groups`: group, sắp xếp theo số tiền tố (`1Main`, `007 Bank`) (M)
- [ ] `core::roblox::games`: server list có phân trang cursor, sort, lọc server còn chỗ (L)
- [ ] `core::roblox::games`: player finder, universe viewer, outfit viewer (L)
- [ ] `core::roblox::servers`: `GetRandomJobId` (shuffle JobId), chọn server ít người (M)
- [ ] `core::roblox::batch`: gộp thumbnail + place details (M)
- [ ] `core::roblox::presence`: cập nhật trạng thái định kỳ, hiển thị trong game / online / offline (M)
- [ ] Favorites + Recent games (M)
- [ ] `core::roblox::account`: đổi password, đổi email, privacy follow, block/unblock, PIN unlock (L)
- [ ] `core::roblox::account`: đổi display name, set avatar, xử lý asset không hợp lệ (M)
- [ ] `core::roblox::account`: quick login (nhập code 6 ký tự) (M)
- [ ] Import hàng loạt: cookie (`.ROBLOSECURITY`), `user:pass`, kéo thả file (M)
- [ ] Auto refresh cookie định kỳ (M)
- [ ] `core::browser`: dò trình duyệt có sẵn (Edge/Chrome) và khởi chạy với profile riêng
      + CDP — **Tầng 0, không tải gì** (`docs/BROWSER.md` mục 2) (M)
- [ ] `core::browser::fetch`: tải Chrome for Testing từ `chrome-for-testing-public`,
      xác minh chữ ký Authenticode, cài đặt nguyên tử — **Tầng 1** (L)
- [ ] UI: nút "Tải trình duyệt" + popup tiến trình + cơ chế tạm dừng màn hình
      (luồng 7 bước ở `docs/BROWSER.md` mục 3) (M)
- [ ] `core::browser::login`: mở trang login không headless, chờ đăng nhập, bắt cookie qua
      `Network.getCookies`, dọn profile tạm (L)
- [ ] `core::watcher`: quét process, tail log, regex DataModel/disconnect, bỏ qua process thứ hai (L)
- [ ] `core::watcher`: cập nhật regex từ xa (tự host + ký) (M)
- [ ] `core::watcher`: auto relaunch khi mất kết nối / bị kick (M)
- [ ] `core::watcher`: nhớ vị trí cửa sổ theo account (S)
- [ ] `core::api`: HTTP server 22 route + 2 route phụ, đủ cờ cấu hình (L)
- [ ] `core::nexus`: WS server + port `Nexus.lua`, panel điều khiển account (L)
- [ ] `core::nexus`: các lệnh `Execute`, `Teleport`, `Rejoin`, `Mute`, `Unmute`, `Performance` (M)
- [ ] `core::nexus`: dựng UI động từ lệnh `Create*` của Lua (M)
- [ ] UI: theme (sáng/tối + tuỳ biến), i18n Việt/Anh (M)
- [ ] UI: màn hình Settings đầy đủ, Developer Mode gate (M)
- [ ] `docs/API.md` + `docs/NEXUS.md` + `docs/COMPATIBILITY.md` (M)

**Nghiệm thu M2:** một script bên thứ ba viết cho RAM chạy nguyên xi mà không sửa một dòng, khi chỉ
đổi cổng. `Nexus.lua` chạy không đổi. Danh sách 22 route có test tích hợp.

---

### M3 — Nâng cấp so với RAM

- [ ] Thay `handle.exe` bằng đọc handle native (`NtQuerySystemInformation`) hoặc dò log theo
      `%LOCALAPPDATA%\Roblox\logs` + so khớp thời điểm khởi động (L)
- [ ] `tauri-plugin-updater` + ký số; installer MSI/NSIS có chữ ký (L)
- [ ] Vault nâng cấp: master password + recovery key, export/import mã hoá (L)
- [ ] Vault: lưu master password trong OS keychain (`keyring`) (M)
- [ ] Scheduler: launch theo giờ, rejoin khi bị kick, refresh cookie theo lịch (L)
- [ ] Rate limiter dùng chung cho mọi request Roblox (`governor`) (M)
- [ ] UI: command palette, phím tắt, thông báo toast (M)
- [ ] Analytics cục bộ: playtime, robux, điểm "sức khoẻ" account, biểu đồ (M)
- [ ] Sức khoẻ account: thay chấm vàng/đỏ của RAM bằng chỉ số rõ nghĩa + gợi ý hành động (S)
- [ ] System tray: chạy nền, khởi động cùng Windows (M)
- [ ] Per-account proxy (M)
- [ ] Xuất/nhập cấu hình toàn bộ app dạng một file (S)
- [ ] Tính năng "duyệt web dưới danh nghĩa account" (mở trình duyệt với cookie của account đã chọn)
      — gate sau Developer Mode, không xuất hiện trong UI mặc định (M)
- [ ] Nút "Xoá trình duyệt đã tải" + hạn mức dung lượng trong Settings (S)
- [ ] Proxy theo từng trình duyệt/account (`docs/BROWSER.md` — RAM dùng `proxies.txt`) (M)

**Nghiệm thu M3:** cài đặt trên máy Windows sạch, Windows Defender không gắn cờ; cập nhật từ v1 lên v2
qua kênh có ký số thành công.

---

### M4 — Đa nền tảng & headless

> **Trạng thái: BACKLOG — không cam kết.** Quyết định 2026-10-10: sản phẩm chỉ nhắm Windows.
> Các mục macOS dưới đây giữ lại chỉ để ghi nhận, KHÔNG đưa vào kế hoạch thực thi.
> Phần headless (CLI/daemon/Docker) vẫn có giá trị và có thể tách ra làm sớm nếu cần.

- [ ] `core::watcher`: trừu tượng hoá theo OS — Windows (`%LOCALAPPDATA%\Roblox\logs`) và
      macOS (`~/Library/Logs/Roblox`) (L)
- [ ] `core::vault`: backend macOS dùng Keychain thay DPAPI (M)
- [ ] `core::launch`: macOS — mở qua `open` với URL scheme tương đương (L)
- [ ] `core::patch`: macOS — đường dẫn `ClientSettings` khác (M)
- [ ] `<project>-cli` hoàn chỉnh: `import`, `export`, `list`, `launch`, `serve`, `watch` (L)
- [ ] Chế độ daemon: chạy nền không GUI, dùng được trên VPS (M)
- [ ] Docker image cho chế độ headless (S)
- [ ] Đóng gói: MSI/NSIS (Windows), DMG (macOS) (M)

**Nghiệm thu M4:** trên macOS, launch được nhiều account và watcher đọc đúng log. CLI chạy được
trên Linux trong Docker cho các tác vụ không cần client.

---

### M5 — Hệ sinh thái

- [ ] Plugin system: WASM plugin cho script cộng đồng (XL)
- [ ] Thư viện plugin + tài liệu viết plugin (M)
- [ ] Remote control qua điện thoại: PWA gọi local API, có pairing + token (L)
- [ ] Đồng bộ vault E2E lên storage của chính người dùng (Drive / S3 / WebDAV) (L)
- [ ] SDK cho local API: TypeScript + Python (M)
- [ ] Discord bot / webhook thông báo trạng thái account (M)
- [ ] Trang web tài liệu + trang tải (M)

---

## 7. Rủi ro & đối sách

| # | Rủi ro | Mức | Đối sách |
| :--- | :--- | :--- | :--- |
| 1 | Byfron/Hyperion chặn hoặc khoá account vì multi-instance | Cao | Giữ nguyên cơ chế của RAM (mutex + `-b`). Không inject. Để multi-instance mặc định TẮT, kèm cảnh báo rõ. |
| 2 | Roblox đổi format log → regex hỏng, watcher chết | Cao | Giữ cơ chế tải regex từ xa; tự host + ký; có regex dự phòng đóng gói sẵn; log cảnh báo khi tỉ lệ khớp tụt. |
| 3 | Rate limit endpoint auth ticket | Cao | Token bucket dùng chung + hàng đợi launch tuần tự + backoff. Không launch song song không giới hạn. |
| 4 | Antivirus gắn cờ (RAM từng bị) | Cao | Bỏ `handle.exe`; updater có ký số; installer ký; không tự ghi đè file thực thi từ URL thô; publish hash SHA-256 của bản phát hành. |
| 5 | DPAPI không giải mã được khi đổi máy / đổi user | Trung bình | Nhánh DPAPI chỉ hoạt động cùng user Windows. Luôn khuyến nghị người dùng chuyển sang nhánh password-locked; nêu rõ trong wizard nhập. |
| 6 | Sai tham số Argon2 khiến không giải mã được file cũ | Trung bình | Test vector thật ở M0 trước khi làm bất cứ thứ gì khác. Không được đoán. |
| 7 | Vi phạm GPL-3.0 khi phát hành | Trung bình | Giữ LICENSE GPL-3.0, ghi rõ phần phái sinh từ RAM và tác giả gốc trong `NOTICE`. |
| 8 | Rủi ro trademark "Roblox" | Trung bình | Không dùng trong tên sản phẩm/crate/domain. Chỉ dùng dạng phụ đề mô tả. |
| 9 | Tính năng nguy hiểm bị lạm dụng (link `rbx-player`, xuất cookie) | Trung bình | Gate sau Developer Mode, cảnh báo rõ, không xuất hiện trong UI mặc định. |
| 10 | Tauri/WebView2 không có trên Windows cũ | Thấp | Yêu cầu WebView2 Runtime, kiểm tra và hướng dẫn cài khi khởi động. |
| 11 | **Xung đột phụ thuộc trong stack Tauri: `schemars 0.8.22` ↔ `indexmap 1.9.3`** | Cao (chặn build) | Đã gặp thật và đã xử lý 2026-10-10. `indexmap 1.9.3` không đưa `std` vào `default` và chỉ phát `has_std` khi có `CARGO_FEATURE_STD`; nếu không nó phó mặc cho autocfg tự dò và việc dò này có thể thất bại → `IndexMap<K, V, S>` mất tham số mặc định → `schemars` (do `tauri-utils` kéo vào với feature `preserve_order`) lỗi E0107. **Đối sách đã áp dụng:** khai `indexmap = { version = "1.9", features = ["std"] }` ở **cả** `[dependencies]` và `[build-dependencies]` của `crates/muster-app/src-tauri/Cargo.toml`, kèm comment giải thích. Đây là bug đã biết của Tauri ([tauri#14928](https://github.com/tauri-apps/tauri/issues/14928)). **Gỡ được** khi indexmap 1.x lên bản mới hoặc Tauri bỏ `schemars 0.8`. Nếu vẫn gặp: `cargo clean` + xoá `~/.cargo/registry/`. |
| 12 | **`tauri-build` luôn cần `icons/icon.ico`, kể cả khi `bundle.active = false`** | Trung bình | Sinh icon bằng `python tools/gen-icons.py`. Icon hiện tại là placeholder hình học — thay bằng logo thật khi có. |
| 13 | **Cache incremental hỏng sau khi đổi tên crate — gây ICE và lỗi I/O** | Trung bình | Gặp thật 2026-10-10, **hai triệu chứng khác nhau, cùng một nguyên nhân**: (a) `cargo test` chết với `the compiler unexpectedly panicked` trong `encode_metadata`; (b) `cargo clippy -- -D warnings` báo `error copying object file ... Access is denied. (os error 5)` — một cảnh báo I/O bị `-D warnings` nâng thành lỗi. Cả hai đều **không phải lỗi code**. **Đối sách:** `cargo clean -p <crate>` (đã kiểm chứng chữa được cả hai), hoặc `cargo clean`. `rename.sh` tự gọi `cargo clean -p` cho các crate sau khi đổi tên. |
| 14 | **`cargo test` chậm gần 200 giây vì Argon2 chạy ở profile `dev`** | Cao (chặn vòng lặp sửa–chạy) | Gặp thật 2026-10-10 khi làm 0.4. Argon2 là hàm nặng tính toán (128 MiB bộ nhớ, 6 vòng); ở `opt-level = 0` nó chậm hơn bản tối ưu khoảng 20 lần — đo được **37,9 giây cho 2 lần dẫn xuất** (~19 s mỗi lần), khiến bộ test vault mất **199 giây**. **Đối sách đã áp dụng:** `[profile.dev.package.argon2\|sha2\|crypto_secretbox\|salsa20\|poly1305] opt-level = 3` trong `Cargo.toml` gốc. `[profile.dev.package.*]` áp cho cả profile `test` vì `test` kế thừa `dev`. Sau khi sửa: **1,94 giây** cho đúng 2 lần dẫn xuất đó. Phần còn lại của workspace vẫn ở `opt-level = 0` nên vòng lặp sửa–chạy không chậm đi. |
| 15 | **`tauri-specta` v2 vẫn là release candidate** | Trung bình | `tauri-specta 2.0.0-rc.25`, `specta 2.0.0-rc.25`, `specta-typescript 0.0.12`. API còn có thể đổi giữa các bản rc, và **docs.rs không build được cho rc.25** (phải đọc docs của rc.21). **Đối sách:** ghim bằng dấu `=` đúng bản rc.25 trong `[workspace.dependencies]` để `cargo update` không âm thầm phá build. Nếu một ngày buộc phải thay: `ts-rs` là phương án dự phòng cho phần sinh type (nhưng không sinh được wrapper command). |
| 16 | **`cargo test -p muster-app` chết với `Access is denied` trong `target/debug/build/.../out/`** | Trung bình | Gặp thật 2026-10-10 khi sinh bindings. Cùng họ với rủi ro #13 nhưng ở thư mục output của build script (`tauri::generate_context!` đọc asset từ đó). **Đối sách:** `cargo clean -p muster-app` rồi chạy lại — đã kiểm chứng chữa được. Lưu ý là nó **tái phát** sau vài lần build; nếu gặp lại thì cứ clean, không phải lỗi code. |
| 17 | **Test binary của `muster-app` không nạp được trên Windows** | Trung bình | `cargo test -p muster-app` sinh ra test binary chết ngay khi nạp: chạy trực tiếp trả mã thoát **127**, cargo báo `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)`. Đã loại trừ hai nghi vấn: (a) `crate-type` — bỏ `staticlib`/`cdylib` **không** chữa được; (b) `dbghelp.dll` cũ trên `PATH` — chỉ có bản của System32. So bảng import với test binary của `muster-core` (chạy được) thì phần dư toàn là DLL giao diện do Tauri kéo vào: `comctl32`, `dwmapi`, `gdi32`, `ole32`, `oleaut32`, `shcore`, `shell32`, `user32`, `uxtheme`. **Nghi vấn chính:** test binary không được gắn manifest Common Controls 6.0 như binary ứng dụng (`tauri-build` chỉ gắn cho bin), nên `comctl32.dll` nạp bản v5 vốn thiếu vài export. **Đối sách đã áp dụng:** việc sinh bindings nằm ở binary riêng `muster-codegen` (`cargo run -p muster-app --bin muster-codegen`, hoặc `npm run codegen`), và CI chạy `cargo test --workspace --exclude muster-app`. Việc loại trừ này hợp lý về kiến trúc: nguyên tắc bất biến #3 nói `muster-app` chỉ chứa command wrapper, không có logic gì để test. **Còn tồn:** muốn test thật trong crate này thì phải tìm cách gắn manifest cho test binary. |

---

## 8. Quality gates

Áp dụng cho mọi milestone:

- [ ] `cargo fmt --check` và `cargo clippy --all-targets -- -D warnings` sạch.
- [ ] `cargo test` xanh; mọi module trong `core` có test đơn vị cho phần logic thuần.
- [ ] `tsc --noEmit` sạch, ESLint sạch.
- [ ] Không có `unwrap()`/`expect()` trên đường đi có thể lỗi do input người dùng hoặc mạng.
- [ ] Không log cookie, auth ticket, password — có test kiểm tra redaction.
- [ ] Mọi command Tauri phải có kiểu trả về rõ ràng và được `tauri-specta` sinh sang TS.
- [ ] `docs/` cập nhật cùng lúc với code, không để sau.
- [ ] Mỗi tính năng tương thích ngược phải có test vector đi kèm.

**Định nghĩa "xong" cho một tính năng port từ RAM:** hành vi quan sát được giống RAM, có test chứng minh,
và có ghi chú trong `docs/COMPATIBILITY.md` nêu rõ điểm khác biệt (nếu có).

---

## 9. Pháp lý & ToS

- **License:** repo này là **GPL-3.0**. `RAM_Source/` là GPL-3.0 (tác giả ic3w0lf22). Mọi bản phái sinh
  — bao gồm bản port sang Rust — bắt buộc phải là GPL-3.0 và phải công khai mã nguồn.
  Ghi công tác giả gốc trong `NOTICE` và giữ nguyên `LICENSE`.
- **Ranh giới với RAM:** nếu muốn license khác GPL, KHÔNG được copy code. Khi đó chỉ được tham khảo
  hành vi API công khai (endpoint, định dạng file) chứ không được dịch mã nguồn.
- **Không redistribute `handle.exe`:** Sysinternals có điều khoản riêng. Đây là lý do kỹ thuật và
  pháp lý để thay thế nó ở M3.
- **Trademark:** "Roblox" là thương hiệu của Roblox Corporation. Không dùng trong tên sản phẩm, không
  ngụ ý được Roblox chứng thực.
- **ToS:** chỉ out-of-process (mục 2, nguyên tắc 1). Không hứa hẹn "không bao giờ bị ban" ở bất kỳ
  tài liệu nào.

---

## 10. Câu hỏi mở

Cần chốt trước hoặc trong M0:

1. **Tên dự án** — quyết định này chặn việc scaffold repo. Xem mục 1.
2. ~~**Có bắt buộc hỗ trợ macOS ngay từ đầu không?**~~ **ĐÃ CHỐT 2026-10-10: KHÔNG.**
   Chỉ Windows. Không trừu tượng hoá OS ở M0. M4 (đa nền tảng) bị đẩy xuống backlog, không cam kết.
   Hệ quả: được phép dùng thẳng Win32/registry/DPAPI ở tầng core mà không cần trait trừu tượng.
3. **Có giữ tính năng captcha/Nopecha không?** Nó cần API trả phí của bên thứ ba và làm phức tạp
   đáng kể phần browser automation.
4. **Có làm plugin system không?** Nếu có, cần định hình interface ổn định sớm, trước khi core đóng băng.
5. **Mô hình phát hành:** miễn phí hoàn toàn, hay có bản trả phí? Ảnh hưởng tới quyết định về
   telemetry, updater, và mức độ đầu tư vào UI.
6. **Có cần chế độ nhiều người dùng trên cùng một máy không?** Ảnh hưởng tới cách lưu vault.

---

## 11. Nhật ký thay đổi tài liệu

| Ngày | Thay đổi |
| :--- | :--- |
| 2026-10-10 | Bản đầu. Dựa trên phân tích trực tiếp `RAM_Source/` (16.015 dòng C#). |
| 2026-10-10 | Rà soát lần 2. **Xác minh được biến thể Argon2 = Argon2i v1.3** (libsodium-net 0.10.0 + libsodium.dll 1.0.11), đóng rủi ro #6 ở mức "biết chắc thuật toán". Ghi lại offset thực tế của `RAMHeader`/salt/nonce. Xác nhận URL regex từ xa là raw GitHub không ký (`RobloxProcess.cs:45`). Thêm mục 6.0 (thứ tự thực thi M0) và mục 12 (chiến lược tên tạm). |
| 2026-10-10 | Rà soát lần 3. **Chốt hoàn toàn tham số Argon2** bằng dịch ngược IL `Sodium.dll` + chạy lại trên native libsodium 1.0.11: `StrengthArgon.Moderate` = `Argon2i v1.3, t=6, m=131072 KiB, p=1, out=32`. Sửa lại giá trị `opslimit` sai trong bản đầu (8388608 → 6). Sửa ghi chú sai về cờ `NoEncryption...`. **Chốt phạm vi: chỉ Windows** — mục 10 câu hỏi #2 đã trả lời, M4 (macOS) chuyển thành backlog không cam kết. |
| 2026-10-10 | Rà soát lần 4. Phát hiện **RAM không có đường đăng nhập bằng API nào** — `user:pass` chỉ được gõ sẵn vào form Chromium (`AccountManager.cs:1396-1417`). **Chốt chiến lược "API trước, browser sau"**, viết tài liệu riêng `docs/BROWSER.md` (3 tầng, luồng UX 7 bước, nguồn tải Chrome for Testing đã kiểm chứng, danh sách thứ KHÔNG copy từ RAM). Thêm task ⭐ vào M1, thêm 6 task browser vào M2/M3. Sửa con số dung lượng: ~150 MB → **~198 MiB** (đo thật). |
| 2026-10-10 | Rà soát lần 5. **M0 bước 0.3 XONG** — workspace 4 crate dựng được và build xanh (`cargo check --workspace`, `cargo test`, `clippy -D warnings`, `tsc --noEmit`, Vite build). Thêm `tools/rename.sh` (5 bước, đã kiểm chứng bằng một vòng đổi tên thật) và `tools/gen-icons.py`. Ghi lại 2 cạm bẫy build thật đã gặp: xung đột `schemars`/`indexmap` (rủi ro #11) và ICE do cache incremental sau khi đổi tên (rủi ro #13). Mục 7 thêm rủi ro #11–#13. |
| 2026-10-10 | Rà soát lần 6. Bổ sung **mục 6.0-UI — nhánh UI/UX chạy song song**. Trước đó tài liệu không có bước thiết kế UI nào; các task UI chỉ nằm rải rác ở M1–M3 nên không thấy được toàn cảnh. Ghi rõ điểm phụ thuộc cứng (UI không được tự định nghĩa type dữ liệu — phải chờ `tauri-specta` ở 0.5 và model `Account` ở M1), phân chia sở hữu giữa hai nhánh, và 6 task thiết kế/hạ tầng. Ghi thêm giới hạn đã biết của `rename.sh` ở mục 12. |

---

## 12. Chiến lược tên tạm (quyết định 2026-10-10)

Tên dự án **vẫn chưa chốt**, nhưng **không được phép chặn công việc**. Quyết định:

1. Dùng **`muster`** làm codename tạm (đã là ví dụ trong tài liệu này, nên không phát sinh thêm khái niệm).
   Mọi nơi dùng placeholder `<PROJECT>` / `<project>` / `<slug>` / `<BINARY>` như mục 1.
2. **Đổi tên phải là thao tác một lệnh.** Tạo `tools/rename.sh` (hoặc `cargo xtask rename`) làm đúng việc:
   - đổi tên thư mục crate + `name` trong mọi `Cargo.toml`;
   - đổi `identifier` trong `tauri.conf.json` (`dev.<slug>.app`);
   - đổi tên file `.exe` mong muốn trong `tauri.conf.json` (`productName`);
   - thay toàn bộ `<PROJECT>`/`<project>`/`<slug>`/`<BINARY>` trong `*.md`, `*.rs`, `*.ts`, `*.json`;
   - **không** sửa `RAM_Source/` (đó là nguồn tham chiếu bất biến).
3. Điều kiện "xong" của một lần đổi tên: `cargo build --workspace` + `cargo test` + `tsc --noEmit` vẫn xanh
   sau khi chạy script, và `git diff` không chạm `RAM_Source/`.

Nhờ vậy việc chọn tên trở thành một thao tác rẻ, có thể làm ở bất kỳ thời điểm nào — kể cả sau M2.

**Giới hạn đã biết của script** (gặp thật 2026-10-10 khi kiểm chứng): phép thay thế là thay chuỗi
thuần, nên **câu văn có chứa cả tên cũ lẫn tên mới sẽ bị làm hỏng**. Ví dụ một câu viết
"đổi sang tên khác rồi khôi phục" thì không sao, nhưng câu dạng "A → B → kiểm tra" sẽ bị đổi
thành "A → A" sau một vòng đổi đi rồi đổi lại. Vì vậy: **sau khi đổi tên, luôn đọc lại
`ROADMAP.md` mục 6.0 và 12 để sửa tay những câu như vậy.** Script đã in cảnh báo này ở bước cuối.
