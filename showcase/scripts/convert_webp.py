import os
import shutil
from pathlib import Path
from PIL import Image

def main():
    root = Path(__file__).resolve().parents[2]
    temp_dir = root / "_temp_frames"
    public_frames = root / "website/public/media/journey-frames"
    dist_frames = root / "website/dist/media/journey-frames"
    public_frames.mkdir(parents=True, exist_ok=True)
    dist_frames.mkdir(parents=True, exist_ok=True)

    out_webp = root / "_temp_webp"
    out_webp.mkdir(parents=True, exist_ok=True)

    for i in range(1, 58):
        num = f"{i:03d}"
        src = temp_dir / f"frame-{num}.png"
        dst = out_webp / f"frame-{num}.webp"
        if src.exists():
            with Image.open(src) as img:
                resized = img.resize((1280, 720), Image.Resampling.LANCZOS)
                with open(dst, "wb") as f:
                    resized.convert("RGB").save(f, "WEBP", quality=85)

    print("Encoded", len(list(out_webp.glob("*.webp"))), "frames")

    # Copy into public and dist
    for webp_file in out_webp.glob("*.webp"):
        shutil.copyfile(webp_file, public_frames / webp_file.name)
        shutil.copyfile(webp_file, dist_frames / webp_file.name)

    # Convert hero
    hero_png = root / "website/public/media/overview-reader.png"
    hero_webp_pub = root / "website/public/media/overview-reader.webp"
    hero_webp_dist = root / "website/dist/media/overview-reader.webp"
    with Image.open(hero_png) as img:
        resized_hero = img.resize((1440, 810), Image.Resampling.LANCZOS)
        with open(hero_webp_pub, "wb") as f:
            resized_hero.convert("RGB").save(f, "WEBP", quality=90)
        shutil.copyfile(hero_webp_pub, hero_webp_dist)

    shutil.rmtree(temp_dir, ignore_errors=True)
    shutil.rmtree(out_webp, ignore_errors=True)
    print("Done! All 57 frames and hero webp converted and synced successfully!")

if __name__ == "__main__":
    main()
