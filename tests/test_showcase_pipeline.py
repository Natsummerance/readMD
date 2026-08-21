# -*- coding: utf-8 -*-
"""Product showcase pipeline contract tests."""
from __future__ import annotations

import hashlib
import importlib
import json
import re
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from types import SimpleNamespace

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "showcase" / "scripts"))

build_story = importlib.import_module("build_story")
validate_package = importlib.import_module("validate_package")
watch_and_publish = importlib.import_module("watch_and_publish")
write_copy = importlib.import_module("write_copy")


def write_story(root: Path) -> dict:
    png = root / "overview-reader.png"
    Image.new("RGB", (32, 20), "#101828").save(png)
    digest = hashlib.sha256(png.read_bytes()).hexdigest()
    return {
        "schema_version": 1,
        "release": "v2.3.7-beta.3",
        "previous_release": "v2.3.7-beta.2",
        "version_state": "prerelease",
        "angle": "ReadMD 正在从 Markdown 阅读器变成完整本地文档工作台",
        "selected_shots": ["overview.reader", "overview.editor"],
        "shots": [
            {
                "id": "overview.reader",
                "file": "overview-reader.png",
                "role": "pure_ui_hero",
                "caption": "ReadMD 主界面",
                "sha256": digest,
                "evidence": ["README.md"],
            },
            {
                "id": "overview.editor",
                "file": "raw/overview-editor.png",
                "role": "annotated_ui",
                "caption": "编辑预览",
                "sha256": hashlib.sha256(b"editor").hexdigest(),
                "evidence": ["README.md"],
            },
        ],
        "claims": [
            {
                "id": "reader",
                "user_value": "打开文档立刻看到完整排版",
                "shot_ids": ["overview.reader"],
                "sources": ["release/release_notes.md"],
            }
        ],
        "card_plan": [
            {"index": 1, "file": "xhs-01-cover.jpg", "role": "cover", "shot_id": None, "ui_min_ratio": 0.0},
            {"index": 2, "file": "xhs-02-overview.jpg", "role": "pure_ui_hero", "shot_id": "overview.reader", "ui_min_ratio": 0.7},
            {"index": 3, "file": "xhs-03-editor.jpg", "role": "annotated_ui", "shot_id": None, "ui_min_ratio": 0.55},
            {"index": 4, "file": "xhs-04-summary.jpg", "role": "summary", "shot_id": None, "ui_min_ratio": 0.3},
        ],
    }


def write_image(path: Path, background: tuple[int, int, int] = (14, 22, 48)) -> None:
    img = Image.new("RGB", (1080, 1440), background)
    for y in range(72, 240):
        for x in range(76, 1004):
            img.putpixel((x, y), (232, 255, 0))
    for y in range(360, 1160):
        for x in range(76, 1004):
            img.putpixel((x, y), (80, 110, 160))
    for y in range(1280, 1400):
        for x in range(76, 1004):
            img.putpixel((x, y), (232, 255, 0))
    img.save(path, "JPEG", quality=92)


class BuildStoryTest(unittest.TestCase):
    def test_release_notes_select_stable_hero_and_relevant_shots(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            notes = out / "notes.md"
            diff = out / "diff.patch"
            notes.write_text(
                "# v2.3.7-beta.3\n\n- Reveal.js 演说模式工具栏重构\n"
                "- Diagram Picker 支持科学图表\n- 更新占位符修复\n",
                encoding="utf-8",
            )
            diff.write_text(
                "+++ b/assets/js/features/export.js\n+++ b/assets/js/features/share.js\n",
                encoding="utf-8",
            )
            story = build_story.build_story(
                release="v2.3.7-beta.3",
                previous_release="v2.3.7-beta.2",
                notes=notes.read_text(encoding="utf-8"),
                diff=diff.read_text(encoding="utf-8"),
                shot_library_path=ROOT / "showcase" / "shot_library.json",
            )
        self.assertEqual(story["schema_version"], 1)
        self.assertEqual(story["version_state"], "prerelease")
        self.assertIn("overview.reader", story["selected_shots"])
        self.assertEqual(story["selected_shots"][0], "overview.reader")
        self.assertIn("presentation.reveal", story["selected_shots"])
        self.assertIn("editor.diagram-picker", story["selected_shots"])
        self.assertIn("overview.editor", story["selected_shots"])
        self.assertIn("convert.home", story["selected_shots"])
        self.assertLessEqual(len(story["selected_shots"]), 6)
        for claim in story["claims"]:
            self.assertTrue(claim["sources"])
            self.assertTrue(claim["user_value"])

    def test_chinese_invisible_fix_gets_nonempty_claim_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            notes = Path(tmp) / "notes.md"
            notes.write_text("- 纯本地稳定性修复\n", encoding="utf-8")
            story = build_story.build_story(
                release="v9.9.9",
                previous_release="v9.9.8",
                notes=notes.read_text(encoding="utf-8"),
                shot_library_path=ROOT / "showcase" / "shot_library.json",
            )
        invisible = [claim for claim in story["claims"] if claim["kind"] == "invisible"]
        self.assertTrue(invisible)
        self.assertTrue(all(claim["id"] for claim in invisible))
        self.assertTrue(all(not re.match(r"^\\d+\\.\\s+", claim["user_value"]) for claim in invisible))

    def test_release_story_filters_assets_and_prioritizes_primary_feature(self) -> None:
        notes = """
## 全平台发布资产

- Windows 安装版：`ReadMDSetup-v9.9.9.exe`
- 校验清单：`SHA256SUMS.txt`

## 本次版本核心修复与优化

### 1. 检查更新提示占位符裸露修复

### 2. 演讲演示深度优化与自定义支持

### 3. 全球语言与自动化测试覆盖率公告
"""
        story = build_story.build_story(
            release="v9.9.9",
            previous_release="v9.9.8",
            notes=notes,
            shot_library_path=ROOT / "showcase" / "shot_library.json",
        )
        claim_text = json.dumps(story["claims"], ensure_ascii=False)
        self.assertNotIn(".exe", claim_text)
        self.assertNotIn("SHA256SUMS", claim_text)
        self.assertNotIn("自动化测试覆盖率", claim_text)
        self.assertEqual(story["selected_shots"][0], "overview.reader")
        self.assertEqual(story["selected_shots"][1], "presentation.reveal")
        self.assertEqual(story["primary_shot"], "presentation.reveal")


class WriteCopyTest(unittest.TestCase):
    def test_title_candidates_cover_five_traceable_formulas(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v2.3.7-beta.2",
            )
        candidates = result["title_candidates"]
        self.assertGreaterEqual(len(candidates), 5)
        self.assertLessEqual(len({item["formula_id"] for item in candidates}), len(candidates))
        for item in candidates:
            self.assertLessEqual(len(item["text"]), 20)
            self.assertRegex(item["formula_id"], r"^#\d+$")
        self.assertLessEqual(len(result["title"]), 20)

    def test_generates_compliant_prerelease_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            story = write_story(out)
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v2.3.7-beta.2",
            )
        self.assertLessEqual(len(result["title"]), 20)
        self.assertTrue(result["title_formula_id"])
        self.assertEqual(len(result["topics"]), 5)
        self.assertTrue(all(not topic.startswith("#") for topic in result["topics"]))
        self.assertGreaterEqual(len(result["body"]), 600)
        self.assertLessEqual(len(result["body"]), 900)
        self.assertNotIn("正式发布", result["body"])
        self.assertRegex(result["body"], "预览版|更新线")
        self.assertNotRegex(result["body"], r"https?://|www\.|公众号|微信|二维码")
        self.assertIn("Natsummerance/readMD", result["body"])
        self.assertIn("source_urls", result)
        self.assertNotIn("对应画面不是概念图", result["body"])
        self.assertNotIn(".exe", result["body"])
        self.assertNotIn("ReadMDSetup", result["body"])
        self.assertEqual(result["body"].count("预览版"), 1)
        self.assertIn("Markdown 直接放映", result["body"])

    def test_release_copy_uses_formal_wording(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            story["version_state"] = "release"
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v2.3.7-beta.2",
            )
        self.assertNotRegex(result["body"], "预览版|更新线")
        self.assertIn("正式版", result["body"])


class ValidatePackageTest(unittest.TestCase):
    maxDiff = None

    def test_accepts_complete_four_image_package(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            (pkg / "raw").mkdir()
            (pkg / "images").mkdir()
            story = write_story(pkg / "raw")
            story["shots"][0]["file"] = "raw/overview-reader.png"
            editor_png = pkg / "raw" / "overview-editor.png"
            editor_png.write_bytes(b"editor")
            (pkg / "raw" / "capture.json").write_text(
                json.dumps({"schema_version": 1, "shots": [
                    {"shot_id": "overview.reader", "file": "raw/overview-reader.png", "sha256": story["shots"][0]["sha256"]},
                    {"shot_id": "overview.editor", "file": "raw/overview-editor.png", "sha256": story["shots"][1]["sha256"]},
                ]}, ensure_ascii=False),
                encoding="utf-8",
            )
            (pkg / "story.json").write_text(json.dumps(story, ensure_ascii=False), encoding="utf-8")
            names = ["xhs-01-cover.jpg", "xhs-02-overview.jpg", "xhs-03-editor.jpg", "xhs-04-summary.jpg"]
            roles = ["cover", "pure_ui_hero", "annotated_ui", "summary"]
            for index, (name, role) in enumerate(zip(names, roles)):
                write_image(pkg / "images" / name, (14 + index * 11, 22, 48))
                story["shots"].append(
                    {"id": role, "file": f"raw/{name}", "role": role, "caption": role, "sha256": "x", "evidence": []}
                )
            story["shots"] = story["shots"][:1]
            metadata = {
                "title": "ReadMD更新：文档变工作台",
                "body": "预览版。" + "这是一个用于验证包结构的最短正文。" * 40,
                "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
                "images": [str(pkg / "images" / name) for name in names],
                "source_urls": ["https://github.com/Natsummerance/readMD/releases"],
                "version_state": "prerelease",
            }
            (pkg / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            composition = {
                "overflow_errors": [],
                "cards": [
                    {"file": "xhs-01-cover.jpg", "ui_min_ratio": 0.0},
                    {"file": "xhs-02-overview.jpg", "ui_min_ratio": 0.7, "ui_area_ratio": 0.72},
                    {"file": "xhs-03-editor.jpg", "ui_min_ratio": 0.55, "ui_area_ratio": 0.58},
                    {"file": "xhs-04-summary.jpg", "ui_min_ratio": 0.3, "ui_area_ratio": 0.34},
                ],
            }
            (pkg / "composition.json").write_text(json.dumps(composition, ensure_ascii=False), encoding="utf-8")
            errors = validate_package.validate_package(pkg)
        self.assertEqual(errors, [])

    def test_rejects_missing_evidence_and_bad_hash(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            (pkg / "raw").mkdir()
            (pkg / "images").mkdir()
            story = write_story(pkg)
            story["shots"][0]["sha256"] = "bad"
            good = hashlib.sha256((pkg / "overview-reader.png").read_bytes()).hexdigest()
            (pkg / "raw" / "overview-reader.png").write_bytes((pkg / "overview-reader.png").read_bytes())
            (pkg / "raw" / "capture.json").write_text(
                json.dumps({"schema_version": 1, "shots": [{"shot_id": "overview.reader", "file": "raw/overview-reader.png", "sha256": good}]}, ensure_ascii=False),
                encoding="utf-8",
            )
            story["claims"][0]["sources"] = []
            (pkg / "story.json").write_text(json.dumps(story, ensure_ascii=False), encoding="utf-8")
            (pkg / "metadata.json").write_text("{}", encoding="utf-8")
            (pkg / "composition.json").write_text("{}", encoding="utf-8")
            errors = validate_package.validate_package(pkg)
        self.assertTrue(any("SHA-256" in error for error in errors), errors)
        self.assertTrue(any("evidence" in error for error in errors))


class WatcherTest(unittest.TestCase):
    def make_package_zip(self, root: Path) -> Path:
        package = root / "package"
        (package / "images").mkdir(parents=True)
        image = package / "images" / "xhs-01-cover.jpg"
        Image.new("RGB", (10, 10)).save(image)
        (package / "story.json").write_text(json.dumps({"release": "v1.2.3"}), encoding="utf-8")
        (package / "qa.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "title.txt").write_text("标题", encoding="utf-8")
        (package / "body.txt").write_text("正文", encoding="utf-8")
        metadata = {
            "title": "标题",
            "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
            "images": ["Z:/remote/package/images/xhs-01-cover.jpg"],
        }
        (package / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
        zip_path = root / "package.zip"
        with zipfile.ZipFile(zip_path, "w") as archive:
            for file in package.rglob("*"):
                if file.is_file():
                    archive.write(file, file.relative_to(package).as_posix())
        return zip_path

    def test_rejects_zip_traversal_and_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = root / "unsafe.zip"
            traversal = zipfile.ZipInfo("../outside.txt")
            symlink = zipfile.ZipInfo("link")
            symlink.external_attr = 0xA000 << 16
            with zipfile.ZipFile(zip_path, "w") as archive:
                archive.writestr(traversal, "bad")
                archive.writestr(symlink, "bad")
            with self.assertRaises(ValueError):
                watch_and_publish.safe_extract(zip_path, root / "destination")

    def test_localize_paths_and_build_draft_command(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            (package / "images").mkdir()
            image = package / "images" / "xhs-01-cover.jpg"
            image.write_bytes(b"jpg")
            metadata = {
                "title": "标题",
                "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
                "images": ["/remote/xhs-01-cover.jpg"],
            }
            (package / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            watch_and_publish.localize_image_paths(package)
            command = watch_and_publish.publish_command(Path("publisher.py"), package, draft=True)
            loaded = json.loads((package / "metadata.json").read_text(encoding="utf-8"))
            expected_image = str(image.resolve())
        self.assertEqual(loaded["images"], [str(image.resolve())])
        self.assertEqual(loaded["images"], [expected_image])
        self.assertIn("--no-publish", command)
        self.assertEqual(command.count("--image"), 0)

    def test_full_publish_records_status_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                payload = (
                    {"published": True, "noteId": "note-1", "url": "https://example.com/note"}
                    if command[2] == "publish"
                    else {"status": "审核中"}
                )
                return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False
                )
            finally:
                watch_and_publish.subprocess.run = original_run
            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertTrue(published)
            self.assertEqual([call[2] for call in calls], ["publish", "status"])
            self.assertEqual(record["release"], "v1.2.3")
            self.assertEqual(record["title"], "标题")
            self.assertEqual(record["note_id"], "note-1")
            self.assertEqual(record["published_url"], "https://example.com/note")
            self.assertEqual(record["audit_status"], "审核中")


if __name__ == "__main__":
    unittest.main()
