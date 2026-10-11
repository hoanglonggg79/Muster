import hashlib
import json

from argon2.low_level import hash_secret_raw, Type

PASSWORD = b"correct horse battery staple"
SALT = bytes(range(16))
SECRET = hashlib.sha512(PASSWORD).digest()

TARGET = json.load(open("libsodium_vectors.json"))

PLAN = {
    "Moderate": 134217728 // 1024,
    "Interactive": 33554432 // 1024,
    "Sensitive": 536870912 // 1024,
}

results = {}
for name, m_cost in PLAN.items():
    target = bytes.fromhex(TARGET[name])
    hit = None
    for t in range(1, 13):
        got = hash_secret_raw(
            secret=SECRET, salt=SALT,
            time_cost=t, memory_cost=m_cost, parallelism=1,
            hash_len=32, type=Type.I, version=19,
        )
        if got == target:
            hit = t
            break
    results[name] = {"t_cost": hit, "m_cost": m_cost, "p_cost": 1}
    print(f"{name:12} m_cost={m_cost:<8} KiB  ->  t_cost={hit}")

json.dump(results, open("rust_params.json", "w"), indent=2)
print("\n" + json.dumps(results, indent=2))
