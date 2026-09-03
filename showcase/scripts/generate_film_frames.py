"""Extract scroll-friendly WebP frames from the finalized product film."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path

from PIL import Image


def main() -> int:
    parser = argparse.ArgumentParser()
    root = Path(__file__).resolve().parents[2]
    parser.add_argument("film", type=Path, nargs="?", default=root / "website/public/media/readmd-product-journey.mp4")
    parser.add_argument("--output", type=Path, default=root / "website/public/media/journey-frames")
    parser.add_argument("--fps", type=int, default=10)
    parser.add_argument("--target-frames", type=int, default=57)
    args = parser.parse_args()
    film = args.film if args.film.is_absolute() else root / args.film
    output = args.output if args.output.is_absolute() else root / args.output

    output.mkdir(parents=True, exist_ok=True)
    for old in output.glob("frame-*.webp"):
        old.unlink()

    with tempfile.TemporaryDirectory(prefix="readmd-film-") as temporary:
        temporary_path = Path(temporary)
        subprocess.run(
            [
                "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
                "-i", str(film),
                "-vf", f"fps={args.fps},scale=1280:720",
                str(temporary_path / "frame-%04d.png"),
            ],
            check=True,
        )
        sources = sorted(temporary_path.glob("frame-*.png"))
        if not sources:
            raise RuntimeError("FFmpeg produced no film frames")

        if args.target_frames and len(sources) != args.target_frames:
            n = len(sources)
            selected_indices = [int(round(i * (n - 1) / (args.target_frames - 1))) for i in range(args.target_frames)]
            selected_sources = [sources[i] for i in selected_indices]
        else:
            selected_sources = sources

        for i, source in enumerate(selected_sources, 1):
            frame_name = f"frame-{i:03d}.webp"
            with Image.open(source) as image:
                image.convert("RGB").save(
                    output / frame_name,
                    "WEBP",
                    quality=72,
                    method=5,
                )

    frames = sorted(output.glob("frame-*.webp"))
    if len(frames) != args.target_frames:
        raise RuntimeError(f"Expected {args.target_frames} frames, got {len(frames)}")
    print(f"Generated {len(frames)} scroll film frames")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
