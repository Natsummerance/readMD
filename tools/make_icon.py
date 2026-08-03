# -*- coding: utf-8 -*-
"""生成 ReadMD 图标（assets/readmd.ico，纯标准库，无需 PIL）。

运行：python tools/make_icon.py
"""
import os
import struct
import zlib

W = H = 32


def seg_dist(px, py, x1, y1, x2, y2):
    vx, vy = x2 - x1, y2 - y1
    wx, wy = px - x1, py - y1
    c1 = vx * wx + vy * wy
    if c1 <= 0:
        return (wx * wx + wy * wy) ** 0.5
    c2 = vx * vx + vy * vy
    if c2 <= c1:
        return ((px - x2) ** 2 + (py - y2) ** 2) ** 0.5
    t = c1 / c2
    return ((px - (x1 + t * vx)) ** 2 + (py - (y1 + t * vy)) ** 2) ** 0.5


def in_rounded_rect(x, y, r=7.0):
    x0, y0, x1, y1 = 0.0, 0.0, float(W - 1), float(H - 1)
    if x < x0 + r and y < y0 + r:
        return (x - x0 - r) ** 2 + (y - y0 - r) ** 2 <= r * r
    if x > x1 - r and y < y0 + r:
        return (x - x1 + r) ** 2 + (y - y0 - r) ** 2 <= r * r
    if x < x0 + r and y > y1 - r:
        return (x - x0 - r) ** 2 + (y - y1 + r) ** 2 <= r * r
    if x > x1 - r and y > y1 - r:
        return (x - x1 + r) ** 2 + (y - y1 + r) ** 2 <= r * r
    return True


def pixel(x, y):
    if not in_rounded_rect(x, y):
        return (0, 0, 0, 0)
    # 深蓝渐变背景
    bg = (28 + int(14 * y / H), 62 + int(18 * y / H), 148 + int(20 * y / H), 255)
    # 白色 “M”
    segs = [((7, 8), (7, 23)), ((25, 8), (25, 23)),
            ((7, 8), (16, 23)), ((25, 8), (16, 23))]
    for (a, b) in segs:
        if seg_dist(x, y, a[0], a[1], b[0], b[1]) <= 2.6:
            return (255, 255, 255, 255)
    return bg


def make_png():
    raw = bytearray()
    for y in range(H):
        raw.append(0)
        for x in range(W):
            raw += bytes(pixel(x, y))

    def chunk(tag, data):
        c = struct.pack('>I', len(data)) + tag + data
        return c + struct.pack('>I', zlib.crc32(tag + data) & 0xffffffff)

    sig = b'\x89PNG\r\n\x1a\n'
    ihdr = struct.pack('>IIBBBBB', W, H, 8, 6, 0, 0, 0)
    idat = zlib.compress(bytes(raw), 9)
    return sig + chunk(b'IHDR', ihdr) + chunk(b'IDAT', idat) + chunk(b'IEND', b'')


def make_ico():
    png = make_png()
    header = struct.pack('<HHH', 0, 1, 1)
    entry = struct.pack('<BBBBHHII', 32, 32, 0, 0, 1, 32, len(png), 22)
    return header + entry + png


if __name__ == '__main__':
    out = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                       'assets', 'readmd.ico')
    with open(out, 'wb') as f:
        f.write(make_ico())
    print('icon written:', out, os.path.getsize(out), 'bytes')