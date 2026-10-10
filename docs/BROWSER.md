<PROJECT> — Trình duyệt & luồng đăng nhập
================================================================================

Trạng thái: **ĐÃ CHỐT** ngày 2026-10-10. Thay thế mọi mô tả mơ hồ trước đó về "port AccountBrowser".
Liên quan: ROADMAP mục 4.1 (vault), mục 5 (tái sử dụng RAM_Source), M1/M2/M3.

Quyết định số 1: **API trước, browser sau.**
Quyết định số 2: **không nhúng sẵn trình duyệt.** Chỉ tải khi người dùng thực sự cần.

--------------------------------------------------------------------------------
## 1. Vì sao phải tách hai đường

RAM trộn hai nhu cầu khác nhau vào cùng một Chromium, và vì thế bắt **mọi** người dùng tải
~200 MB ngay lần chạy đầu, kể cả khi họ chỉ dùng `user:pass` hoặc import cookie.

Đã kiểm chứng trong `RAM_Source`: **RAM không có đường đăng nhập bằng API nào cả.**
Chỉ có duy nhất một chỗ nhắc `auth.roblox.com/v2/login` — `AccountBrowser.Page_RequestFinished`
— và đó là *quan sát* request do Chromium gửi, không phải tự gửi. `user:pass` trong RAM chỉ là
chữ được gõ sẵn vào form của trình duyệt; hàm nhập hàng loạt
(`AccountManager.cs:1396-1417`) mở **một cửa sổ Chromium cho mỗi dòng**.

`<PROJECT>` làm khác: đường API là đường chính, trình duyệt là đường dự phòng.

--------------------------------------------------------------------------------
## 2. Ba tầng, thử từ trên xuống

### Tầng 0 — Dùng trình duyệt có sẵn trên máy  (0 MB tải về)

Edge có mặt trên **mọi** Windows 10/11 (Microsoft cài sẵn). Chrome thì rất phổ biến.
Cả hai đều là Chromium nên nói cùng một giao thức CDP.

Cách làm: khởi chạy trình duyệt có sẵn với một profile **riêng, tách biệt hoàn toàn**,
rồi kết nối qua CDP.

```
msedge.exe  (hoặc chrome.exe)
  --remote-debugging-port=0          # tự chọn cổng, ghi vào DevToolsActivePort
  --user-data-dir=%LOCALAPPDATA%\<PROJECT>\browser-profile\<session-id>
  --no-first-run --no-default-browser-check
```

- **Tuyệt đối không dùng profile mặc định của người dùng.** Dùng thư mục riêng, xoá sau khi xong.
- Đọc cổng thật từ file `DevToolsActivePort` trong `--user-data-dir`, rồi mới kết nối.
- ⚠️ Cảnh báo cần xử lý: trong môi trường doanh nghiệp, Edge/Chrome có thể bị policy khoá
  remote debugging → khởi chạy sẽ thất bại. Khi đó rơi xuống Tầng 1.
- ⚠️ Trình duyệt có thể tự cập nhật giữa các lần chạy → không ghim được version. Chấp nhận được
  cho mục đích đăng nhập; nếu cần version cố định thì dùng Tầng 1.

**Đây là tầng mặc định.** Trên máy dev hiện tại đã có sẵn cả hai:
`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe` và
`C:\Program Files\Google\Chrome\Application\chrome.exe`.

### Tầng 1 — Tải Chrome for Testing  (~198 MiB)

Nguồn **chính thức của Google**, có version và có JSON API để tra cứu. Khác hẳn cách RAM làm
(RAM dùng `BrowserFetcher` của Puppeteer trỏ vào `chromium-browser-snapshots` — ảnh chụp không
có version, không có bảo đảm).

Endpoint tra cứu:

| Endpoint | Dùng để |
| :--- | :--- |
| `https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json` | Lấy bản Stable mới nhất (10,4 KB) |
| `https://googlechromelabs.github.io/chrome-for-testing/known-good-versions-with-downloads.json` | Toàn bộ version đã biết (lớn — chỉ dùng khi cần ghim) |

Mẫu URL tải (đã kiểm tra trả HTTP 200):

```
https://storage.googleapis.com/chrome-for-testing-public/<version>/win64/chrome-win64.zip
https://storage.googleapis.com/chrome-for-testing-public/<version>/win64/chrome-headless-shell-win64.zip
```

Số liệu thật đo ngày 2026-10-10 (Stable `155.0.8059.39`, revision `r1697595`):

| Gói | Kích thước thật |
| :--- | ---: |
| `chrome-win64.zip` | **207.197.429 byte ≈ 197,6 MiB** |
| `chrome-headless-shell-win64.zip` | 124.737.213 byte ≈ 118,9 MiB |

⚠️ Con số "~150 MB" trong các ghi chú cũ là **thấp hơn thực tế**. Dùng ~198 MiB cho mọi
thông báo dung lượng trong UI.

Thư viện Rust: crate **`chromiumoxide_fetcher`** (tách ra từ `chromiumoxide`) tải và cài đặt
được cả Chromium và Chrome for Testing, theo tag hoặc theo revision cụ thể.

### Tầng 2 — Không có gì cả

Nếu người dùng từ chối tải và máy không có Edge/Chrome, các đường API và import cookie
**vẫn hoạt động bình thường**. Chỉ những tài khoản cần captcha/2FA mới bị chặn, và app phải
nói rõ điều đó thay vì im lặng thất bại.

--------------------------------------------------------------------------------
## 3. Luồng UX (đã chốt)

Nguyên tắc: **không tải gì cho tới khi có một thất bại cụ thể cần tới nó.**

```
Người dùng bấm "Thêm tài khoản" và nhập user:pass
        │
        ▼
   [1] Gọi API login  (POST auth.roblox.com/v2/login + CSRF)
        │
        ├── 200 + Set-Cookie .ROBLOSECURITY ──► Lưu vào vault. XONG. Không tải gì.
        │
        └── Roblox đòi captcha / 2FA / lỗi khác
                 │
                 ▼
   [2] Hiện thông báo rõ ràng, kèm nút "Tải trình duyệt"
       "Tài khoản này cần xác minh. Cần trình duyệt để đăng nhập thủ công."
                 │
                 ▼
   [3] Người dùng bấm nút → MÀN HÌNH HIỆN TẠI TẠM DỪNG
       (khoá tương tác, không cho đóng giữa chừng)
                 │
                 ▼
   [4] Popup tiến trình tải, có: phần trăm, số MB đã tải / tổng, tốc độ, nút Huỷ
                 │
                 ▼
   [5] Giải nén → xác minh chữ ký Authenticode của chrome.exe
       (nhà phát hành phải là Google) TRƯỚC KHI chạy lần đầu
                 │
                 ▼
   [6] Tải xong → bỏ tạm dừng → mở cửa sổ đăng nhập cho đúng tài khoản đó
                 │
                 ▼
   [7] Bắt cookie .ROBLOSECURITY → lưu vault → đóng trình duyệt → xoá profile tạm
```

Chi tiết bắt buộc:

- **Không tự động tải trong nền.** Việc tải chỉ bắt đầu từ một cú bấm của người dùng.
- **Tạm dừng thật sự.** Trong lúc tải, không cho thao tác khác; nếu thoát app giữa chừng thì
  lần sau phải phát hiện được file tải dở và tải lại từ đầu (RAM cảnh báo "exiting may corrupt
  your chromium installation" — ta tránh bằng cách giải nén ra thư mục tạm rồi mới đổi tên).
- **Tải dở dang không được để lại rác.** Giải nén vào `<đích>.tmp-<random>`, xác minh, rồi
  `rename` nguyên tử sang `<đích>`.
- Có nút **Huỷ**, và sau khi huỷ thì quay lại đúng màn hình ở bước [2].

--------------------------------------------------------------------------------
## 4. Bắt cookie — chi tiết kỹ thuật

Không cần mô phỏng thao tác người dùng. Cách gọn nhất:

1. Mở `https://roblox.com/login` (KHÔNG headless — người dùng phải thấy và tự giải captcha).
2. Chờ người dùng đăng nhập xong. Dấu hiệu: CDP `Network.responseReceived` thấy
   `POST https://auth.roblox.com/v2/login` trả về, hoặc URL đổi sang `/home`.
   (RAM làm tương tự: `AccountBrowser.Page_RequestFinished`, `AccountBrowser.cs:307-319`.)
3. Đọc cookie bằng CDP `Network.getCookies` với URL `https://roblox.com/`, tìm `.ROBLOSECURITY`.
4. Lưu vào vault, đóng trình duyệt, xoá profile tạm.

Bàn giao diện: dùng crate `chromiumoxide` (async, CDP). Không cần `chromedriver`.

Body của request login, đọc từ `AccountBrowser.cs:309-312`:

```json
{ "ctype": "Username", "cvalue": "<username>", "password": "<password>" }
```

Hai khoá `ctype` (giá trị `Username`) và `password` đã xác nhận từ source. Tên khoá chứa
username khả năng cao là `cvalue` — **phải xác minh lại bằng request thật khi code Tầng 1**,
vì đây là thứ dễ sai và sai thì lỗi trả về sẽ rất mơ hồ.

--------------------------------------------------------------------------------
## 5. Những thứ KHÔNG copy từ RAM

RAM bật sẵn một số thứ mà `<PROJECT>` cố tình không làm. Ghi lại để không ai vô tình port sang.

| RAM làm | `<PROJECT>` làm | Lý do |
| :--- | :--- | :--- |
| `--disable-web-security` mặc định (`AccountBrowser.cs:64`) | **Không bao giờ** bật mặc định | Tắt same-origin policy. Chỉ bật trong tính năng "URL + JS tuỳ chỉnh" sau Developer Mode, kèm cảnh báo |
| `StealthPlugin` (PuppeteerExtraSharp) để né phát hiện bot | Không dùng mặc định | Né phát hiện là cuộc chiến không hồi kết, và là rủi ro ToS |
| Nopecha tự giải FunCaptcha (`NopechaKey`) | Không tích hợp | API bên thứ ba trả phí; xử lý khoá API và dữ liệu người dùng rất phức tạp |
| Cào password từ body POST để lưu (`AccountBrowser.cs:311`) | Chỉ lưu khi `SavePasswords` bật, và **không bao giờ** ghi log | Nguyên tắc bất biến #4 |
| Nạp extension tuỳ ý từ thư mục `extension/` | Không ở bản đầu | Mở rộng bề mặt tấn công |
| Tải từ `chromium-browser-snapshots` (không version) | Tầng 0 có sẵn, hoặc Chrome for Testing | Nguồn có version, có JSON API, do Google phát hành |

--------------------------------------------------------------------------------
## 6. Vị trí trên đĩa

```
%LOCALAPPDATA%\<PROJECT>\
├─ browser\
│  └─ chrome-for-testing\<version>\      # giải nén ở đây, giữ được nhiều version
└─ browser-profile\
   └─ <session-id>\                      # profile tạm, xoá ngay sau khi lấy cookie
```

- Kích thước tối đa cho phép: đặt hạn mức, và có nút "Xoá trình duyệt đã tải" trong Settings.
- **Không** đặt trong thư mục cài đặt (Program Files) — cần quyền ghi.

--------------------------------------------------------------------------------
## 7. Việc cần làm

- [ ] M1 — `core::roblox::auth`: đăng nhập bằng API từ `user:pass` → cookie (đường chính)
- [ ] M1 — nhận diện và phân loại lỗi API: sai mật khẩu / captcha / 2FA / rate limit.
      Đây là điều kiện để biết khi nào cần đề nghị tải trình duyệt.
- [ ] M2 — `core::browser`: dò trình duyệt có sẵn (Tầng 0), khởi chạy + CDP
- [ ] M2 — `core::browser::fetch`: tải Chrome for Testing, xác minh chữ ký, cài nguyên tử
- [ ] M2 — UI: nút "Tải trình duyệt" + popup tiến trình + cơ chế tạm dừng màn hình
- [ ] M2 — `core::browser::login`: mở trang login, chờ, bắt cookie, dọn profile
- [ ] M3 — tính năng "duyệt web dưới danh nghĩa account" (gate sau Developer Mode)
- [ ] M3 — nút xoá trình duyệt đã tải + hạn mức dung lượng
