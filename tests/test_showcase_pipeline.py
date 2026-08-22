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
audit_copy = importlib.import_module("audit_copy")
content_memory = importlib.import_module("content_memory")
copy_variants = importlib.import_module("copy_variants")
export_wechat = importlib.import_module("export_wechat")
performance_report = importlib.import_module("performance_report")
pattern_audit = importlib.import_module("pattern_audit")
review_dashboard = importlib.import_module("review_dashboard")
style_audit = importlib.import_module("style_audit")
build_package_module = importlib.import_module("build_package")
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
            story["selected_shots"] = ["overview.reader", "overview.editor"]
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
            (pkg / "pattern-audit.json").write_text(json.dumps({"ok": True, "errors": []}), encoding="utf-8")
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

    def test_rejects_failed_design_audit(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            (pkg / "raw").mkdir()
            (pkg / "images").mkdir()
            story = write_story(pkg / "raw")
            story["selected_shots"] = ["overview.reader", "overview.editor"]
            story["card_plan"] = [
                {"index": 1, "file": "xhs-01-cover.jpg", "role": "cover", "shot_id": None, "ui_min_ratio": 0.0},
                {"index": 2, "file": "xhs-02-overview.jpg", "role": "pure_ui_hero", "shot_id": "overview.reader", "ui_min_ratio": 0.7, "ui_area_ratio": 0.72},
                {"index": 3, "file": "xhs-03-editor.jpg", "role": "annotated_ui", "shot_id": "overview.editor", "ui_min_ratio": 0.55, "ui_area_ratio": 0.58},
                {"index": 4, "file": "xhs-04-summary.jpg", "role": "summary", "shot_id": None, "ui_min_ratio": 0.3},
            ]
            story["shots"][0]["file"] = "raw/overview-reader.png"
            story["shots"][1]["file"] = "raw/overview-editor.png"
            (pkg / "raw" / "overview-reader.png").write_bytes(b"reader")
            (pkg / "raw" / "overview-editor.png").write_bytes(b"editor")
            story["shots"][0]["sha256"] = hashlib.sha256((pkg / "raw/overview-reader.png").read_bytes()).hexdigest()
            story["shots"][1]["sha256"] = hashlib.sha256((pkg / "raw/overview-editor.png").read_bytes()).hexdigest()
            good = story["shots"][0]["sha256"]
            (pkg / "raw" / "capture.json").write_text(
                json.dumps({"schema_version": 1, "shots": [
                    {"shot_id": "overview.reader", "file": "raw/overview-reader.png", "sha256": hashlib.sha256((pkg / "raw/overview-reader.png").read_bytes()).hexdigest()},
                    {"shot_id": "overview.editor", "file": "raw/overview-editor.png", "sha256": hashlib.sha256((pkg / "raw/overview-editor.png").read_bytes()).hexdigest()},
                ]}),
                encoding="utf-8",
            )
            (pkg / "story.json").write_text(json.dumps(story, ensure_ascii=False), encoding="utf-8")
            for index, card in enumerate(story["card_plan"]):
                write_image(pkg / "images" / card["file"], (14 + index * 11, 22, 48))
            metadata = {
                "title": "标题",
                "body": "预览版。" + ("这是一段用于结构验证的正文。" * 46),
                "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
                "images": [str(pkg / "images" / card["file"]) for card in story["card_plan"]],
                "source_urls": ["https://example.com/release"],
                "version_state": "prerelease",
            }
            (pkg / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            composition = {
                "overflow_errors": [],
                "design_audit": {"contrast_errors": ["muted text"], "small_text": [], "images_failed": []},
                "cards": [{"file": card["file"], "ui_min_ratio": card["ui_min_ratio"], "ui_area_ratio": card.get("ui_area_ratio", 0.4)} for card in story["card_plan"]],
            }
            (pkg / "composition.json").write_text(json.dumps(composition, ensure_ascii=False), encoding="utf-8")
            errors = validate_package.validate_package(pkg)
        self.assertTrue(any("design audit" in error.lower() for error in errors), errors)


class BenchmarkRubricTest(unittest.TestCase):
    def test_rubric_has_balanced_weights_and_hard_gates(self) -> None:
        rubric = json.loads((ROOT / "showcase/content/benchmark-rubric.json").read_text(encoding="utf-8"))
        weights = rubric["dimensions"]
        self.assertEqual(sum(item["weight"] for item in weights.values()), 100)
        self.assertGreaterEqual(rubric["pass"]["total"], 88)
        self.assertEqual(set(rubric["pass"]["minimum_dimension"]), set(weights))
        for name, dimension in weights.items():
            self.assertTrue(dimension["criteria"], name)

    def test_pattern_library_uses_reviewed_open_sources(self) -> None:
        data = json.loads((ROOT / "showcase/content/pattern-library.json").read_text(encoding="utf-8"))
        sources = {item["id"]: item for item in data["sources"]}
        self.assertIn("xhs-visual-director", sources)
        self.assertEqual(sources["xhs-visual-director"]["license"], "MIT")
        self.assertGreaterEqual(len(data["patterns"]), 8)
        serialized = json.dumps(data, ensure_ascii=False).lower()
        for dangerous in ("curl | sh", "rm -rf", "eval(requests", "api_key="):
            self.assertNotIn(dangerous, serialized)
        for pattern in data["patterns"]:
            self.assertIn(pattern["source_id"], sources)
            self.assertTrue(pattern["mechanism"])
            self.assertTrue(pattern["application"])


class AuditCopyTest(unittest.TestCase):
    def make_audit_inputs(self, body: str) -> tuple[dict, dict, dict]:
        story = {
            "release": "v9.9.9-beta.1",
            "version_state": "prerelease",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "primary_shot": "presentation.reveal",
            "selected_shots": ["overview.reader", "presentation.reveal"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
            ],
        }
        metadata = {
            "title": "不用重做PPT，Markdown直接放映",
            "title_formula_id": "#36",
            "body": body,
            "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
            "version_state": "prerelease",
        }
        composition = {
            "overflow_errors": [],
            "design_audit": {"contrast_errors": [], "small_text": [], "images_failed": []},
            "cards": [
                {"file": "cover.jpg", "role": "cover", "ui_min_ratio": 0, "ui_area_ratio": 0.3},
                {"file": "hero.jpg", "role": "pure_ui_hero", "ui_min_ratio": 0.7, "ui_area_ratio": 0.8},
                {"file": "feature.jpg", "role": "annotated_ui", "ui_min_ratio": 0.55, "ui_area_ratio": 0.6},
                {"file": "summary.jpg", "role": "summary", "ui_min_ratio": 0.3, "ui_area_ratio": 0.4},
            ],
        }
        return story, metadata, composition

    def good_body(self) -> str:
        return (
            "文档写完了，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。\n\n"
            "先说清楚：这是 ReadMD v9.9.9-beta.1 预览版，文件仍在你自己的电脑里。\n\n"
            "放映界面能换主题、调字号、切开场和转场；AST 保护分片尽量保住代码块、表格和公式。\n\n"
            "下面的画面来自当前版本真实运行状态，不是概念图。改稿时回到同屏预览，讲稿不会跑偏。\n\n"
            "如果你要写课程讲义、组会报告或论文汇报，它会省掉重做演示稿这一步。\n\n"
            "GitHub 搜 Natsummerance/readMD，你会先拿哪一份 Markdown 试放映？\n\n"
            "渲染阶段只处理显示结果，不会替你改写原始 Markdown 文件。所有演示都来自同一个本地工作流，不需要先把文档上传到别处。对长文档来说，稳定的目录和搜索比炫技功能更重要。转换结果会开成新标签页，方便先检查再保存。界面支持跟随系统语言，中文和英文术语都保持统一。目录和全文搜索跨页联动，长文档不会因为一次渲染丢掉入口。暗色主题只影响显示，源文件内容不变。公式和图表在阅读页直接渲染，减少截图拼接。本地优先意味着草稿、笔记和讲稿都留在自己的设备里。"
        )

    def test_good_package_passes_alignment_gate(self) -> None:
        story, metadata, composition = self.make_audit_inputs(self.good_body())
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertTrue(report["ok"], json.dumps(report, ensure_ascii=False, indent=2))
        self.assertGreaterEqual(report["total_score"], 88)
        self.assertEqual(report["scores"]["compliance"], 5)

    def test_repeated_ai_fingerprint_fails_hard_gate(self) -> None:
        body = self.good_body() + "\n\n对应画面不是概念图。对应画面不是概念图。"
        story, metadata, composition = self.make_audit_inputs(body)
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertTrue(any("repeated" in item.lower() or "重复" in item for item in report["hard_failures"]))

    def test_style_audit_passes_concrete_developer_voice(self) -> None:
        report = style_audit.audit_style(self.good_body(), audience="程序员")
        self.assertGreaterEqual(report["score"], 85, json.dumps(report, ensure_ascii=False, indent=2))
        self.assertEqual(report["hard_failures"], [])

    def test_style_audit_flags_uniform_generic_copy(self) -> None:
        body = "这款工具非常强大。这款工具非常高效。这款工具非常安全。快来关注点赞。"
        report = style_audit.audit_style(body, audience="程序员")
        self.assertLess(report["score"], 75)
        self.assertTrue(any(item["id"] == "uniform-rhythm" for item in report["findings"]))
        self.assertTrue(any(item["id"] == "generic-adjective" for item in report["findings"]))
        self.assertTrue(any(item["id"] == "generic-cta" for item in report["findings"]))

    def test_semantic_qa_integrates_style_hard_gate(self) -> None:
        body = "这款工具非常强大。这款工具非常高效。这款工具非常安全。快来关注点赞。\n\n先说清楚：这是 ReadMD v9.9.9-beta.1 预览版。"
        story, metadata, composition = self.make_audit_inputs(body)
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertIn("style", report)
        self.assertLess(report["style"]["score"], 75)
        self.assertTrue(any("style" in item.lower() for item in report["hard_failures"]))


class PatternAuditTest(unittest.TestCase):
    def make_inputs(self) -> tuple[dict, dict, dict]:
        story = {
            "schema_version": 1,
            "version_state": "prerelease",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "primary_shot": "presentation.reveal",
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "card_plan": [
                {"index": 1, "file": "cover.jpg", "role": "cover", "shot_id": None, "ui_min_ratio": 0},
                {"index": 2, "file": "hero.jpg", "role": "pure_ui_hero", "shot_id": "overview.reader", "ui_min_ratio": 0.7},
                {"index": 3, "file": "reveal.jpg", "role": "annotated_ui", "shot_id": "presentation.reveal", "ui_min_ratio": 0.55},
                {"index": 4, "file": "editor.jpg", "role": "annotated_ui", "shot_id": "overview.editor", "ui_min_ratio": 0.55},
                {"index": 5, "file": "summary.jpg", "role": "summary", "shot_id": None, "ui_min_ratio": 0.3},
            ],
        }
        metadata = {
            "title": "不用重做PPT，Markdown直接放映",
            "title_formula_id": "#36",
            "body": (
                "文档写完了，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。\n\n"
                "先说清楚：这是 ReadMD 预览版，文件仍在你自己的电脑里。\n\n"
                "如果你常写课程讲义、组会报告、技术分享或论文汇报，它会省掉重新做演示稿这一步。\n\n"
                "你会先拿哪一份 Markdown 试放映？"
            ),
        }

        def card(file: str, role: str, area: float, minimum: float) -> dict:
            return {
                "file": file,
                "role": role,
                "ui_area_ratio": area,
                "ui_min_ratio": minimum,
                "screenshot_box": {"x": 24, "y": 24, "width": 900, "height": 900},
            }

        composition = {
            "overflow_errors": [],
            "design_audit": {"contrast_errors": [], "small_text": [], "images_failed": []},
            "cards": [
                card("cover.jpg", "cover", 0.35, 0),
                card("hero.jpg", "pure_ui_hero", 0.82, 0.7),
                card("reveal.jpg", "annotated_ui", 0.62, 0.55),
                card("editor.jpg", "annotated_ui", 0.60, 0.55),
                card("summary.jpg", "summary", 0.45, 0.3),
            ],
        }
        return story, metadata, composition

    def write_package(self, package: Path, story: dict, metadata: dict, composition: dict) -> None:
        package.mkdir(parents=True, exist_ok=True)
        for name, data in (
            ("story.json", story),
            ("metadata.json", metadata),
            ("composition.json", composition),
        ):
            (package / name).write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")

    def test_good_package_passes_every_reviewed_pattern(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertTrue(report["ok"], report["errors"])
        self.assertEqual(len(report["patterns"]), 10)
        self.assertTrue(all(item["ok"] for item in report["patterns"]))

    def test_broken_hero_and_generic_voice_fail_hard(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            story["card_plan"][1] = {**story["card_plan"][1], "role": "annotated_ui", "shot_id": "presentation.reveal"}
            metadata["body"] = "产品强大高效安全智能先进。" + metadata["body"]
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
            errors = validate_package.validate_package(package)
        self.assertFalse(report["ok"])
        product = next(item for item in report["patterns"] if item["id"] == "product-first-proof")
        outcome = next(item for item in report["patterns"] if item["id"] == "outcome-not-adjective")
        self.assertFalse(product["ok"])
        self.assertFalse(outcome["ok"])
        self.assertTrue(any("hot-post pattern gate failed" in error for error in errors), errors)


class ExportWechatTest(unittest.TestCase):
    def test_exports_paste_safe_inline_html(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            metadata = {
                "title": "不用重做PPT，Markdown直接放映",
                "body": (
                    "文档已经写完，讲的时候还要复制进 PPT。\n\n"
                    "这次把这一步**砍掉**：Markdown 直接放映，代码块用 `cmd=true` 标记。"
                    "\n\n放映界面能换主题、调字号、切开场和转场；AST 保护分片尽量保住代码块、表格和公式。"
                    "\n\n如果你要写课程讲义、组会报告或论文汇报，它会省掉重做演示稿这一步。"
                    "\n\nGitHub 搜 Natsummerance/readMD，你会先拿哪一份 Markdown 试放映？"
                ),
                "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
            }
            (package / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            story = {"release": "v1.2.3", "angle": "Markdown 直接放映"}
            report = export_wechat.export_package(package, story=story)
            output = package / "wechat" / "readmd-wechat.html"
            html = output.read_text(encoding="utf-8")

        self.assertTrue(report["ok"], report)
        self.assertTrue(html)
        for forbidden in ("<style", "<script", "class=", "id=", ":hover", ":before", ":after", "<img", "<table", "http://", "https://"):
            self.assertNotIn(forbidden, html.lower())
        self.assertIn("<h1 style=", html)
        self.assertIn("<strong style=", html)
        self.assertIn("<code style=", html)
        self.assertGreaterEqual(html.count("<p style="), 4)
        for paragraph in re.findall(r"<p style=\"([^\"]+)\"", html):
            self.assertIn("font-size", paragraph)
            self.assertIn("line-height", paragraph)
            self.assertIn("color", paragraph)
        self.assertIn("#GitHub #开源项目 #程序员 #效率工具 #Markdown", html)
        self.assertNotIn("PPT。<", html)

    def test_rejects_wechat_html_missing_inline_styles(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "broken.html"
            path.write_text('<!doctype html><html><body><p>缺行内样式</p></body></html>', encoding="utf-8")
            errors = export_wechat.validate_wechat_html(path)
        self.assertTrue(any("paragraph inline style" in error for error in errors))


class CopyVariantsTest(unittest.TestCase):
    def history_record(self, release: str, hook_type: str, formula: str) -> dict:
        return {
            "release": release,
            "title": "test",
            "title_formula_id": formula,
            "hook_type": hook_type,
            "published_at": "2026-08-22T00:00:00Z",
            "impressions": 1000,
            "likes": 20,
            "collects": 80,
            "comments": 30,
            "shares": 10,
            "follows": 4,
            "lessons": "verified",
        }

    def test_builds_three_distinct_evidence_backed_variants(self) -> None:
        story = {
            "release": "v1.1.0",
            "previous_release": "v1.0.0",
            "version_state": "prerelease",
            "primary_shot": "presentation.reveal",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
            ],
        }
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        self.assertEqual(len(variants), 3)
        self.assertEqual(len({item["strategy"] for item in variants}), 3)
        self.assertEqual(len({item["title"] for item in variants}), 3)
        self.assertEqual(len({item["body"] for item in variants}), 3)
        for variant in variants:
            report = audit_copy.audit_copy(
                story=story,
                metadata=variant,
                composition=copy_variants.projected_composition(story),
            )
            self.assertTrue(report["ok"], report)

    def test_selection_prefers_verified_identity_and_penalizes_recent_outcome(self) -> None:
        story = {
            "release": "v1.1.0",
            "previous_release": "v1.0.0",
            "version_state": "prerelease",
            "primary_shot": "presentation.reveal",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
            ],
        }
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        history = [
            {**self.history_record("v1.0.0", "identity-led", "#22"), "impressions": 2000, "likes": 100, "collects": 160, "comments": 40, "shares": 20},
            self.history_record("v1.0.1", "mechanism-curiosity", "#9"),
            self.history_record("v1.0.2", "outcome-led", "#36"),
        ]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertEqual(chosen["strategy"], "identity-led")
        self.assertTrue(report["ok"])

    def test_selection_ignores_pending_hook_history(self) -> None:
        story = {
            "release": "v1.1.0",
            "previous_release": "v1.0.0",
            "version_state": "prerelease",
            "primary_shot": "presentation.reveal",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
            ],
        }
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        history = [
            {**self.history_record("v1.0.1", "identity-led", "#22"), "metrics_status": "pending"},
        ]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertEqual(chosen["strategy"], "outcome-led")
        self.assertEqual(report["ranked"][0]["history_adjustment"] + report["ranked"][1]["history_adjustment"], 0)

    def variant_story(self) -> dict:
        return {
            "release": "v1.1.0",
            "previous_release": "v1.0.0",
            "version_state": "prerelease",
            "primary_shot": "presentation.reveal",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
            ],
        }

    def test_originality_gate_rejects_exact_body_collision(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        reused = next(variant for variant in variants if variant["strategy"] == "outcome-led")
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            "body_sha256": hashlib.sha256(reused["body"].encode("utf-8")).hexdigest(),
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertEqual(chosen["strategy"], "identity-led")
        self.assertFalse(next(item for item in report["ranked"] if item["strategy"] == "outcome-led")["ok"])
        self.assertTrue(any("body hash" in failure for failure in next(
            item for item in report["ranked"] if item["strategy"] == "outcome-led"
        )["hard_failures"]))

    def test_originality_gate_rejects_reused_opening_and_closing(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        outcome = next(variant for variant in variants if variant["strategy"] == "outcome-led")
        fingerprints = copy_variants.text_fingerprints(outcome["body"])
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            **fingerprints,
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertNotEqual(chosen["strategy"], "outcome-led")
        outcome_report = next(item for item in report["ranked"] if item["strategy"] == "outcome-led")
        self.assertFalse(outcome_report["ok"])
        self.assertTrue(any("opening" in failure for failure in outcome_report["hard_failures"]))
        self.assertTrue(any("closing" in failure for failure in outcome_report["hard_failures"]))

    def test_originality_gate_rejects_near_duplicate_template(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        outcome = next(variant for variant in variants if variant["strategy"] == "outcome-led")
        lightly_edited = outcome["body"].replace("讲的时候还要复制进 PPT", "讲的时候还得复制到 PPT")
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            "body_trigrams": list(copy_variants.text_trigrams(lightly_edited)),
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        outcome_report = next(item for item in report["ranked"] if item["strategy"] == "outcome-led")
        self.assertNotEqual(chosen["strategy"], "outcome-led")
        self.assertFalse(outcome_report["ok"])
        self.assertGreaterEqual(outcome_report["max_body_similarity"], 0.85)
        self.assertTrue(any("near-duplicate body" in failure for failure in outcome_report["hard_failures"]))


class PerformanceReportTest(unittest.TestCase):
    def complete(self, release: str, formula: str, hook_type: str, impressions: int, likes: int, collects: int) -> dict:
        return {
            "release": release,
            "title": release,
            "title_formula_id": formula,
            "hook_type": hook_type,
            "published_at": "2026-08-22T00:00:00Z",
            "impressions": impressions,
            "likes": likes,
            "collects": collects,
            "comments": 10,
            "shares": 5,
            "follows": 2,
            "metrics_status": "complete",
        }

    def test_generates_markdown_and_json_without_pending_learning(self) -> None:
        records = [
            self.complete("v1", "#36", "outcome-led", 1000, 40, 60),
            self.complete("v2", "#22", "identity-led", 2000, 120, 180),
            {**self.complete("v3", "#9", "mechanism-curiosity", 500, 5, 5), "metrics_status": "pending"},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp)
            result = performance_report.generate_report(records, output)
            markdown = (output / "performance-report.md").read_text(encoding="utf-8")
            data = json.loads((output / "performance-report.json").read_text(encoding="utf-8"))
        self.assertTrue(result["ok"])
        self.assertEqual(data["learning_count"], 2)
        self.assertEqual(data["pending_count"], 1)
        self.assertNotIn("#9", data["formula_stats"])
        self.assertEqual(data["recommended_formula"], "#22")
        self.assertEqual(data["recommended_hook_type"], "identity-led")
        self.assertIn("Pending metrics", markdown)
        self.assertIn("#22", markdown)


class BuildPipelineTest(unittest.TestCase):
    def test_workflow_aggregates_qa_before_packaging(self) -> None:
        workflow = Path(ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        showcase_start = workflow.index("product-showcase:")
        showcase = workflow[showcase_start:]
        compose_index = showcase.index("- name: Compose Xiaohongshu cards")
        finalize_index = showcase.index("- name: Generate preflight dashboard and aggregate QA")
        package_index = showcase.index("- name: Package QA-green content")
        self.assertLess(compose_index, finalize_index)
        self.assertLess(finalize_index, package_index)
        self.assertIn("--finalize", showcase)

    def test_dashboard_failure_turns_aggregate_qa_red(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            package.mkdir(exist_ok=True)
            original_run = build_package_module.subprocess.run
            original_audit = build_package_module.audit_package
            original_export = build_package_module.export_package
            original_validate = build_package_module.validate_package
            original_dashboard = build_package_module.review_dashboard.generate_package
            build_package_module.subprocess.run = lambda *args, **kwargs: SimpleNamespace(returncode=0)
            build_package_module.audit_package = lambda package_dir: {"ok": True, "total_score": 100}
            build_package_module.export_package = lambda package_dir: {"ok": True, "errors": []}
            build_package_module.validate_package = lambda package_dir, repo_root=None: []
            build_package_module.review_dashboard.generate_package = lambda package_dir: {
                "ok": False,
                "errors": ["dashboard contains a script tag"],
            }
            try:
                errors = build_package_module.compose_and_validate(package, ROOT)
            finally:
                build_package_module.subprocess.run = original_run
                build_package_module.audit_package = original_audit
                build_package_module.export_package = original_export
                build_package_module.validate_package = original_validate
                build_package_module.review_dashboard.generate_package = original_dashboard

            report = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertFalse(report["ok"])
        self.assertTrue(any("review dashboard" in error for error in report["errors"]))

    def test_pattern_audit_precedes_hard_validation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            calls: list[str] = []
            original_run = build_package_module.subprocess.run
            original_audit = build_package_module.audit_package
            original_export = build_package_module.export_package
            original_pattern = build_package_module.pattern_audit.audit_package
            original_validate = build_package_module.validate_package
            original_dashboard = build_package_module.review_dashboard.generate_package
            build_package_module.subprocess.run = lambda *args, **kwargs: SimpleNamespace(returncode=0)
            build_package_module.audit_package = lambda package_dir: {"ok": True, "total_score": 100}
            build_package_module.export_package = lambda package_dir: {"ok": True, "errors": []}

            def run_pattern(package_dir: Path) -> dict:
                calls.append("pattern")
                report = {"schema_version": 1, "ok": True, "passed_count": 10, "total_count": 10, "errors": []}
                (package_dir / "pattern-audit.json").write_text(json.dumps(report), encoding="utf-8")
                return report

            def run_validate(package_dir: Path, repo_root=None) -> list[str]:
                calls.append("validate")
                return []

            def run_dashboard(package_dir: Path) -> dict:
                calls.append("dashboard")
                return {"ok": True, "errors": []}

            build_package_module.pattern_audit.audit_package = run_pattern
            build_package_module.validate_package = run_validate
            build_package_module.review_dashboard.generate_package = run_dashboard
            try:
                errors = build_package_module.compose_and_validate(package, ROOT)
            finally:
                build_package_module.subprocess.run = original_run
                build_package_module.audit_package = original_audit
                build_package_module.export_package = original_export
                build_package_module.pattern_audit.audit_package = original_pattern
                build_package_module.validate_package = original_validate
                build_package_module.review_dashboard.generate_package = original_dashboard

            report = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertEqual(errors, [])
        self.assertEqual(calls, ["pattern", "validate", "dashboard"])
        self.assertTrue(report["ok"])

    def test_early_failure_still_writes_red_aggregate_qa(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            original_run = build_package_module.subprocess.run
            original_audit = build_package_module.audit_package
            original_export = build_package_module.export_package
            original_dashboard = build_package_module.review_dashboard.generate_package
            build_package_module.subprocess.run = lambda *args, **kwargs: SimpleNamespace(returncode=0)
            build_package_module.audit_package = lambda package_dir: {
                "ok": False,
                "errors": ["claim lacks evidence"],
            }

            def fail_unexpected(package_dir: object) -> dict[str, object]:
                raise AssertionError("later gates should not run after semantic failure")

            build_package_module.export_package = fail_unexpected
            build_package_module.review_dashboard.generate_package = fail_unexpected
            try:
                errors = build_package_module.compose_and_validate(package, ROOT)
            finally:
                build_package_module.subprocess.run = original_run
                build_package_module.audit_package = original_audit
                build_package_module.export_package = original_export
                build_package_module.review_dashboard.generate_package = original_dashboard

            report = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertFalse(report["ok"])
        self.assertEqual(errors, report["errors"])
        self.assertTrue(any("semantic alignment gate" in error for error in report["errors"]))


class ContentMemoryTest(unittest.TestCase):
    def record(self, formula: str = "#61", release: str = "v1.0.0", title: str = "别再把Markdown只当笔记了") -> dict:
        return {
            "release": release,
            "title": title,
            "title_formula_id": formula,
            "hook_type": "stop-misuse",
            "published_at": "2026-08-22T10:00:00Z",
            "impressions": 1000,
            "likes": 40,
            "collects": 60,
            "comments": 30,
            "shares": 10,
            "follows": 5,
            "lessons": "Outcome hook drew presentation questions.",
        }

    def test_append_load_and_deduplicate_publication_assets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            first = self.record()
            content_memory.append_record(store, first)
            with self.assertRaises(ValueError):
                content_memory.append_record(store, self.record())
            second = self.record(formula="#36", release="v1.0.1", title="不用重做PPT，Markdown直接放映")
            content_memory.append_record(store, second)
            records = content_memory.load_records(store)
        self.assertEqual([item["release"] for item in records], ["v1.0.0", "v1.0.1"])

    def test_summary_ranks_verified_formula_performance(self) -> None:
        records = [
            self.record(release="v1.0.0", formula="#61"),
            {**self.record(release="v1.0.1", formula="#36"), "impressions": 2000, "likes": 120, "collects": 180, "comments": 50, "shares": 20},
        ]
        summary = content_memory.summarize(records)
        self.assertEqual(summary["record_count"], 2)
        self.assertEqual(summary["recommended_formula"], "#36")
        self.assertGreater(summary["formula_stats"]["#36"]["score"], summary["formula_stats"]["#61"]["score"])

    def test_pending_feedback_is_partitioned_out_of_learning(self) -> None:
        complete = self.record(release="v1.0.0", formula="#61")
        pending = {
            **self.record(release="v1.0.1", formula="#36"),
            "metrics_status": "pending",
        }
        learning, waiting = content_memory.partition_records([complete, pending])
        self.assertEqual([item["release"] for item in learning], ["v1.0.0"])
        self.assertEqual([item["release"] for item in waiting], ["v1.0.1"])
        summary = content_memory.summarize(learning)
        self.assertEqual(summary["record_count"], 1)
        self.assertNotIn("#36", summary["formula_stats"])

    def test_copy_selection_uses_history_and_avoids_fatigue(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = {
                "release": "v1.1.0",
                "version_state": "prerelease",
                "angle": "ReadMD 让同一份 Markdown 直接放映",
                "primary_shot": "presentation.reveal",
                "claims": [{"id": "reveal", "shot_ids": ["presentation.reveal"], "user_value": "放映"}],
            }
            history = [
                {**self.record(release="v1.0.9", formula="#36"), "likes": 2, "collects": 2, "comments": 2, "shares": 0},
                {**self.record(release="v1.0.8", formula="#36", title="ReadMD更新：4个文档工作台升级"), "impressions": 800, "likes": 3, "collects": 3, "comments": 1, "shares": 1},
                {**self.record(release="v1.0.7", formula="#61"), "likes": 120, "collects": 180, "comments": 50, "shares": 25},
            ]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v1.0.9",
                history=history,
            )
        self.assertEqual(result["title_formula_id"], "#61")
        self.assertIn("#36", result["title_selection"]["avoided_formulas"])
        self.assertIn("historical", result["title_selection"]["strategy"])

    def test_copy_selection_ignores_pending_zero_metric_history(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = {
                "release": "v1.1.0",
                "version_state": "prerelease",
                "angle": "ReadMD 让同一份 Markdown 直接放映",
                "primary_shot": "presentation.reveal",
                "claims": [{"id": "reveal", "shot_ids": ["presentation.reveal"], "user_value": "放映"}],
            }
            history = [
                {**self.record(release="v1.0.9", formula="#61"), "impressions": 2400, "likes": 130, "collects": 190, "comments": 45, "shares": 22},
                {**self.record(release="v1.0.8", formula="#36"), "metrics_status": "pending"},
            ]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v1.0.9",
                history=history,
            )
        self.assertEqual(result["title_formula_id"], "#61")
        self.assertNotIn("#36", result["title_selection"]["avoided_formulas"])

    def test_update_record_merges_real_metrics_without_duplicating(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            content_memory.append_record(store, self.record(release="v1.0.0"))
            updated = content_memory.update_record(store, "v1.0.0", {
                "impressions": 2400,
                "likes": 130,
                "collects": 190,
                "comments": 45,
                "shares": 22,
                "follows": 18,
                "lessons": "Identity opening drew thesis questions.",
                "metrics_status": "complete",
            })
            records = content_memory.load_records(store)
        self.assertEqual(updated["impressions"], 2400)
        self.assertEqual(updated["release"], "v1.0.0")
        self.assertEqual(updated["title_formula_id"], "#61")
        self.assertEqual(len(records), 1)
        self.assertEqual(records[0]["metrics_status"], "complete")

    def test_update_record_rejects_invalid_metrics_and_preserves_ledger(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            content_memory.append_record(store, self.record(release="v1.0.0"))
            before = store.read_text(encoding="utf-8")
            with self.assertRaises(ValueError):
                content_memory.update_record(store, "v1.0.0", {"likes": -1})
            after = store.read_text(encoding="utf-8")
        self.assertEqual(after, before)


class ReviewDashboardTest(unittest.TestCase):
    def sample_inputs(self) -> dict:
        return {
            "release": "v1.2.3",
            "title": "不用重做PPT，Markdown直接放映",
            "strategy": "outcome-led",
            "body": "文档已经写完，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。",
            "qa": {"ok": True, "errors": []},
            "copy_review": {
                "ok": True,
                "total_score": 100,
                "scores": {"title": 15, "hook": 15, "focus": 20},
                "hard_failures": [],
                "style": {"score": 96, "findings": []},
            },
            "variants": {
                "ok": True,
                "chosen_strategy": "outcome-led",
                "ranked": [
                    {"strategy": "outcome-led", "adjusted_score": 100, "semantic_score": 100},
                    {"strategy": "identity-led", "adjusted_score": 96, "semantic_score": 100},
                ],
            },
            "wechat_qa": {"ok": True, "errors": []},
            "pattern_audit": {
                "ok": True,
                "passed_count": 10,
                "total_count": 10,
                "errors": [],
            },
            "performance": {
                "learning_count": 2,
                "pending_count": 1,
                "recommended_formula": "#22",
                "recommended_hook_type": "identity-task",
            },
        }

    def test_builds_paste_ready_review_dashboard(self) -> None:
        html = review_dashboard.build_dashboard(self.sample_inputs())
        self.assertIn("不用重做PPT，Markdown直接放映", html)
        self.assertIn("outcome-led", html)
        self.assertIn("100 / 100", html)
        self.assertIn("Style resonance", html)
        self.assertIn("96 / 100", html)
        self.assertIn("Hot-post patterns", html)
        self.assertIn("10 / 10", html)
        self.assertIn("Pending metrics", html)
        for forbidden in ("<script", "class=", "id=", "<img", "<table", "http://", "https://"):
            self.assertNotIn(forbidden.lower(), html.lower())

    def test_marks_failed_gate_without_script_or_external_asset(self) -> None:
        inputs = self.sample_inputs()
        inputs["qa"] = {"ok": False, "errors": ["blank band exceeds 120px"]}
        inputs["pattern_audit"] = {"ok": False, "passed_count": 8, "total_count": 10, "errors": ["card two must be the pure overview.reader hero"]}
        html = review_dashboard.build_dashboard(inputs)
        self.assertIn("NEEDS FIX", html)
        self.assertIn("blank band exceeds 120px", html)
        self.assertIn("card two must be the pure overview.reader hero", html)
        for forbidden in ("<script", "class=", "id=", "<img", "http://"):
            self.assertNotIn(forbidden.lower(), html.lower())


class WatcherTest(unittest.TestCase):
    def make_package_zip(self, root: Path, *, pattern_ok: bool = True) -> Path:
        package = root / "package"
        (package / "images").mkdir(parents=True)
        image = package / "images" / "xhs-01-cover.jpg"
        Image.new("RGB", (10, 10)).save(image)
        (package / "story.json").write_text(json.dumps({"release": "v1.2.3"}), encoding="utf-8")
        (package / "qa.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "copy-review.json").write_text(json.dumps({"ok": True, "total_score": 92}), encoding="utf-8")
        (package / "variants.json").write_text(json.dumps({
            "ok": True,
            "chosen_strategy": "outcome-led",
            "ranked": [{"strategy": "outcome-led", "ok": True, "originality_failures": []}],
        }), encoding="utf-8")
        (package / "dashboard-qa.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "pattern-audit.json").write_text(json.dumps({
            "ok": pattern_ok,
            "passed_count": 10 if pattern_ok else 8,
            "total_count": 10,
            "errors": [] if pattern_ok else ["card two must be the pure overview.reader hero"],
        }), encoding="utf-8")
        (package / "title.txt").write_text("标题", encoding="utf-8")
        (package / "body.txt").write_text("这篇正文足够长，可以形成稳定的三元组指纹。", encoding="utf-8")
        metadata = {
            "title": "标题",
            "strategy": "outcome-led",
            "hook_type": "outcome-led",
            "title_formula_id": "#36",
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
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
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
            records = content_memory.load_records(root / "publication-ledger.jsonl")
            self.assertEqual(len(records), 1)
            feedback = records[0]
            self.assertEqual(feedback["release"], "v1.2.3")
            self.assertEqual(feedback["title_formula_id"], "#36")
            self.assertEqual(feedback["hook_type"], "outcome-led")
            self.assertEqual(feedback["note_id"], "note-1")
            self.assertEqual(feedback["published_url"], "https://example.com/note")
            self.assertEqual(feedback["metrics_status"], "pending")
            self.assertEqual(feedback["impressions"], 0)
        self.assertIn("body_sha256", feedback)
        self.assertIn("opening", feedback)
        self.assertIn("closing", feedback)
        self.assertTrue(feedback["body_trigrams"])

    def test_rejects_failed_hot_post_pattern_gate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, pattern_ok=False)

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run for a failed pattern gate")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = forbidden_publish
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, True,
                )
            finally:
                watch_and_publish.subprocess.run = original_run
            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
        self.assertFalse(published)
        self.assertEqual(record["status"], "retrying")
        self.assertIn("pattern-audit.json is not green", record["error"])


if __name__ == "__main__":
    unittest.main()
