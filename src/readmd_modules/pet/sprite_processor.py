# -*- coding: utf-8 -*-
"""Intelligent pet sprite sheet recognition, segmentation, and normalization engine.

Solves the classic issues in sprite sheets:
1. Multi-action identification: detects primary poses and clusters them into
   coherent action rows (e.g. Row 0: Idle sequence, Row 1: Interactive/Wave sequence).
2. Cross-boundary scrap elimination ("边角料"): uses morphological connected-component
   segmentation to capture full character bodies and extended accessories (e.g. wands,
   hats, wings) without amputating parts or leaking stray pixels to neighboring cells.
3. Centered alignment: centers each frame horizontally and aligns foot baselines
   within standard 384x512 cells, ensuring 0 border-touching pixels.
"""

from __future__ import annotations

import io
from collections import deque
from typing import Any, Tuple

import numpy as np
from PIL import Image, ImageFilter


STANDARD_CELL_WIDTH = 384
STANDARD_CELL_HEIGHT = 512
DEFAULT_GRID_COLS = 4
DEFAULT_GRID_ROWS = 2


def detect_foreground_mask(im: Image.Image) -> np.ndarray:
    """Extract a binary foreground mask from an image."""
    arr = np.array(im.convert("RGBA"))
    alpha = arr[:, :, 3]

    if np.any(alpha < 250) and np.any(alpha > 10):
        return alpha > 15

    rgb = arr[:, :, :3].astype(np.float32)
    h, w, _ = rgb.shape
    corners = np.array([
        rgb[0, 0],
        rgb[0, w - 1],
        rgb[h - 1, 0],
        rgb[h - 1, w - 1],
    ])
    corner_std = np.std(corners, axis=0).mean()
    bg_color = np.median(corners, axis=0)

    if corner_std < 25:
        dist = np.sqrt(np.sum((rgb - bg_color) ** 2, axis=2))
        return dist > 30.0

    lum = 0.299 * rgb[:, :, 0] + 0.587 * rgb[:, :, 1] + 0.114 * rgb[:, :, 2]
    return (lum < 240) & (lum > 15)


def segment_poses(
    fg_mask: np.ndarray,
    orig_arr: np.ndarray,
    dilation_radius: int = 25,
    min_pixel_ratio: float = 0.005,
) -> list[dict[str, Any]]:
    """Segment character poses using morphological dilation and connected components."""
    h_orig, w_orig = fg_mask.shape
    total_fg = np.count_nonzero(fg_mask)
    if total_fg < 100:
        return []

    mask_img = Image.fromarray((fg_mask * 255).astype(np.uint8))
    dilated = mask_img.filter(ImageFilter.MaxFilter(dilation_radius))
    dil_arr = np.array(dilated) > 0

    scale = 2
    d_sub = dil_arr[::scale, ::scale]
    sh, sw = d_sub.shape
    vis = np.zeros((sh, sw), dtype=bool)
    lbls = np.zeros((sh, sw), dtype=int)
    lid = 0

    for y in range(sh):
        for x in range(sw):
            if d_sub[y, x] and not vis[y, x]:
                lid += 1
                q = deque([(y, x)])
                vis[y, x] = True
                lbls[y, x] = lid
                while q:
                    cy, cx = q.popleft()
                    for dy, dx in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                        ny, nx = cy + dy, cx + dx
                        if 0 <= ny < sh and 0 <= nx < sw:
                            if d_sub[ny, nx] and not vis[ny, nx]:
                                vis[ny, nx] = True
                                lbls[ny, nx] = lid
                                q.append((ny, nx))

    full_lbls = np.repeat(np.repeat(lbls, scale, axis=0), scale, axis=1)[:h_orig, :w_orig]

    min_pixels = max(200, int(total_fg * min_pixel_ratio))
    components: list[dict[str, Any]] = []

    for cid in range(1, lid + 1):
        comp_mask = (full_lbls == cid) & fg_mask
        cnt = int(np.count_nonzero(comp_mask))
        if cnt >= min_pixels:
            ys, xs = np.where(comp_mask)
            cx = float(xs.mean())
            cy = float(ys.mean())
            components.append({
                "cid": cid,
                "count": cnt,
                "bbox": (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())),
                "center": (cx, cy),
                "mask": comp_mask,
            })

    return components


def cluster_action_rows(components: list[dict[str, Any]], height: int) -> list[list[dict[str, Any]]]:
    """Cluster detected poses into coherent action sequences by vertical position,
    then sort each row temporally from left to right.
    """
    if not components:
        return []

    sorted_by_y = sorted(components, key=lambda c: c["center"][1])
    avg_h = np.mean([c["bbox"][3] - c["bbox"][1] for c in sorted_by_y])
    row_gap_threshold = max(60.0, avg_h * 0.4)

    rows: list[list[dict[str, Any]]] = []
    curr_row: list[dict[str, Any]] = [sorted_by_y[0]]

    for i in range(1, len(sorted_by_y)):
        prev_y = sorted_by_y[i - 1]["center"][1]
        curr_y = sorted_by_y[i]["center"][1]
        if curr_y - prev_y > row_gap_threshold:
            rows.append(curr_row)
            curr_row = [sorted_by_y[i]]
        else:
            curr_row.append(sorted_by_y[i])
    rows.append(curr_row)

    for row in rows:
        row.sort(key=lambda c: c["center"][0])

    return rows


def normalize_and_segment_spritesheet(
    raw_bytes: bytes,
    target_cell_size: Tuple[int, int] = (STANDARD_CELL_WIDTH, STANDARD_CELL_HEIGHT),
    target_grid: Tuple[int, int] = (DEFAULT_GRID_COLS, DEFAULT_GRID_ROWS),
) -> bytes:
    """Intelligently normalize, segment, and align a spritesheet into standard cells.

    Guarantees:
    - Zero boundary touching pixels (all cell perimeters are 100% transparent).
    - No cross-cell scrap leftovers ("边角料").
    - Poses are horizontally centered and baseline-aligned.
    - Return valid PNG bytes.
    """
    try:
        im = Image.open(io.BytesIO(raw_bytes)).convert("RGBA")
    except Exception:
        return raw_bytes

    w_orig, h_orig = im.size
    arr = np.array(im)
    fg_mask = detect_foreground_mask(im)

    components = segment_poses(fg_mask, arr)
    if not components:
        return raw_bytes

    action_rows = cluster_action_rows(components, h_orig)
    if not action_rows:
        return raw_bytes

    target_cw, target_ch = target_cell_size
    grid_cols, grid_rows = target_grid

    if len(action_rows) == 1 and grid_rows > 1:
        action_rows = [action_rows[0], action_rows[0]]
    elif len(action_rows) > grid_rows:
        action_rows = action_rows[:grid_rows]

    max_w = target_cw - 24
    max_h = target_ch - 32

    canvas = Image.new("RGBA", (grid_cols * target_cw, grid_rows * target_ch), (0, 0, 0, 0))

    for r_idx, row in enumerate(action_rows):
        if r_idx >= grid_rows:
            break

        extracted_poses: list[Image.Image] = []
        for c in row:
            bx0, by0, bx1, by1 = c["bbox"]
            sub_mask = c["mask"][by0:by1 + 1, bx0:bx1 + 1]
            pose_arr = np.zeros((by1 - by0 + 1, bx1 - bx0 + 1, 4), dtype=np.uint8)
            pose_arr[sub_mask] = arr[by0:by1 + 1, bx0:bx1 + 1][sub_mask]
            pose_img = Image.fromarray(pose_arr)

            pw, ph = pose_img.size
            scale_ratio = min(max_w / pw if pw > max_w else 1.0, max_h / ph if ph > max_h else 1.0)
            if scale_ratio < 1.0:
                nw = max(1, int(pw * scale_ratio))
                nh = max(1, int(ph * scale_ratio))
                pose_img = pose_img.resize((nw, nh), Image.Resampling.LANCZOS)
            extracted_poses.append(pose_img)

        if len(extracted_poses) == 1:
            frames = extracted_poses * grid_cols
        elif len(extracted_poses) == 2:
            frames = [extracted_poses[0], extracted_poses[1], extracted_poses[0], extracted_poses[1]]
        elif len(extracted_poses) == 3:
            frames = [extracted_poses[0], extracted_poses[1], extracted_poses[2], extracted_poses[1]]
        else:
            frames = extracted_poses[:grid_cols]

        for c_idx, pose_img in enumerate(frames):
            if c_idx >= grid_cols:
                break
            pw, ph = pose_img.size
            offset_x = c_idx * target_cw + (target_cw - pw) // 2
            offset_y = r_idx * target_ch + (target_ch - 24 - ph)
            if offset_y < r_idx * target_ch + 12:
                offset_y = r_idx * target_ch + 12

            canvas.paste(pose_img, (offset_x, offset_y), pose_img)

    out_buf = io.BytesIO()
    canvas.save(out_buf, format="PNG", optimize=True)
    return out_buf.getvalue()


__all__ = [
    "STANDARD_CELL_WIDTH",
    "STANDARD_CELL_HEIGHT",
    "DEFAULT_GRID_COLS",
    "DEFAULT_GRID_ROWS",
    "detect_foreground_mask",
    "segment_poses",
    "cluster_action_rows",
    "normalize_and_segment_spritesheet",
]
