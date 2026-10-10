#!/usr/bin/env python3
"""
Sinh icon placeholder cho <PROJECT>  (tools/gen-icons.py).

Tauri BAT BUOC phai co `icons/icon.ico` de sinh Windows Resource File, ke ca khi
`bundle.active = false`. Repo chua co logo that, nen script nay ve tam mot icon hinh hoc
don gian — du de build chay, va de thay bang logo that sau nay bang cach chay lai script
voi tham so khac hoac thay file.

Chi dung thu vien chuan (zlib, struct) — khong can Pillow.

Chay:  python tools/gen-icons.py
Ket qua: crates/<project>-app/src-tauri/icons/
"""

from __future__ import annotations

import os
import struct
import sys
import zlib

# ---------------------------------------------------------------- mau

BG = (15, 110, 86, 255)  # teal 600
FG = (159, 225, 203, 255)  # teal 100
SUPERSAMPLE = 4

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))


def find_icons_dir() -> str:
    """Tim thu muc icons cua crate app, khong phu thuoc vao codename."""
    crates = os.path.join(ROOT, "crates")
    for name in sorted(os.listdir(crates)):
        candidate = os.path.join(crates, name, "src-tauri", "icons")
        if os.path.isdir(os.path.join(crates, name, "src-tauri")):
            return candidate
    raise SystemExit("khong tim thay crates/*/src-tauri/")


# ---------------------------------------------------------------- hinh hoc

def rounded_rect_hit(x: float, y: float, w: float, h: float, r: float,
                     px: float, py: float) -> bool:
    """Diem (px, py) co nam trong hinh chu nhat bo goc khong."""
    if px < x or px >= x + w or py < y or py >= y + h:
        return False
    cx = min(max(px, x + r), x + w - r)
    cy = min(max(py, y + r), y + h - r)
    dx = px - cx
    dy = py - cy
    return dx * dx + dy * dy <= r * r


def render(size: int) -> list[list[tuple[int, int, int, int]]]:
    """Ve icon o kich thuoc `size`, tra ve ma tran diem anh RGBA."""
    s = size * SUPERSAMPLE
    acc = [[[0, 0, 0, 0] for _ in range(size)] for _ in range(size)]

    bg_r = s * 0.22
    bar_w = s * 0.56
    bar_h = s * 0.085
    bar_r = bar_h / 2
    bar_x = (s - bar_w) / 2
    gap = s * 0.075
    total_h = bar_h * 3 + gap * 2
    top = (s - total_h) / 2

    bars = [(bar_x, top + i * (bar_h + gap)) for i in range(3)]
    # Thanh thu ba ngan hon — trong nhu mot danh sach.
    bar_widths = [bar_w, bar_w, bar_w * 0.62]

    for sy in range(s):
        for sx in range(s):
            px, py = sx + 0.5, sy + 0.5
            if not rounded_rect_hit(0, 0, s, s, bg_r, px, py):
                continue
            color = BG
            for (bx, by), bw in zip(bars, bar_widths):
                if rounded_rect_hit(bx, by, bw, bar_h, bar_r, px, py):
                    color = FG
                    break
            out = acc[sy // SUPERSAMPLE][sx // SUPERSAMPLE]
            out[0] += color[0]
            out[1] += color[1]
            out[2] += color[2]
            out[3] += color[3]

    n = SUPERSAMPLE * SUPERSAMPLE
    rows = []
    for row in acc:
        rows.append([(r // n, g // n, b // n, a // n) for (r, g, b, a) in row])
    return rows


# ---------------------------------------------------------------- ghi PNG

def png_bytes(rows: list[list[tuple[int, int, int, int]]]) -> bytes:
    height = len(rows)
    width = len(rows[0])

    raw = bytearray()
    for row in rows:
        raw.append(0)  # filter type 0
        for (r, g, b, a) in row:
            raw += bytes((r, g, b, a))

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


# ---------------------------------------------------------------- ghi ICO

def ico_bytes(sizes: list[int]) -> bytes:
    images = [png_bytes(render(s)) for s in sizes]

    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)

    entries = bytearray()
    for size, data in zip(sizes, images):
        dim = 0 if size >= 256 else size
        entries += struct.pack(
            "<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset
        )
        offset += len(data)

    return header + bytes(entries) + b"".join(images)


# ---------------------------------------------------------------- main

def main() -> int:
    out_dir = find_icons_dir()
    os.makedirs(out_dir, exist_ok=True)

    pngs = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square150x150Logo.png": 150,
        "Square44x44Logo.png": 44,
        "StoreLogo.png": 50,
    }
    for name, size in pngs.items():
        path = os.path.join(out_dir, name)
        with open(path, "wb") as f:
            f.write(png_bytes(render(size)))
        print(f"  {name:26} {size}x{size}")

    ico_sizes = [16, 32, 48, 64, 128, 256]
    path = os.path.join(out_dir, "icon.ico")
    with open(path, "wb") as f:
        f.write(ico_bytes(ico_sizes))
    print(f"  {'icon.ico':26} {', '.join(str(s) for s in ico_sizes)}")

    print(f"\n-> {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
