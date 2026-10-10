import ctypes
import hashlib
import os

HERE = os.path.dirname(os.path.abspath(__file__))
LIB = os.path.join(HERE, "libsodium-64.dll")   # libsodium 1.0.11 (x64) from libsodium-net 0.10.0

s = ctypes.CDLL(LIB)
s.sodium_init()

print("sodium_library_version_major =", s.sodium_library_version_major())
print("sodium_library_version_minor =", s.sodium_library_version_minor())

s.crypto_pwhash.restype = ctypes.c_int
s.crypto_pwhash.argtypes = [
    ctypes.c_char_p,               # out
    ctypes.c_ulonglong,            # outlen
    ctypes.c_char_p,               # passwd
    ctypes.c_ulonglong,            # passwdlen
    ctypes.c_char_p,               # salt
    ctypes.c_ulonglong,            # opslimit
    ctypes.c_size_t,               # memlimit
    ctypes.c_int,                  # alg
]

ALG_ARGON2I13 = 1

PASSWORD = b"correct horse battery staple"
SALT = bytes(range(16))

# RAM: Argon2 input is SHA-512(password), 64 bytes  (CryptoHash.Hash)
pw = hashlib.sha512(PASSWORD).digest()
print("sha512(pw) len =", len(pw), pw.hex()[:32], "...")

CASES = {
    "Interactive": (4, 33554432),
    "Moderate": (6, 134217728),
    "Sensitive": (8, 536870912),
}

out = {}
for name, (opslimit, memlimit) in CASES.items():
    buf = ctypes.create_string_buffer(32)
    rc = s.crypto_pwhash(buf, 32, pw, len(pw), SALT, opslimit, memlimit, ALG_ARGON2I13)
    print(f"{name:12} opslimit={opslimit:<4} memlimit={memlimit:<12} rc={rc} key={buf.raw.hex()}")
    out[name] = buf.raw.hex()

import json

open("libsodium_vectors.json", "w").write(json.dumps(out, indent=2))
print("\nwrote libsodium_vectors.json")
