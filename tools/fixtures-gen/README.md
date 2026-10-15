# tools/fixtures-gen

Sinh test vector cho `core::vault` — bước **0.1** của M0 (xem `ROADMAP.md` §9.1).

## Chạy

```bash
python tools/fixtures-gen/generate.py
```

Cần: Python 3.11+, `argon2-cffi`, `pynacl` (chỉ để chạy lại; fixture đã được sinh sẵn trong repo).
Windows bắt buộc cho 2 nhánh DPAPI.

Kết quả ghi vào `crates/muster-core/tests/fixtures/`.

## Vì sao có thư mục này

`core::vault` phải đọc được `AccountData.json` do Roblox Account Manager ghi ra. Không có test vector
thì mọi dòng code vault chỉ là phỏng đoán. Script này tạo ra 4 file mẫu — một cho mỗi nhánh định dạng
mà RAM có thể ghi — kèm `manifest.json` mô tả cách nhận diện từng nhánh.

Dữ liệu trong fixture là **tổng hợp và đã khử định danh**: cookie và UserID đều là giá trị bia,
không có tài khoản thật nào.

## Tham số đã xác minh (không được đoán)

| Hạng mục | Giá trị |
| :--- | :--- |
| KDF | **Argon2i v1.3** — KHÔNG phải Argon2id |
| `t_cost` | 6 |
| `m_cost` | 131072 KiB (128 MiB) |
| `p_cost` | 1 |
| Độ dài khoá ra | 32 byte |
| Đầu vào Argon2 | **SHA-512(password)** (64 byte), không phải password thô |
| Salt / Nonce | 16 byte / 24 byte |
| `RAMHeader` | 64 byte ASCII `Roblox Account Manager created by ic3w0lf22 @ github.com .......` |

Nguồn: `StrengthArgon.Moderate` của `libsodium-net 0.10.0`, dịch ngược từ `lib/Net40/Sodium.dll`
và đối chiếu với native `libsodium 1.0.11`. Bằng chứng tái lập được nằm ở `provenance/`.

## provenance/

Các script dùng để chốt tham số. Giữ lại để lần sau không phải làm lại từ đầu.

| File | Việc nó làm |
| :--- | :--- |
| `disasm_sodium_argon.py` | Dịch ngược IL của `Sodium.PasswordHash` trong `Sodium.dll`; đọc ra cặp `(opslimit, memlimit)` cho từng mức `StrengthArgon` và xác nhận lời gọi native dùng `alg = 1` (`ARGON2I13`) |
| `probe_native_libsodium.py` | Gọi thẳng `crypto_pwhash` của native libsodium 1.0.11 qua ctypes, sinh khoá tham chiếu |
| `solve_rust_params.py` | Dò ngược `(t_cost, m_cost, p_cost)` tương đương bằng implementation tham chiếu |
| `libsodium_vectors.json` | Khoá do native libsodium sinh ra (đầu vào cố định) |
| `rust_params.json` | Kết quả dò: `t_cost = 6/4/8` cho `Moderate/Interactive/Sensitive` |

Để chạy lại `provenance/`, cần tải `libsodium-net 0.10.0` từ NuGet và giải nén
`lib/Net40/Sodium.dll` cùng `output/libsodium-64.dll`.
