# -*- coding: utf-8 -*-
"""Generate the neutral Markdown document icon used by Windows associations.

This intentionally does not modify ReadMD's application logo.  The output is
a multi-size ICO plus a PNG preview, rendered with the standard library only.
"""
import os

from make_icon import SIZES, make_png, struct

SS = 4
BLUE = (37, 99, 235, 255)


def _inside(x, y, left, top, right, bottom):
    return left <= x <= right and top <= y <= bottom


def _segment_distance(px, py, x1, y1, x2, y2):
    vx, vy = x2 - x1, y2 - y1
    wx, wy = px - x1, py - y1
    length = vx * vx + vy * vy
    if not length:
        return ((px - x1) ** 2 + (py - y1) ** 2) ** 0.5
    t = max(0.0, min(1.0, (wx * vx + wy * vy) / length))
    return ((px - x1 - t * vx) ** 2 + (py - y1 - t * vy) ** 2) ** 0.5


def _letter(x, y):
    thickness = 0.026
    segments = (
        # M
        (0.34, 0.69, 0.34, 0.84), (0.34, 0.69, 0.46, 0.79),
        (0.46, 0.79, 0.58, 0.69), (0.58, 0.69, 0.58, 0.84),
        # D
        (0.65, 0.69, 0.65, 0.84), (0.65, 0.69, 0.78, 0.69),
        (0.78, 0.69, 0.87, 0.75), (0.87, 0.75, 0.87, 0.79),
        (0.87, 0.79, 0.78, 0.84), (0.78, 0.84, 0.65, 0.84),
    )
    return any(_segment_distance(x, y, *seg) <= thickness for seg in segments)


def _pixel(x, y):
    # White document with a folded top-right corner.
    in_sheet = _inside(x, y, 0.14, 0.08, 0.80, 0.89)
    if in_sheet and x > 0.61 and y < 0.27 and y < x - 0.53:
        in_sheet = False
    # The blue MD badge slightly overlaps the sheet, like a native file-type tag.
    if _inside(x, y, 0.27, 0.63, 0.94, 0.89):
        if _letter(x, y):
            return (255, 255, 255, 255)
        return BLUE
    if not in_sheet:
        # Narrow shadow only; never paint across the document face.
        if (_inside(x, y, 0.19, 0.89, 0.84, 0.93) or
                _inside(x, y, 0.80, 0.15, 0.84, 0.89)):
            return (91, 103, 120, 55)
        return (0, 0, 0, 0)
    border = x < 0.18 or y > 0.85 or (x > 0.76 and y > 0.27)
    if border:
        return (148, 163, 184, 255)
    if x >= 0.61 and y <= 0.27:
        if abs((x - 0.61) - (0.27 - y)) < 0.035:
            return (148, 163, 184, 255)
        return (226, 232, 240, 255)
    # Three quiet text strokes give the file-document silhouette at large sizes.
    if ((0.25 <= x <= 0.65 and 0.34 <= y <= 0.37) or
            (0.25 <= x <= 0.69 and 0.44 <= y <= 0.47) or
            (0.25 <= x <= 0.55 and 0.54 <= y <= 0.57)):
        return (184, 196, 211, 255)
    return (248, 250, 252, 255)


def render(size):
    raw = bytearray()
    scale = size * SS
    for py in range(size):
        for px in range(size):
            rgba = [0, 0, 0, 0]
            for sy in range(SS):
                for sx in range(SS):
                    color = _pixel((px * SS + sx + 0.5) / scale,
                                   (py * SS + sy + 0.5) / scale)
                    for index in range(4):
                        rgba[index] += color[index]
            raw.extend(value // (SS * SS) for value in rgba)
    return bytes(raw)


def make_ico():
    images = [(size, make_png(size, render(size))) for size in SIZES]
    offset = 6 + 16 * len(images)
    entries = bytearray()
    for size, png in images:
        dimension = 0 if size >= 256 else size
        entries.extend(struct.pack('<BBBBHHII', dimension, dimension, 0, 0,
                                   1, 32, len(png), offset))
        offset += len(png)
    return (struct.pack('<HHH', 0, 1, len(images)) + bytes(entries) +
            b''.join(png for _size, png in images))


def main():
    base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    ico_path = os.path.join(base, 'assets', 'markdown-file.ico')
    png_path = os.path.join(base, 'assets', 'markdown-file-256.png')
    with open(ico_path, 'wb') as handle:
        handle.write(make_ico())
    with open(png_path, 'wb') as handle:
        handle.write(make_png(256, render(256)))
    print('Markdown file icon written: %s, %s' % (ico_path, png_path))


if __name__ == '__main__':
    main()
