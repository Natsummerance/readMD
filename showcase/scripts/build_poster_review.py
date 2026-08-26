#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Build a reviewer PDF and immutable approval request for poster batches."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import zipfile
from datetime import datetime, timezone
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _font(size: int, *, bold: bool = False) -> ImageFont.FreeTypeFont:
    path = Path("C:/Windows/Fonts/msyhbd.ttc" if bold else "C:/Windows/Fonts/msyh.ttc")
    return ImageFont.truetype(str(path), size=size)


def _wrapped(draw: ImageDraw.ImageDraw, text: str, width: int, font: ImageFont.FreeTypeFont) -> list[str]:
    lines: list[str] = []
    for paragraph in str(text).splitlines():
        current = ""
        for token in paragraph:
            candidate = current + token
            if draw.textbbox((0, 0), candidate, font=font)[2] <= width or not current:
                current = candidate
            else:
                lines.append(current)
                current = token
        lines.append(current)
    return lines


def _text_page(
    *,
    eyebrow: str,
    title: str,
    subtitle: str,
    rows: list[tuple[str, str]],
    note: str,
) -> Image.Image:
    page = Image.new("RGB", (1080, 1440), "#f4f5f7")
    draw = ImageDraw.Draw(page)
    draw.rectangle((64, 64, 1016, 1376), fill="#ffffff", outline="#d9dee5", width=2)
    draw.rectangle((64, 64, 1016, 76), fill="#d6482c")
    cursor = 148
    eyebrow_font = _font(24)
    title_font = _font(68, bold=True)
    subtitle_font = _font(32)
    label_font = _font(24)
    value_font = _font(29)
    note_font = _font(25)
    draw.text((112, cursor), eyebrow.upper(), font=eyebrow_font, fill="#d6482c")
    cursor += 56
    for line in _wrapped(draw, title, 840, title_font):
        draw.text((112, cursor), line, font=title_font, fill="#17212b")
        cursor += int(title_font.size * 1.18)
    cursor += 20
    for line in _wrapped(draw, subtitle, 840, subtitle_font):
        draw.text((112, cursor), line, font=subtitle_font, fill="#556370")
        cursor += int(subtitle_font.size * 1.42)
    cursor += 34
    for label, value in rows:
        draw.text((112, cursor), label.upper(), font=label_font, fill="#7a8794")
        cursor += 36
        for line in _wrapped(draw, value, 832, value_font):
            draw.text((112, cursor), line, font=value_font, fill="#182029")
            cursor += 41
        cursor += 18
    note_lines = _wrapped(draw, note, 832, note_font)
    note_height = len(note_lines) * 38 + 44
    draw.rounded_rectangle((112, 1268 - note_height, 968, 1268), radius=14, fill="#f7f8fa")
    note_cursor = 1268 - note_height + 22
    for line in note_lines:
        draw.text((136, note_cursor), line, font=note_font, fill="#556370")
        note_cursor += 38
    return page


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--batch", type=Path, required=True)
    parser.add_argument("--root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--dpi", type=int, default=150)
    args = parser.parse_args()

    batch_path = args.batch.resolve()
    root = (args.root or batch_path.parent).resolve()
    batch = json.loads(batch_path.read_text(encoding="utf-8"))
    output = (args.output or batch_path.parent / "poster-review.pdf").resolve()
    packages: list[dict] = []
    blocks: list[dict] = []

    for entry in batch.get("packages", []):
        package_path = (root / str(entry["package"])).resolve()
        if not package_path.is_relative_to(root):
            raise ValueError(f"package escapes root: {package_path}")
        with zipfile.ZipFile(package_path) as archive:
            metadata = json.loads(archive.read("metadata.json"))
            story = json.loads(archive.read("story.json"))
            composition = json.loads(archive.read("composition.json"))
            by_name = {item["file"]: item for item in composition["cards"]}
            images = []
            for name in metadata["images"]:
                filename = Path(name).name
                payload = archive.read(f"images/{filename}")
                image = Image.open(io.BytesIO(payload)).convert("RGB")
                if image.size != (1080, 1440):
                    raise ValueError(f"{package_path.name}/{filename} must be 1080x1440")
                images.append(image)
        blocks.append({
            "metadata": metadata,
            "story": story,
            "composition": composition,
            "images": images,
        })

    # Reserve the cover page, then alternate one divider before each poster set.
    cursor_page = 1
    for block in blocks:
        cursor_page += 1
        block["divider_page"] = cursor_page
        block["first_poster_page"] = cursor_page + 1
        block["last_poster_page"] = cursor_page + len(block["images"])
        cursor_page += len(block["images"])

    for entry, block in zip(batch.get("packages", []), blocks):
        metadata = block["metadata"]
        story = block["story"]
        composition = block["composition"]
        images = block["images"]
        by_name = {item["file"]: item for item in composition["cards"]}
        package_path = (root / str(entry["package"])).resolve()
        packages.append({
            "release": story["release"],
            "title": metadata["title"],
            "poster_style": composition["poster_style"],
            "card_count": len(images),
            "package": package_path.name,
            "package_sha256": entry["package_sha256"],
            "divider_page": block["divider_page"],
            "first_poster_page": block["first_poster_page"],
            "last_poster_page": block["last_poster_page"],
            "cards": [
                {
                    "file": filename,
                    "role": by_name[filename]["role"],
                    "sha256": by_name[filename]["sha256"],
                }
                for filename in (Path(path).name for path in metadata["images"])
            ],
        })

    if not any(block["images"] for block in blocks):
        raise ValueError("batch has no poster pages")
    package_path_by_release = {
        str(entry["release"]): entry["package"]
        for entry in batch.get("packages", [])
    }
    cover_rows = []
    for index, block in enumerate(blocks, 1):
        cover_rows.extend((
            (f"批次 {index} / {len(blocks)}", (
                f"{block['story']['release']} · {block['composition']['poster_style']}\n"
                f"{block['metadata']['title']}\n"
                f"PDF 第 {block['first_poster_page']}-{block['last_poster_page']} 页 · "
                f"SHA256 {sha256(root / package_path_by_release[block['story']['release']])[:16]}"
            )),
        ))
    cover = _text_page(
        eyebrow="ReadMD Poster Review",
        title="小红书发布确认稿",
        subtitle=f"共 {sum(len(block['images']) for block in blocks)} 张海报 / {len(blocks)} 个发布包。请先逐页检查，再生成批准文件。",
        rows=cover_rows,
        note="本 PDF 绑定批次与 ZIP 哈希；确认后的一键发布只会提交这些已核对页面。",
    )
    pages: list[Image.Image] = [cover]
    for index, block in enumerate(blocks, 1):
        metadata = block["metadata"]
        story = block["story"]
        composition = block["composition"]
        divider = _text_page(
            eyebrow=f"Batch {index} / {len(blocks)}",
            title=story["release"],
            subtitle=composition["poster_style"],
            rows=[
                ("标题", metadata["title"]),
                ("海报页", f"PDF 第 {block['first_poster_page']}-{block['last_poster_page']} 页"),
                ("包哈希", sha256(root / package_path_by_release[story["release"]])),
            ],
            note="以下截图均来自真实运行画面；请重点检查比例、完整性、裁切和留白。",
        )
        pages.append(divider)
        pages.extend(block["images"])
    pages[0].save(
        output,
        format="PDF",
        save_all=True,
        append_images=pages[1:],
        resolution=args.dpi,
        quality=95,
    )
    request = {
        "schema_version": 1,
        "created_at": datetime.now(timezone.utc).isoformat(),
        "status": "pending_operator_review",
        "batch": batch_path.name,
        "batch_sha256": sha256(batch_path),
        "review_pdf": output.name,
        "review_pdf_sha256": sha256(output),
        "page_count": len(pages),
        "poster_page_count": sum(len(block["images"]) for block in blocks),
        "packages": packages,
    }
    request_path = output.with_suffix(".approval-request.json")
    request_path.write_text(json.dumps(request, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"ok": True, "pdf": str(output), "request": str(request_path), "pages": len(pages)}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
