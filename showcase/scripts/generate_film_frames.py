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
    parser.add_argument("--fps", type=int, default=5)
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
                str(temporary_path / "frame-%03d.png"),
            ],
            check=True,
        )
        sources = sorted(temporary_path.glob("frame-*.png"))
        if not sources:
            raise RuntimeError("FFmpeg produced no film frames")
        for source in sources:
            with Image.open(source) as image:
                image.convert("RGB").save(
                    output / f"{source.stem}.webp",
                    "WEBP",
                    quality=72,
                    method=5,
                )

    frames = sorted(output.glob("frame-*.webp"))
    if len(frames) < 24:
        raise RuntimeError(f"Too few scroll film frames: {len(frames)}")
    print(f"Generated {len(frames)} scroll film frames")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
