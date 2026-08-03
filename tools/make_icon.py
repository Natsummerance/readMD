# -*- coding: utf-8 -*-
"""生成 ReadMD 精美多尺寸图标（纯标准库，无第三方依赖）。

设计：圆角渐变底（靛蓝→紫）+ 白色 Markdown M 徽标（右腿带向下箭头）
     + 顶部高光 + 星光点缀。
输出：assets/readmd.ico（16/24/32/48/64/128/256 多尺寸）+ assets/icon-256.png

运行：python tools/make_icon.py
"""
import math
import os
import struct
import zlib

SIZES = [16, 24, 32, 48, 64, 128, 256]
SS = 4  # 超采样倍数（抗锯齿）

TOP = (59, 110, 245)      # #3B6EF5 靛蓝
BOTTOM = (123, 63, 242)   # #7B3FF2 紫
WHITE = (255, 255, 255, 255)
SPARKLE = (255, 255, 255, 235)


def lerp(a, b, t):
    return a + (b - a) * t


def mix(c1, c2, t):
    return tuple(lerp(c1[i], c2[i], t) for i in range(3))


def seg_dist(px, py, x1, y1, x2, y2):
    vx, vy = x2 - x1, y2 - y1
    wx, wy = px - x1, py - y1
    c1 = vx * wx + vy * wy
    if c1 <= 0:
        return math.hypot(wx, wy)
    c2 = vx * vx + vy * vy
    if c2 <= c1:
        return math.hypot(px - x2, py - y2)
    t = c1 / c2
    return math.hypot(px - (x1 + t * vx), py - (y1 + t * vy))


def in_round_rect(x, y, r):
    x0, y0, x1, y1 = r, r, 1.0 - r, 1.0 - r
    if x < x0 and y < y0:
        return math.hypot(x - r, y - r) <= r
    if x > x1 and y < y0:
        return math.hypot(x - (1 - r), y - r) <= r
    if x < x0 and y > y1:
        return math.hypot(x - r, y - (1 - r)) <= r
    if x > x1 and y > y1:
        return math.hypot(x - (1 - r), y - (1 - r)) <= r
    return True


def glyph(x, y, t):
    """白色 M 徽标：左腿 + 中 V + 右腿（带向下箭头）。"""
    segs = [
        (0.30, 0.32, 0.30, 0.68),   # 左腿
        (0.30, 0.32, 0.50, 0.58),   # V 左
        (0.50, 0.58, 0.70, 0.32),   # V 右
        (0.70, 0.32, 0.70, 0.56),   # 右腿
    ]
    for (x1, y1, x2, y2) in segs:
        if seg_dist(x, y, x1, y1, x2, y2) <= t:
            return True
    # 箭头：向下三角形（顶点 0.70,0.76；底边 0.55..0.85 于 y=0.60）
    ax, ay = 0.70, 0.76
    bx1, by1, bx2, by2 = 0.55, 0.60, 0.85, 0.60
    # 点在三角形内（用重心法）
    d1 = (x - bx2) * (ay - by2) - (ax - bx2) * (y - by2)
    d2 = (bx1 - bx2) * (ay - by2) - (ax - bx2) * (by1 - by2)
    d3 = (x - bx1) * (ay - by1) - (ax - bx1) * (y - by1)
    d4 = (bx2 - bx1) * (ay - by1) - (ax - bx1) * (by2 - by1)
    if d2 != 0 and d4 != 0:
        in_tri = (d1 / d2 >= 0 and d3 / d4 >= 0) and \
                 (d1 / d2 + d3 / d4 <= 1)
        if in_tri:
            return True
    return False


def pixel(x, y):
    """x,y 归一化到 [0,1]。"""
    if not in_round_rect(x, y, 0.09):
        return (0, 0, 0, 0)
    # 渐变背景
    c = mix(TOP, BOTTOM, y)
    # 顶部高光
    hl = max(0.0, 1.0 - math.hypot(x - 0.30, y - 0.18) / 0.9)
    c = mix(c, (255, 255, 255), hl * 0.16)
    # 边缘轻微压暗（立体感）
    edge = max(0.0, 1.0 - math.hypot(x - 0.5, y - 0.5) / 0.75)
    c = mix(c, (20, 30, 70), edge * 0.10)
    # 徽标
    if glyph(x, y, 0.042):
        return WHITE
    # 星光
    for (sx, sy, sr) in ((0.16, 0.22, 0.030), (0.85, 0.28, 0.022), (0.80, 0.78, 0.018)):
        if math.hypot(x - sx, y - sy) <= sr:
            return SPARKLE
    return (int(c[0]), int(c[1]), int(c[2]), 255)


def render(size):
    """以 SS 倍超采样渲染指定尺寸的 RGBA 像素。"""
    N = size * SS
    raw = bytearray()
    for py in range(size):
        for px in range(size):
            r = g = b = a = 0
            for sy in range(SS):
                for sx in range(SS):
                    xx = (px * SS + sx + 0.5) / N
                    yy = (py * SS + sy + 0.5) / N
                    rp = pixel(xx, yy)
                    r += rp[0]; g += rp[1]; b += rp[2]; a += rp[3]
            n = SS * SS
            raw += bytes((r // n, g // n, b // n, a // n))
    return bytes(raw)


def make_png(size, raw):
    def chunk(tag, data):
        c = struct.pack('>I', len(data)) + tag + data
        return c + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)

    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)
    rows = bytearray()
    stride = size * 4
    for y in range(size):
        rows.append(0)
        rows += raw[y * stride:(y + 1) * stride]
    idat = zlib.compress(bytes(rows), 9)
    return sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')


def make_ico():
    pngs = []
    for size in SIZES:
        raw = render(size)
        pngs.append((size, make_png(size, raw)))
    header = struct.pack('<HHH', 0, 1, len(pngs))
    entries = b''
    offset = 6 + 16 * len(pngs)
    for size, png in pngs:
        w = 0 if size >= 256 else size
        h = 0 if size >= 256 else size
        entries += struct.pack('<BBBBHHII', w, h, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
    return header + entries + b''.join(p for _, p in pngs)


def main():
    base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ico = os.path.join(base, 'assets', 'readmd.ico')
    png = os.path.join(base, 'assets', 'icon-256.png')
    with open(ico, 'wb') as f:
        f.write(make_ico())
    with open(png, 'wb') as f:
        f.write(make_png(256, render(256)))
    print('icon written: %s (%d bytes), %s (%d bytes)' %
          (ico, os.path.getsize(ico), png, os.path.getsize(png)))


if __name__ == '__main__':
    main()