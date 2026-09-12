# -*- coding: utf-8 -*-
"""Unit tests for the intelligent pet sprite processor."""

from pathlib import Path
import numpy as np
from PIL import Image

from src.readmd_modules.pet.sprite_processor import (
    STANDARD_CELL_WIDTH,
    STANDARD_CELL_HEIGHT,
    DEFAULT_GRID_COLS,
    DEFAULT_GRID_ROWS,
    detect_foreground_mask,
    segment_poses,
    cluster_action_rows,
    normalize_and_segment_spritesheet,
)


def test_normalize_and_segment_hermes_eliminates_all_scraps():
    """Verify that orig_hermes (which had overlapping staff, head, and cross-border artifacts)

    is cleanly segmented into 8 frames across 2 coherent action rows with 0 boundary touching pixels.
    """
    orig_path = Path("scratch/orig_hermes.png")
    # The fixture is a large local asset kept out of the repository; the
    # normalization pipeline itself is covered by the synthetic tests below.
    if not orig_path.is_file():
        import pytest

        pytest.skip("scratch/orig_hermes.png fixture not present on this machine")

    raw_bytes = orig_path.read_bytes()
    normalized_bytes = normalize_and_segment_spritesheet(raw_bytes)

    assert normalized_bytes.startswith(b"\x89PNG\r\n\x1a\n")

    import io
    norm_img = Image.open(io.BytesIO(normalized_bytes))
    assert norm_img.size == (DEFAULT_GRID_COLS * STANDARD_CELL_WIDTH, DEFAULT_GRID_ROWS * STANDARD_CELL_HEIGHT)

    arr = np.array(norm_img)
    alpha = arr[:, :, 3]

    # 1. Verify internal grid boundaries have 0 touching pixels
    for c in range(1, DEFAULT_GRID_COLS):
        x = c * STANDARD_CELL_WIDTH
        touch = np.count_nonzero(alpha[:, x - 1 : x + 1] > 10)
        assert touch == 0, f"Boundary x={x} has {touch} touching pixels (edge scraps detected)"

    for r in range(1, DEFAULT_GRID_ROWS):
        y = r * STANDARD_CELL_HEIGHT
        touch = np.count_nonzero(alpha[y - 1 : y + 1, :] > 10)
        assert touch == 0, f"Boundary y={y} has {touch} touching pixels (edge scraps detected)"

    # 2. Verify each cell's outer 1-pixel perimeter has 0 non-transparent pixels
    for r in range(DEFAULT_GRID_ROWS):
        for c in range(DEFAULT_GRID_COLS):
            x0, y0 = c * STANDARD_CELL_WIDTH, r * STANDARD_CELL_HEIGHT
            cell = alpha[y0 : y0 + STANDARD_CELL_HEIGHT, x0 : x0 + STANDARD_CELL_WIDTH]
            assert np.count_nonzero(cell[0, :] > 10) == 0, f"Cell ({r}, {c}) touches top border"
            assert np.count_nonzero(cell[-1, :] > 10) == 0, f"Cell ({r}, {c}) touches bottom border"
            assert np.count_nonzero(cell[:, 0] > 10) == 0, f"Cell ({r}, {c}) touches left border"
            assert np.count_nonzero(cell[:, -1] > 10) == 0, f"Cell ({r}, {c}) touches right border"

            # Each cell must have non-empty character figure
            assert np.count_nonzero(cell > 10) > 1000, f"Cell ({r}, {c}) must contain character"


def test_normalize_fallback_on_invalid_or_stub_bytes():
    stub = b"\x89PNG\r\n\x1a\nstub-not-image"
    result = normalize_and_segment_spritesheet(stub)
    assert result == stub


def test_solid_background_detection_and_segmentation():
    """Create a synthetic sprite sheet with solid white background."""
    img = Image.new("RGB", (800, 400), (255, 255, 255))
    # Draw two blue boxes as "characters"
    arr = np.array(img)
    arr[50:150, 100:200] = [0, 100, 200]
    arr[50:150, 500:600] = [0, 100, 200]
    test_img = Image.fromarray(arr)

    import io
    buf = io.BytesIO()
    test_img.save(buf, format="PNG")

    res = normalize_and_segment_spritesheet(buf.getvalue())
    res_img = Image.open(io.BytesIO(res))
    assert res_img.size == (DEFAULT_GRID_COLS * STANDARD_CELL_WIDTH, DEFAULT_GRID_ROWS * STANDARD_CELL_HEIGHT)

    res_arr = np.array(res_img)
    # Check that background is transparent
    assert res_arr[0, 0, 3] == 0
