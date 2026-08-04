# -*- coding: utf-8 -*-
"""生成 PyInstaller 启动画面 installer/splash.png（深色底 + 居中 ReadMD 徽标 + 柔光）。"""
import os, struct, zlib, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "tools"))
from make_icon import render

W, H = 640, 400
ICON = 256

def lerp(a, b, t):
    return a + (b - a) * t

def bg(x, y):
    # 垂直渐变 #0d1220 -> #090d17
    t = y / H
    r = int(lerp(13, 9, t)); g = int(lerp(18, 13, t)); b = int(lerp(32, 23, t))
    # 中心柔光
    dx = (x - W / 2) / (W / 2); dy = (y - H * 0.42) / (H / 2)
    d = (dx * dx + dy * dy) ** 0.5
    glow = max(0.0, 1.0 - d) ** 2 * 0.35
    r = int(r + 40 * glow); g = int(g + 70 * glow); b = int(b + 160 * glow)
    return (r, g, b)

def main():
    icon_raw = render(ICON)  # RGBA bytes, ICON*ICON
    stride = ICON * 4
    rows = bytearray()
    for y in range(H):
        rows.append(0)
        for x in range(W):
            ix = x - (W - ICON) // 2
            iy = y - (H - ICON) // 2
            if 0 <= ix < ICON and 0 <= iy < ICON:
                off = iy * stride + ix * 4
                a = icon_raw[off + 3]
                if a:
                    # 图标合成
                    f = a / 255.0
                    br, bg2, bb = bg(x, y)
                    r = int(icon_raw[off] * f + br * (1 - f))
                    g = int(icon_raw[off + 1] * f + bg2 * (1 - f))
                    b = int(icon_raw[off + 2] * f + bb * (1 - f))
                    rows += bytes((r, g, b, 255))
                else:
                    rows += bytes(bg(x, y) + (255,))
            else:
                rows += bytes(bg(x, y) + (255,))
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(rows), 9)
    png = sig + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
    out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "installer", "splash.png")
    with open(out, "wb") as f:
        f.write(png)
    print("splash written:", out, len(png), "bytes")

if __name__ == "__main__":
    main()
