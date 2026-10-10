#!/usr/bin/env python3
"""
Sinh test vector cho core::vault  (ROADMAP buoc 0.1).

Muc tieu: tao ra 4 file mau dai dien cho 4 nhanh dinh dang ma Roblox Account Manager
(RAM) co the ghi ra, de `core::vault` co hop dong bat bien de kiem thu.

Nhanh 1  password-locked   : RAMHeader(64) | salt(16) | nonce(24) | secretbox ciphertext
Nhanh 2  DPAPI CurrentUser : CryptProtectData(json, entropy, CurrentUser)
Nhanh 3  DPAPI LocalMachine: CryptProtectData(json, entropy, LocalMachine)
Nhanh 4  plain JSON        : UTF-8 JSON tho

Tham so Argon2 da duoc xac minh bang cach dich nguoc IL cua Sodium.dll (libsodium-net 0.10.0)
va doi chieu voi native libsodium 1.0.11:
    Algorithm = Argon2i (v1.3), t_cost = 6, m_cost = 131072 KiB (128 MiB), p_cost = 1, out = 32
    dau vao Argon2 = SHA-512(password)  (64 byte), KHONG phai password tho

Xem ROADMAP.md muc 4.1.

Chay:  python tools/fixtures-gen/generate.py
Ket qua ghi vao: crates/muster-core/tests/fixtures/
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes as wt
import hashlib
import json
import os
import secrets
import sys

# ---------------------------------------------------------------- hang so da xac minh

RAM_HEADER = bytes(
    [82, 111, 98, 108, 111, 120, 32, 65, 99, 99, 111, 117, 110, 116, 32, 77, 97, 110, 97, 103,
     101, 114, 32, 99, 114, 101, 97, 116, 101, 100, 32, 98, 121, 32, 105, 99, 51, 119, 48, 108,
     102, 50, 50, 32, 64, 32, 103, 105, 116, 104, 117, 98, 46, 99, 111, 109, 32, 46, 46, 46, 46,
     46, 46, 46]
)
assert len(RAM_HEADER) == 64, len(RAM_HEADER)
assert RAM_HEADER.startswith(b"Roblox Account Manager created by ic3w0lf22 @ github.com")

# AccountManager.cs:93  -- 56 byte, giu nguyen loi chinh ta "BUY"
DPAPI_ENTROPY = bytes(
    [0x52, 0x4f, 0x42, 0x4c, 0x4f, 0x58, 0x20, 0x41, 0x43, 0x43, 0x4f, 0x55, 0x4e, 0x54, 0x20,
     0x4d, 0x41, 0x4e, 0x41, 0x47, 0x45, 0x52, 0x20, 0x7c, 0x20, 0x3a, 0x29, 0x20, 0x7c, 0x20,
     0x42, 0x52, 0x4f, 0x55, 0x47, 0x48, 0x54, 0x20, 0x54, 0x4f, 0x20, 0x59, 0x4f, 0x55, 0x20,
     0x42, 0x55, 0x59, 0x20, 0x69, 0x63, 0x33, 0x77, 0x30, 0x6c, 0x66]
)
assert len(DPAPI_ENTROPY) == 56, len(DPAPI_ENTROPY)
assert DPAPI_ENTROPY.decode("ascii") == "ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf"

ARGON2_T_COST = 6
ARGON2_M_COST_KIB = 131072
ARGON2_P_COST = 1
ARGON2_OUT_LEN = 32
SALT_LEN = 16
NONCE_LEN = 24
KEY_LEN = 32

FIXTURE_PASSWORD = "ram-fixture-password"  # chi la du lieu tong hop, khong phai bi mat that

CRYPTPROTECT_UI_FORBIDDEN = 0x1
CRYPTPROTECT_LOCAL_MACHINE = 0x4

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
OUT_DIR = os.path.join(ROOT, "crates", "muster-core", "tests", "fixtures")


# ---------------------------------------------------------------- payload mau

def build_payload() -> bytes:
    """JSON gia lap theo dung hinh dang `JsonConvert.SerializeObject(List<Account>)`.

    Da khu dinh danh: cookie va UserID deu la gia tri bia.
    """
    accounts = [
        {
            "Valid": True,
            "SecurityToken": "FIXTURE_ONLY_NOT_A_REAL_COOKIE_0000000000000000000000000000",
            "Username": "fixture_alpha",
            "LastUse": "2026-09-28T21:14:03.512",
            "Group": "Default",
            "UserID": 100000001,
            "Fields": {
                "Window_Position_X": "120",
                "Window_Position_Y": "80",
                "Window_Width": "880",
                "Window_Height": "740",
            },
            "LastAttemptedRefresh": "2026-10-01T07:00:00.104",
            "BrowserTrackerID": "100000000000001",
            "Alias": "Alpha",
            "Description": "fixture account, khong phai tai khoan that",
            "Password": "",
        },
        {
            "Valid": True,
            "SecurityToken": "FIXTURE_ONLY_NOT_A_REAL_COOKIE_1111111111111111111111111111",
            "Username": "fixture_beta",
            "LastUse": "2026-10-02T09:45:11.900",
            "Group": "007 Bank",
            "UserID": 100000002,
            "Fields": {"Window_Position_X": "0", "Window_Position_Y": "0"},
            "LastAttemptedRefresh": "2026-10-02T09:45:12.001",
            "BrowserTrackerID": "100000000000002",
            "Alias": "Beta",
            "Description": "",
            "Password": "",
        },
    ]
    return json.dumps(accounts, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


# ---------------------------------------------------------------- nhanh 1

def derive_key(password: str, salt: bytes) -> bytes:
    from argon2.low_level import Type, hash_secret_raw

    return hash_secret_raw(
        secret=hashlib.sha512(password.encode("utf-8")).digest(),
        salt=salt,
        time_cost=ARGON2_T_COST,
        memory_cost=ARGON2_M_COST_KIB,
        parallelism=ARGON2_P_COST,
        hash_len=ARGON2_OUT_LEN,
        type=Type.I,
        version=19,
    )


def make_password_locked(payload: bytes, password: str) -> tuple[bytes, bytes, bytes]:
    from nacl.secret import SecretBox

    salt = secrets.token_bytes(SALT_LEN)
    nonce = secrets.token_bytes(NONCE_LEN)
    key = derive_key(password, salt)
    ciphertext = SecretBox(key).encrypt(payload, nonce).ciphertext
    blob = RAM_HEADER + salt + nonce + ciphertext
    return blob, salt, nonce


# ---------------------------------------------------------------- nhanh 2/3

class DATA_BLOB(ctypes.Structure):
    _fields_ = [("cbData", wt.DWORD), ("pbData", ctypes.POINTER(ctypes.c_char))]


def _blob(data: bytes) -> DATA_BLOB:
    buf = ctypes.create_string_buffer(data, len(data))
    return DATA_BLOB(len(data), ctypes.cast(buf, ctypes.POINTER(ctypes.c_char)))


def make_dpapi(payload: bytes, local_machine: bool) -> bytes:
    if not sys.platform.startswith("win"):
        raise RuntimeError("DPAPI chi co tren Windows")

    crypt32 = ctypes.WinDLL("crypt32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

    blob_in = _blob(payload)
    blob_entropy = _blob(DPAPI_ENTROPY)
    blob_out = DATA_BLOB()

    flags = CRYPTPROTECT_UI_FORBIDDEN | (CRYPTPROTECT_LOCAL_MACHINE if local_machine else 0)

    ok = crypt32.CryptProtectData(
        ctypes.byref(blob_in), None, ctypes.byref(blob_entropy), None, None, flags,
        ctypes.byref(blob_out),
    )
    if not ok:
        raise OSError(f"CryptProtectData that bai: {ctypes.get_last_error()}")

    try:
        return ctypes.string_at(blob_out.pbData, blob_out.cbData)
    finally:
        kernel32.LocalFree(blob_out.pbData)


def make_dpapi_unprotect(blob: bytes, local_machine: bool) -> bytes:
    crypt32 = ctypes.WinDLL("crypt32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)

    blob_in = _blob(blob)
    blob_entropy = _blob(DPAPI_ENTROPY)
    blob_out = DATA_BLOB()
    flags = CRYPTPROTECT_UI_FORBIDDEN | (CRYPTPROTECT_LOCAL_MACHINE if local_machine else 0)

    ok = crypt32.CryptUnprotectData(
        ctypes.byref(blob_in), None, ctypes.byref(blob_entropy), None, None, flags,
        ctypes.byref(blob_out),
    )
    if not ok:
        raise OSError(f"CryptUnprotectData that bai: {ctypes.get_last_error()}")
    try:
        return ctypes.string_at(blob_out.pbData, blob_out.cbData)
    finally:
        kernel32.LocalFree(blob_out.pbData)


# ---------------------------------------------------------------- main

def main() -> int:
    if sys.platform.startswith("win"):
        try:
            ctypes.windll.kernel32.SetConsoleOutputCP(65001)
        except Exception:
            pass

    os.makedirs(OUT_DIR, exist_ok=True)
    payload = build_payload()
    manifest = {
        "generated_by": "tools/fixtures-gen/generate.py",
        "note": "Du lieu tong hop, da khu dinh danh. Khong chua cookie that.",
        "verified_params": {
            "kdf": "Argon2i v1.3",
            "t_cost": ARGON2_T_COST,
            "m_cost_kib": ARGON2_M_COST_KIB,
            "p_cost": ARGON2_P_COST,
            "out_len": ARGON2_OUT_LEN,
            "kdf_input": "SHA-512(password)",
            "salt_len": SALT_LEN,
            "nonce_len": NONCE_LEN,
            "ram_header_len": len(RAM_HEADER),
        },
        "fixtures": [],
    }

    # nhanh 4
    p = os.path.join(OUT_DIR, "accountdata.plain.json")
    with open(p, "wb") as f:
        f.write(payload)
    manifest["fixtures"].append(
        {"file": os.path.basename(p), "branch": 4, "kind": "plain-json",
         "detect": "byte dau tien la '['", "password": None}
    )
    print("[4/4] plain JSON          ->", os.path.basename(p), len(payload), "byte")

    # nhanh 1
    blob, salt, nonce = make_password_locked(payload, FIXTURE_PASSWORD)
    p = os.path.join(OUT_DIR, "accountdata.password-locked.bin")
    with open(p, "wb") as f:
        f.write(blob)
    manifest["fixtures"].append(
        {"file": os.path.basename(p), "branch": 1, "kind": "password-locked",
         "detect": "64 byte dau khop RAMHeader",
         "password": FIXTURE_PASSWORD, "salt_hex": salt.hex(), "nonce_hex": nonce.hex(),
         "size": len(blob)}
    )
    print("[1/4] password-locked     ->", os.path.basename(p), len(blob), "byte")

    # nhanh 2 / 3
    for branch, local_machine, fname in (
        (2, False, "accountdata.dpapi-currentuser.bin"),
        (3, True, "accountdata.dpapi-localmachine.bin"),
    ):
        try:
            blob = make_dpapi(payload, local_machine)
        except OSError as e:
            print(f"[{branch}/4] DPAPI BO QUA: {e}")
            manifest["fixtures"].append(
                {"file": fname, "branch": branch, "kind": "dpapi",
                 "skipped": str(e), "scope": "LocalMachine" if local_machine else "CurrentUser"}
            )
            continue
        p = os.path.join(OUT_DIR, fname)
        with open(p, "wb") as f:
            f.write(blob)
        manifest["fixtures"].append(
            {"file": fname, "branch": branch, "kind": "dpapi",
             "scope": "LocalMachine" if local_machine else "CurrentUser",
             "entropy_ascii": DPAPI_ENTROPY.decode("ascii"), "size": len(blob)}
        )
        print(f"[{branch}/4] DPAPI {'LocalMachine' if local_machine else 'CurrentUser':13} ->",
              fname, len(blob), "byte")

    # self-check: giai ma lai va doi chieu
    print("\nself-check:")
    ok = True

    from nacl.secret import SecretBox

    with open(os.path.join(OUT_DIR, "accountdata.password-locked.bin"), "rb") as f:
        raw = f.read()
    s2, n2, ct = raw[64:80], raw[80:104], raw[104:]
    dec = SecretBox(derive_key(FIXTURE_PASSWORD, s2)).decrypt(n2 + ct)
    ok &= dec == payload
    print("  nhanh 1 password-locked :", "OK" if dec == payload else "FAIL")

    if sys.platform.startswith("win"):
        for local_machine, fname in ((False, "accountdata.dpapi-currentuser.bin"),
                                     (True, "accountdata.dpapi-localmachine.bin")):
            fp = os.path.join(OUT_DIR, fname)
            if not os.path.exists(fp):
                continue
            with open(fp, "rb") as f:
                blob = f.read()
            try:
                dec = make_dpapi_unprotect(blob, local_machine)
                good = dec == payload
            except OSError as e:
                dec, good = b"", False
                print(f"    ({e})")
            ok &= good
            print(f"  nhanh {2 if not local_machine else 3} DPAPI {'LocalMachine' if local_machine else 'CurrentUser':13}:",
                  "OK" if good else "FAIL")

    with open(os.path.join(OUT_DIR, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2, ensure_ascii=False)
    print("\nmanifest ->", os.path.join(OUT_DIR, "manifest.json"))
    print("ket qua:", "TAT CA OK" if ok else "CO LOI")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
