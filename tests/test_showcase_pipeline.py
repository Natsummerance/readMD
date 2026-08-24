# -*- coding: utf-8 -*-
"""Product showcase pipeline contract tests."""
from __future__ import annotations

import hashlib
import importlib
import json
import re
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
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
copy_profiles = importlib.import_module("copy_profiles")
export_wechat = importlib.import_module("export_wechat")
performance_report = importlib.import_module("performance_report")
pattern_audit = importlib.import_module("pattern_audit")
package_content = importlib.import_module("package_content")
review_dashboard = importlib.import_module("review_dashboard")
resolve_previous_release = importlib.import_module("resolve_previous_release")
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
    def test_reader_values_are_card_length_contracts(self) -> None:
        for shot_id, value in build_story.USER_VALUES.items():
            with self.subTest(shot_id=shot_id):
                self.assertTrue(8 <= len(value) <= 42)
                self.assertNotIn("CodeMirror", value)

    def test_every_mechanism_has_a_distinct_core_narrative(self) -> None:
        angles = {key: value["narrative_angle"] for key, value in copy_profiles.PROFILES.items()}
        self.assertEqual(len(angles), len(set(angles.values())))
        for primary_shot, angle in angles.items():
            with self.subTest(primary_shot=primary_shot):
                self.assertTrue(angle.startswith("ReadMD "))
                self.assertLessEqual(len(angle), 40)

    def test_mechanism_summary_hooks_are_scannable_and_distinct(self) -> None:
        hooks = {key: value["summary"] for key, value in copy_profiles.PROFILES.items()}
        self.assertEqual(len(hooks), len({item["title"] for item in hooks.values()}))
        for primary_shot, hook in hooks.items():
            with self.subTest(primary_shot=primary_shot):
                self.assertTrue(2 <= len(hook["title"]) <= 10)
                self.assertTrue(8 <= len(hook["caption"]) <= 32)
                self.assertEqual(len(hook["proof_points"]), 3)
                self.assertTrue(all(1 <= len(point) <= 10 for point in hook["proof_points"]))

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
        self.assertEqual(story["cover_hook"], {
            "formula_id": "#36",
            "title": "写完就能讲",
            "caption": "Markdown 直接放映，不用重做 PPT。",
        })
        self.assertEqual(story["narrative_angle"], story["angle"])
        self.assertEqual(
            story["angle"],
            "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
        )
        self.assertEqual(story["summary_hook"], {
            "title": "一条放映路",
            "caption": "写作、修改和上台共用一份文件。",
            "proof_points": ["同一份 MD", "真实排版", "直接放映"],
        })
        plan_by_shot = {item["shot_id"]: item for item in story["card_plan"] if item["shot_id"]}
        claim_by_shot = {
            shot_id: claim["user_value"]
            for claim in story["claims"]
            for shot_id in claim["shot_ids"]
        }
        for shot_id, plan_item in plan_by_shot.items():
            if plan_item["role"] in {"pure_ui_hero", "annotated_ui"} and shot_id in claim_by_shot:
                self.assertEqual(plan_item["caption"], claim_by_shot[shot_id])
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

        self.assertEqual(story["primary_shot"], "overview.editor")
        self.assertEqual(story["cover_hook"]["title"], "同屏改稿")
        self.assertEqual(story["summary_hook"]["title"], "改稿不切窗")

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

    def test_sharing_release_uses_sharing_narrative_not_generic_workbench(self) -> None:
        notes = "- 共享面板支持移动端分享和访问控制\n"
        story = build_story.build_story(
            release="v9.9.9",
            previous_release="v9.9.8",
            notes=notes,
            shot_library_path=ROOT / "showcase" / "shot_library.json",
        )
        self.assertEqual(story["primary_shot"], "sharing.export")
        self.assertEqual(story["angle"], "ReadMD 让本地文档直接生成可控制的共享入口")
        self.assertNotEqual(story["angle"], "ReadMD 正在从 Markdown 阅读器变成完整本地文档工作台")
        self.assertEqual(story["summary_hook"]["title"], "分享可控")

    def test_dense_evidence_outranks_presentation_category_bias(self) -> None:
        notes = (
            "- Diagram Picker 支持 PlantUML、Graphviz 和 Vega 科研图表\n"
            "- Reveal.js 演示\n"
        )
        story = build_story.build_story(
            release="v9.9.9",
            previous_release="v9.9.8",
            notes=notes,
            shot_library_path=ROOT / "showcase" / "shot_library.json",
        )
        self.assertIn("presentation.reveal", story["selected_shots"])
        self.assertEqual(story["primary_shot"], "editor.diagram-picker")
        self.assertEqual(story["angle"], "ReadMD 把科研图表放进同一条 Markdown 工作流")

    def test_claims_can_target_packaged_release_snapshot(self) -> None:
        notes = "- Reveal.js 演示\n"
        story = build_story.build_story(
            release="v9.9.9",
            previous_release="v9.9.8",
            notes=notes,
            shot_library_path=ROOT / "showcase" / "shot_library.json",
            notes_source="evidence/release-notes.md",
        )
        reveal_claim = next(item for item in story["claims"] if item["id"] == "presentation-reveal")
        invisible_claim = story["claims"][-1]
        self.assertIn("evidence/release-notes.md", reveal_claim["sources"])
        self.assertEqual(invisible_claim["sources"], ["evidence/release-notes.md"])


class WriteCopyTest(unittest.TestCase):
    def test_support_phrases_cover_every_authentic_shot(self) -> None:
        self.assertEqual(set(copy_profiles.SUPPORT_PHRASES), set(build_story.USER_VALUES))
        for shot_id, phrase in copy_profiles.SUPPORT_PHRASES.items():
            with self.subTest(shot_id=shot_id):
                self.assertTrue(6 <= len(phrase) <= 24)

    def test_supporting_diagram_workflow_is_not_silently_dropped(self) -> None:
        story = {
            "release": "v1.2.0",
            "version_state": "prerelease",
            "angle": "ReadMD 把科研图表放进同一条 Markdown 工作流",
            "primary_shot": "overview.editor",
            "selected_shots": ["overview.reader", "editor.diagram-picker"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "diagram", "user_value": "科研图表从面板选择", "shot_ids": ["editor.diagram-picker"], "sources": ["README.md"]},
                {"id": "code", "user_value": "代码可以运行", "shot_ids": ["editor.code-chunk"], "sources": ["README.md"]},
            ],
        }
        result = write_copy.generate_copy(
            story,
            repository="Natsummerance/readMD",
            previous_release="v1.1.0",
        )
        self.assertIn("阅读端保持目录和公式排版", result["body"])
        self.assertIn("科研图表留在文档里", result["body"])
        self.assertNotIn("代码示例可以就地验证", result["body"])

    def test_support_workflows_follow_primary_mechanism_priority(self) -> None:
        story = {
            "release": "v1.2.0",
            "version_state": "prerelease",
            "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
            "primary_shot": "presentation.reveal",
            "selected_shots": [
                "overview.reader",
                "presentation.reveal",
                "editor.diagram-picker",
                "editor.code-chunk",
            ],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "diagram", "user_value": "科研图表从面板选择", "shot_ids": ["editor.diagram-picker"], "sources": ["README.md"]},
                {"id": "code", "user_value": "代码可以运行", "shot_ids": ["editor.code-chunk"], "sources": ["README.md"]},
            ],
        }
        result = write_copy.generate_copy(
            story,
            repository="Natsummerance/readMD",
            previous_release="v1.1.0",
        )

        self.assertIn("代码示例可以就地验证", result["body"])
        self.assertIn("科研图表留在文档里", result["body"])
        self.assertNotIn("阅读端保持目录和公式排版", result["body"])

    def test_mechanism_cover_hooks_are_short_unique_and_traceable(self) -> None:
        hooks = {key: value["cover"] for key, value in copy_profiles.PROFILES.items()}
        self.assertEqual(len(hooks), len({item["title"] for item in hooks.values()}))
        for primary_shot, hook in hooks.items():
            with self.subTest(primary_shot=primary_shot):
                self.assertRegex(hook["formula_id"], r"^#\d+$")
                self.assertTrue(2 <= len(hook["title"]) <= 8)
                self.assertTrue(8 <= len(hook["caption"]) <= 32)

    def test_mechanism_cover_variants_track_all_title_formulas(self) -> None:
        expected_formulas = set(copy_variants.TITLE_FORMULAS)
        self.assertEqual(expected_formulas, {"#36", "#9", "#22", "#26", "#61", "#12"})
        for primary_shot, profile in copy_profiles.PROFILES.items():
            with self.subTest(primary_shot=primary_shot):
                variants = profile["cover_variants"]
                self.assertEqual(set(variants), expected_formulas)
                self.assertEqual(set(variants), set(profile["titles"]))
                self.assertEqual(len({item["title"] for item in variants.values()}), 6)
                for formula_id, hook in variants.items():
                    self.assertEqual(hook["formula_id"], formula_id)
                    self.assertTrue(2 <= len(hook["title"]) <= 8)
                    self.assertTrue(8 <= len(hook["caption"]) <= 32)

    def test_apply_selected_cover_syncs_feed_trigger_with_title(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            story["primary_shot"] = "presentation.reveal"
            metadata = {
                "title_formula_id": "#22",
                "variant_id": "identity-led__22",
                "strategy": "identity-led",
                "hook_type": "identity-led",
            }
            updated = build_story.apply_selected_cover(story, metadata)
        self.assertEqual(updated["cover_hook"]["formula_id"], "#22")
        self.assertEqual(updated["cover_hook"]["title"], "上台讲文档的人")
        self.assertEqual(updated["cover_variant_formula_id"], "#22")
        self.assertEqual(updated["card_plan"][0]["title"], "上台讲文档的人")

    def test_title_candidates_cover_all_traceable_experiment_formulas(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v2.3.7-beta.2",
            )
        candidates = result["title_candidates"]
        errors = copy_profiles.title_candidate_errors(candidates)
        self.assertEqual(
            {item["formula_id"] for item in candidates},
            set(copy_variants.TITLE_FORMULAS),
        )
        self.assertEqual(errors, [])
        self.assertGreaterEqual(len({
            copy_profiles.TITLE_FORMULA_CONTRACTS[item["formula_id"]]["family"]
            for item in candidates
        }), 3)
        for item in candidates:
            self.assertLessEqual(len(item["text"]), 20)
            self.assertEqual(copy_profiles.title_formula_errors(item["text"], item["formula_id"]), [])
        self.assertLessEqual(len(result["title"]), 20)

    def test_numeric_title_anchors_match_planned_card_count(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            story["selected_shots"] = ["overview.reader", "overview.editor", "presentation.reveal"]
            story["card_plan"] = [{"file": f"card-{index}.jpg"} for index in range(1, 6)]
            # Deliberately differs from the planned five-card carousel.
            for index in range(2):
                story["claims"].append({
                    "id": f"extra-{index}",
                    "user_value": f"更新 {index}",
                    "shot_ids": ["overview.editor"],
                    "sources": ["release/release-notes.md"],
                })
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v2.3.7-beta.2",
            )

        numeric_titles = [
            item["text"]
            for item in result["title_candidates"]
            if item["formula_id"] in {"#12", "#26"}
        ]
        self.assertEqual(len(numeric_titles), 2)
        for title in numeric_titles:
            with self.subTest(title=title):
                self.assertRegex(title, r"5张")

    def test_topics_follow_release_mechanism_search_intent(self) -> None:
        expected = {
            "overview.editor": ["Markdown", "效率工具", "程序员", "写作", "笔记软件"],
            "presentation.reveal": ["Markdown", "PPT", "演讲", "程序员", "效率工具"],
            "editor.diagram-picker": ["Markdown", "流程图", "科研绘图", "论文", "研究生"],
            "academic.latex-bib": ["LaTeX", "论文写作", "研究生", "学术排版", "Markdown"],
            "editor.code-chunk": ["编程", "Markdown", "程序员", "技术教程", "代码运行"],
            "convert.home": ["PDF", "资料整理", "Markdown", "效率工具", "Word"],
            "sharing.export": ["文档分享", "Markdown", "效率工具", "开源项目", "程序员"],
        }
        for primary_shot, topics in expected.items():
            with self.subTest(primary_shot=primary_shot):
                self.assertEqual(copy_profiles.MECHANISM_TOPICS[primary_shot], topics)

        for primary_shot, topics in expected.items():
            with self.subTest(primary_shot=primary_shot):
                story = {
                    "release": "v1.0.0",
                    "version_state": "prerelease",
                    "primary_shot": primary_shot,
                    "angle": f"ReadMD 把{primary_shot}放进同一条工作流",
                    "selected_shots": ["overview.reader", primary_shot],
                    "claims": [{"id": "primary", "user_value": "机制画面", "shot_ids": [primary_shot], "sources": ["README.md"]}],
                }
                result = write_copy.generate_copy(
                    story,
                    repository="Natsummerance/readMD",
                    previous_release="v0.9.0",
                )
                self.assertEqual(result["topics"], topics)
                self.assertEqual(len(topics), len(set(topics)))
                self.assertTrue(all(not topic.startswith("#") for topic in topics))

    def test_mechanism_topic_sets_form_valid_experiment_pool(self) -> None:
        for primary_shot, topic_sets in copy_profiles.MECHANISM_TOPIC_SETS.items():
            with self.subTest(primary_shot=primary_shot):
                self.assertGreaterEqual(len(topic_sets), 2)
                self.assertEqual(
                    copy_profiles.MECHANISM_TOPICS[primary_shot],
                    topic_sets[0]["topics"],
                )
                self.assertEqual(len({item["label"] for item in topic_sets}), len(topic_sets))
                self.assertEqual(
                    len({write_copy.topic_set_id(item["topics"]) for item in topic_sets}),
                    len(topic_sets),
                )
                markers = copy_profiles.MECHANISM_TOPIC_MARKERS[primary_shot]
                for item in topic_sets:
                    topics = item["topics"]
                    self.assertEqual(len(topics), 5)
                    self.assertEqual(len(topics), len(set(topics)))
                    self.assertTrue(all(topic and not topic.startswith("#") for topic in topics))
                    self.assertTrue(markers.intersection(topics))

    def test_topic_set_selection_uses_confident_history(self) -> None:
        primary_shot = "presentation.reveal"
        candidates = copy_profiles.MECHANISM_TOPIC_SETS[primary_shot]
        default_set, alternate_set = candidates
        default_id = write_copy.topic_set_id(default_set["topics"])
        alternate_id = write_copy.topic_set_id(alternate_set["topics"])

        def history_record(release: str, topic_set_id: str, label: str, impressions: int, likes: int) -> dict:
            return {
                "release": release,
                "primary_shot": primary_shot,
                "topic_set_id": topic_set_id,
                "topic_set_label": label,
                "topics": default_set["topics"] if topic_set_id == default_id else alternate_set["topics"],
                "impressions": impressions,
                "likes": likes,
                "collects": 0,
                "comments": 0,
                "shares": 0,
                "follows": 0,
                "metrics_status": "complete",
            }

        story = {
            "release": "v2.0.0",
            "version_state": "prerelease",
            "primary_shot": primary_shot,
            "angle": "ReadMD 让同一份 Markdown 直接放映",
            "selected_shots": ["overview.reader", primary_shot],
            "claims": [{"id": "primary", "user_value": "直接放映", "shot_ids": [primary_shot], "sources": ["README.md"]}],
        }
        without_history = write_copy.generate_copy(story, repository="x", previous_release="v1.0.0")
        confident_history = [
            history_record("v1.0.0", default_id, default_set["label"], 1000, 1),
            history_record("v1.0.1", default_id, default_set["label"], 1000, 1),
            history_record("v1.1.0", alternate_id, alternate_set["label"], 2000, 120),
            history_record("v1.1.1", alternate_id, alternate_set["label"], 2000, 120),
        ]
        winner = write_copy.generate_copy(
            story,
            repository="x",
            previous_release="v1.1.1",
            history=confident_history,
        )
        fatigued_history = confident_history + [
            history_record("v1.2.0", alternate_id, alternate_set["label"], 1000, 80),
            history_record("v1.2.1", alternate_id, alternate_set["label"], 1000, 80),
        ]
        rotated = write_copy.generate_copy(
            story,
            repository="x",
            previous_release="v1.2.1",
            history=fatigued_history,
        )

        self.assertEqual(without_history["topic_set_label"], default_set["label"])
        self.assertEqual(winner["topic_set_label"], alternate_set["label"])
        self.assertEqual(winner["topic_set_selection"]["chosen_topic_set_id"], alternate_id)
        self.assertIn("historical performance", winner["topic_set_selection"]["reasons"][alternate_id])
        self.assertEqual(rotated["topic_set_label"], default_set["label"])
        self.assertIn("underused topic set", rotated["topic_set_selection"]["reasons"][default_id])
        self.assertIn("recent topic-set fatigue penalty", rotated["topic_set_selection"]["reasons"][alternate_id])

    def test_topic_set_identity_is_stable_and_content_bound(self) -> None:
        story = {
            "release": "v1.0.0",
            "version_state": "prerelease",
            "primary_shot": "presentation.reveal",
            "angle": "ReadMD 让同一份 Markdown 直接放映",
            "selected_shots": ["overview.reader", "presentation.reveal"],
            "claims": [{"id": "primary", "user_value": "直接放映", "shot_ids": ["presentation.reveal"], "sources": ["README.md"]}],
        }
        first = write_copy.generate_copy(story, repository="x", previous_release="v0.9.0")
        second = write_copy.generate_copy(story, repository="x", previous_release="v0.9.0")
        expected_id = hashlib.sha256("\n".join(first["topics"]).encode("utf-8")).hexdigest()[:12]

        self.assertTrue(first["topic_set_id"])
        self.assertEqual(first["topic_set_id"], second["topic_set_id"])
        self.assertEqual(first["topic_set_id"], expected_id)

    def test_generates_compliant_prerelease_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            story = write_story(out)
            story["primary_shot"] = "presentation.reveal"
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

    def test_comment_focus_shapes_reader_scenario(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            story["release"] = "v9.0.0"
            history = [
                {
                    "release": "v8.0.0",
                    "title": "v8",
                    "title_formula_id": "#36",
                    "hook_type": "outcome-led",
                    "published_at": "2026-08-20T10:00:00Z",
                    "metrics_status": "complete",
                    "comment_insights": {
                        "schema_version": 1,
                        "unique_count": 2,
                        "themes": [
                            {"theme": "code", "mentions": 2, "weighted_score": 5, "intents": ["request", "question"]},
                            {"theme": "presentation", "mentions": 1, "weighted_score": 2},
                        ],
                        "top_theme": "code",
                    },
                },
                {
                    "release": "v8.1.0",
                    "title": "v8.1",
                    "title_formula_id": "#22",
                    "hook_type": "identity-led",
                    "published_at": "2026-08-21T10:00:00Z",
                    "metrics_status": "complete",
                    "comment_insights": {
                        "schema_version": 1,
                        "unique_count": 3,
                        "themes": [
                            {"theme": "code", "mentions": 3, "weighted_score": 7, "intents": ["request"]},
                            {"theme": "table", "mentions": 1, "weighted_score": 2},
                        ],
                        "top_theme": "code",
                    },
                },
            ]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v8.1.0",
                history=history,
            )
        self.assertIn("代码教程、技术笔记或示例文档", result["body"])
        directive = result["resonance_directive"]
        self.assertTrue(directive["applied"])
        self.assertEqual(directive["evidence"]["focus"], "code")
        self.assertEqual(directive["evidence"]["confidence"], "medium")
        self.assertEqual(directive["evidence"]["top_intents"], ["request", "question"])
        self.assertEqual(
            set(directive["decisions"]),
            {"keep", "strengthen", "compress", "delete"},
        )

    def test_comment_focus_reorders_supporting_feature(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            story["selected_shots"].extend(["editor.diagram-picker", "editor.code-chunk"])
            story["claims"].extend([
                {
                    "id": "diagram",
                    "user_value": "科研图表从面板选择",
                    "shot_ids": ["editor.diagram-picker"],
                    "sources": ["release/release_notes.md"],
                },
                {
                    "id": "code",
                    "user_value": "代码块就地运行",
                    "shot_ids": ["editor.code-chunk"],
                    "sources": ["release/release_notes.md"],
                },
            ])
            history = [
                {
                    "release": f"v8.{version}.0",
                    "title": f"v8.{version}",
                    "title_formula_id": "#36",
                    "hook_type": "outcome-led",
                    "published_at": f"2026-08-{20 + version}T10:00:00Z",
                    "metrics_status": "complete",
                    "comment_insights": {
                        "schema_version": 1,
                        "unique_count": 1,
                        "themes": [{
                            "theme": "code",
                            "mentions": 2,
                            "weighted_score": 5,
                            "intents": ["request"],
                        }],
                        "top_theme": "code",
                    },
                }
                for version in range(2)
            ]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v8.1.0",
                history=history,
            )
        self.assertLess(
            result["body"].index("代码示例可以就地验证"),
            result["body"].index("阅读端保持目录和公式排版"),
        )
        self.assertNotIn("科研图表留在文档里", result["body"])
        self.assertTrue(result["resonance_directive"]["applied"])

    def test_low_confidence_comment_focus_keeps_default_scenario(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = write_story(Path(tmp))
            history = [{
                "release": "v8.0.0",
                "title": "v8",
                "title_formula_id": "#36",
                "hook_type": "outcome-led",
                "published_at": "2026-08-20T10:00:00Z",
                "metrics_status": "complete",
                "comment_insights": {
                    "schema_version": 1,
                    "unique_count": 2,
                    "themes": [{"theme": "code", "mentions": 2, "weighted_score": 7}],
                    "top_theme": "code",
                },
            }]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v8.0.0",
                history=history,
            )
        self.assertIn("课程讲义、组会报告、技术分享或论文汇报", result["body"])
        self.assertNotIn("代码教程、技术笔记或示例文档", result["body"])
        self.assertFalse(result["resonance_directive"]["applied"])

    def test_copy_follows_selected_mechanism_instead_of_presentation(self) -> None:
        story = {
            "release": "v1.2.0",
            "previous_release": "v1.1.0",
            "version_state": "prerelease",
            "primary_shot": "editor.diagram-picker",
            "angle": "ReadMD 把科研图表放进同一条 Markdown 工作流",
            "selected_shots": ["overview.reader", "editor.diagram-picker"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "diagram", "user_value": "科研图表从面板选择", "shot_ids": ["editor.diagram-picker"], "sources": ["release/release_notes.md"]},
            ],
        }
        result = write_copy.generate_copy(
            story,
            repository="Natsummerance/readMD",
            previous_release="v1.1.0",
        )
        self.assertEqual(result["primary_shot"], "editor.diagram-picker")
        self.assertIn("图表", result["title"])
        first_paragraph = result["body"].split("\n\n", 1)[0]
        self.assertIn("图表", first_paragraph)
        self.assertIn("不用", first_paragraph)
        self.assertNotIn("复制进 PPT", first_paragraph)
        self.assertIn("手写一遍就报废的图表语法", result["body"])
        self.assertIn("论文里的哪类内容画图", result["body"])

    def test_variant_pool_follows_selected_mechanism(self) -> None:
        story = {
            "release": "v1.2.0",
            "previous_release": "v1.1.0",
            "version_state": "prerelease",
            "primary_shot": "editor.diagram-picker",
            "angle": "ReadMD 把科研图表放进同一条 Markdown 工作流",
            "selected_shots": ["overview.reader", "editor.diagram-picker"],
            "claims": [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "diagram", "user_value": "科研图表从面板选择", "shot_ids": ["editor.diagram-picker"], "sources": ["release/release_notes.md"]},
            ],
        }
        base = write_copy.generate_copy(
            story,
            repository="Natsummerance/readMD",
            previous_release="v1.1.0",
        )
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        self.assertEqual(len(variants), 72)
        self.assertTrue(all(item["_report"]["ok"] for item in variants))
        self.assertTrue(all("图表" in item["body"].split("\n\n", 1)[0] for item in variants))
        self.assertTrue(all("复制进 PPT" not in item["body"].split("\n\n", 1)[0] for item in variants))

    def test_every_release_mechanism_has_a_qa_green_variant_pool(self) -> None:
        for primary_shot in copy_profiles.PROFILES:
            with self.subTest(primary_shot=primary_shot):
                story = {
                    "release": "v1.0.0",
                    "previous_release": "v0.9.0",
                    "version_state": "prerelease",
                    "primary_shot": primary_shot,
                    "angle": f"ReadMD 把 {primary_shot} 放进同一条本地工作流",
                    "selected_shots": ["overview.reader", primary_shot],
                    "claims": [
                        {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                        {"id": primary_shot.replace(".", "-"), "user_value": "选中机制", "shot_ids": [primary_shot], "sources": ["release/release_notes.md"]},
                    ],
                }
                base = write_copy.generate_copy(
                    story,
                    repository="Natsummerance/readMD",
                    previous_release="v0.9.0",
                )
                self.assertEqual(base["primary_shot"], primary_shot)
                self.assertTrue(600 <= len(base["body"]) <= 900)
                self.assertTrue(all(len(item["text"]) <= 20 for item in base["title_candidates"]))
                variants = copy_variants.build_variants(story=story, base_metadata=base)
                self.assertEqual(len(variants), 72)
                failed = [item["variant_id"] for item in variants if not item["_report"]["ok"]]
                self.assertEqual(failed, [])


class ValidatePackageTest(unittest.TestCase):
    maxDiff = None

    def test_rejects_chosen_variant_id_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            for name in ("story.json", "metadata.json", "composition.json"):
                (pkg / name).write_text("{}", encoding="utf-8")
            (pkg / "raw").mkdir()
            (pkg / "raw" / "capture.json").write_text("{}", encoding="utf-8")
            (pkg / "variants.json").write_text(json.dumps({
                "ok": True,
                "chosen_strategy": "identity-led",
                "chosen_variant_id": "identity-led__22",
                "ranked": [
                    {"strategy": "identity-led", "variant_id": "identity-led__36", "ok": True},
                    {"strategy": "identity-led", "variant_id": "identity-led__22", "ok": False, "originality_failures": ["body hash matches v1"]},
                ],
            }), encoding="utf-8")
            errors = validate_package.validate_package(pkg)
        self.assertTrue(any("variant originality gate failed" in error for error in errors), errors)

    def test_rejects_chosen_copy_frame_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            for name in ("story.json", "composition.json"):
                (pkg / name).write_text("{}", encoding="utf-8")
            (pkg / "raw").mkdir()
            (pkg / "raw" / "capture.json").write_text("{}", encoding="utf-8")
            (pkg / "metadata.json").write_text(json.dumps({
                "variant_id": "outcome-led__36",
                "copy_frame": "workflow",
            }), encoding="utf-8")
            (pkg / "variants.json").write_text(json.dumps({
                "ok": True,
                "chosen_strategy": "outcome-led",
                "chosen_variant_id": "outcome-led__36",
                "chosen_copy_frame": "core",
                "ranked": [{
                    "strategy": "outcome-led",
                    "variant_id": "outcome-led__36",
                    "copy_frame": "core",
                    "ok": True,
                }],
            }), encoding="utf-8")
            errors = validate_package.validate_package(pkg)
        self.assertTrue(any("selected copy_frame mismatch" in error for error in errors), errors)

    def test_accepts_complete_four_image_package(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            (pkg / "raw").mkdir()
            (pkg / "images").mkdir()
            story = write_story(pkg / "raw")
            story["selected_shots"] = ["overview.reader", "overview.editor"]
            story["primary_shot"] = "overview.editor"
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
                "primary_shot": "overview.editor",
                "topics": ["Markdown", "效率工具", "程序员", "写作", "笔记软件"],
                "topic_set_id": write_copy.topic_set_id(["Markdown", "效率工具", "程序员", "写作", "笔记软件"]),
                "topic_set_label": "writing-core",
                "images": [str(pkg / "images" / name) for name in names],
                "source_urls": ["https://github.com/Natsummerance/readMD/releases"],
                "version_state": "prerelease",
            }
            (pkg / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            (pkg / "title.txt").write_text(metadata["title"], encoding="utf-8")
            (pkg / "body.txt").write_text(metadata["body"], encoding="utf-8")
            (pkg / "topics.txt").write_text("\n".join(metadata["topics"]), encoding="utf-8")
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
            metadata["topic_set_label"] = "not-an-approved-label"
            (pkg / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            mismatched_labels = validate_package.validate_package(pkg)
            self.assertTrue(
                any("topic_set_label does not match approved topics" in error for error in mismatched_labels),
                mismatched_labels,
            )
            metadata["topic_set_id"] = "tampered-topic-set"
            (pkg / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
            mismatched = validate_package.validate_package(pkg)
        self.assertTrue(
            any("topic_set_id does not match approved topic set" in error for error in mismatched),
            mismatched,
        )

    def test_publisher_inputs_recheck_approved_topic_identity(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            pkg = Path(tmp)
            (pkg / "images").mkdir()
            (pkg / "images" / "xhs-01-cover.jpg").write_bytes(b"jpg")
            topics = ["Markdown", "PPT", "演讲", "程序员", "效率工具"]
            story = {"release": "v1.2.3", "primary_shot": "presentation.reveal"}
            metadata = {
                "title": "标题",
                "body": "正文",
                "primary_shot": "presentation.reveal",
                "topics": topics,
                "topic_set_id": write_copy.topic_set_id(topics),
                "topic_set_label": "talk-core",
                "images": [str(pkg / "images" / "xhs-01-cover.jpg")],
            }
            (pkg / "story.json").write_text(json.dumps(story), encoding="utf-8")
            (pkg / "metadata.json").write_text(json.dumps(metadata), encoding="utf-8")
            (pkg / "title.txt").write_text(metadata["title"], encoding="utf-8")
            (pkg / "body.txt").write_text(metadata["body"], encoding="utf-8")
            (pkg / "topics.txt").write_text("\n".join(topics), encoding="utf-8")
            self.assertEqual(validate_package.publisher_input_errors(pkg), [])

            conflicts = [
                (
                    "mechanism",
                    {**story, "primary_shot": "overview.editor"},
                    metadata,
                    {},
                ),
                (
                    "topics",
                    story,
                    {**metadata, "topics": ["Markdown", "LaTeX", "研究生", "学术排版", "论文写作"]},
                    {},
                ),
                ("id", story, {**metadata, "topic_set_id": "tampered-id"}, {}),
                ("label", story, {**metadata, "topic_set_label": "wrong-label"}, {}),
            ]
            for name, next_story, next_metadata, _ in conflicts:
                with self.subTest(name=name):
                    (pkg / "story.json").write_text(json.dumps(next_story), encoding="utf-8")
                    (pkg / "metadata.json").write_text(json.dumps(next_metadata), encoding="utf-8")
                    errors = validate_package.publisher_input_errors(pkg)
                    self.assertTrue(errors, errors)
                    self.assertTrue(
                        any("approved" in error.lower() or "differs from story" in error.lower() for error in errors),
                        errors,
                    )

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
            "cover_hook": {"formula_id": "#36", "title": "写完就能讲", "caption": "Markdown 直接放映，不用重做 PPT。"},
            "summary_hook": {"title": "一条放映路", "caption": "写作、修改和上台共用一份文件。", "proof_points": ["同一份 MD", "真实排版", "直接放映"]},
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
            "topics": ["Markdown", "PPT", "演讲", "程序员", "效率工具"],
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

    def test_audit_rejects_generic_topics_for_mechanism(self) -> None:
        story, metadata, composition = self.make_audit_inputs(self.good_body())
        metadata["topics"] = ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"]
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertIn("topics missing mechanism marker", report["hard_failures"])

    def test_repeated_ai_fingerprint_fails_hard_gate(self) -> None:
        body = self.good_body() + "\n\n对应画面不是概念图。对应画面不是概念图。"
        story, metadata, composition = self.make_audit_inputs(body)
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertTrue(any("repeated" in item.lower() or "重复" in item for item in report["hard_failures"]))

    def test_title_carousel_count_must_match_composed_cards(self) -> None:
        story, metadata, composition = self.make_audit_inputs(self.good_body())
        metadata["title"] = "看完这3张，重新看Markdown"
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertTrue(any("title carousel count" in item.lower() for item in report["hard_failures"]))

    def test_audit_rejects_title_formula_label_without_formula_structure(self) -> None:
        story, metadata, composition = self.make_audit_inputs(self.good_body())
        metadata["title"] = "ReadMD更新：文档变工作台"
        report = audit_copy.audit_copy(story=story, metadata=metadata, composition=composition)
        self.assertFalse(report["ok"])
        self.assertIn("title formula #36 is missing a removal condition", report["hard_failures"])
        self.assertIn("title formula #36 is missing an outcome", report["hard_failures"])

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

    def test_style_audit_flags_structural_ai_fingerprints(self) -> None:
        cases = [
            ("fixed-connectors", "然而第一点。然而第二点。然而第三点。"),
            ("reversal-density", "不是文件，而是工作流。不是界面，而是路径。不是截图，而是证据。"),
            ("depth-overfit", "本质上这是工具。归根结底这是方法。"),
            ("blessing-close", "具体画面都在上面。你值得更稳的文档流。"),
        ]
        for finding_id, body in cases:
            with self.subTest(finding_id=finding_id):
                report = style_audit.audit_style(body)
                self.assertTrue(any(item["id"] == finding_id for item in report["findings"]))
                failed_ids = {
                    item["id"]
                    for item in report["findings"]
                    if item["severity"] == "fail"
                }
                self.assertIn(finding_id, failed_ids)

    def test_style_audit_records_structural_fingerprint_metrics(self) -> None:
        report = style_audit.audit_style("然而有一点。这不是文件，而是工作流。本质上它保留本地文件。")
        metrics = report["metrics"]
        self.assertEqual(metrics["fixed_connector_count"], 1)
        self.assertEqual(metrics["reversal_count"], 1)
        self.assertEqual(metrics["depth_term_count"], 1)
        self.assertFalse(metrics["blessing_close"])

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
            "cover_hook": {"formula_id": "#36", "title": "写完就能讲", "caption": "Markdown 直接放映，不用重做 PPT。"},
            "summary_hook": {"title": "一条放映路", "caption": "写作、修改和上台共用一份文件。", "proof_points": ["同一份 MD", "真实排版", "直接放映"]},
            "selected_shots": ["overview.reader", "presentation.reveal", "overview.editor"],
            "claims": [
                {"id": "reader", "user_value": "打开文档就能看到完整排版、目录和公式渲染", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": "reveal", "user_value": "写完的 Markdown 能直接上台放映，代码、表格和公式不会被切片", "shot_ids": ["presentation.reveal"], "sources": ["release/release_notes.md"]},
                {"id": "editor", "user_value": "讲稿和源文件在同一处修改，预览不会跑偏", "shot_ids": ["overview.editor"], "sources": ["README.md"]},
            ],
            "card_plan": [
                {"index": 1, "file": "cover.jpg", "role": "cover", "shot_id": None, "ui_min_ratio": 0},
                {"index": 2, "file": "hero.jpg", "role": "pure_ui_hero", "shot_id": "overview.reader", "caption": "打开文档就能看到完整排版、目录和公式渲染", "ui_min_ratio": 0.7},
                {"index": 3, "file": "reveal.jpg", "role": "annotated_ui", "shot_id": "presentation.reveal", "caption": "写完的 Markdown 能直接上台放映，代码、表格和公式不会被切片", "ui_min_ratio": 0.55},
                {"index": 4, "file": "editor.jpg", "role": "annotated_ui", "shot_id": "overview.editor", "caption": "讲稿和源文件在同一处修改，预览不会跑偏", "ui_min_ratio": 0.55},
                {"index": 5, "file": "summary.jpg", "role": "summary", "shot_id": None, "title": "一条放映路", "caption": "写作、修改和上台共用一份文件。", "proof_points": ["同一份 MD", "真实排版", "直接放映"], "ui_min_ratio": 0.3},
            ],
        }
        metadata = {
            "title": "不用重做PPT，Markdown直接放映",
            "title_formula_id": "#36",
            "body": (
                "文档写完了，讲的时候还要复制进 PPT。这次把这一步砍掉：Markdown 直接放映。\n\n"
                "这一版的核心就一件事：ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映。\n\n"
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
                {
                    **card("cover.jpg", "cover", 0.35, 0),
                    "feed_readiness": {
                        "ok": True,
                        "title_font_size": 84,
                        "title_width_ratio": 0.36,
                        "title_height_ratio": 0.07,
                        "caption_font_size": 31,
                        "failures": [],
                    },
                },
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
        self.assertEqual(len(report["patterns"]), 11)
        self.assertTrue(all(item["ok"] for item in report["patterns"]))

    def test_cover_type_must_survive_feed_thumbnail_scale(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            composition["cards"][0]["feed_readiness"] = {
                "ok": True,
                "title_font_size": 48,
                "title_width_ratio": 0.08,
                "title_height_ratio": 0.03,
                "caption_font_size": 22,
                "failures": [],
            }
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")

        self.assertFalse(report["ok"])
        cover = next(item for item in report["patterns"] if item["id"] == "thumbnail-first-cover")
        self.assertFalse(cover["ok"])
        self.assertIn("display type", cover["failures"][0])

    def test_cover_hook_must_match_release_mechanism(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            story["cover_hook"]["title"] = "本地文档台"
            story["cover_hook"]["caption"] = "一个本地文档工作台。"
            metadata["title_formula_id"] = "#36"
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertFalse(report["ok"])
        cover = next(item for item in report["patterns"] if item["id"] == "one-hook-cover")
        self.assertFalse(cover["ok"])
        self.assertIn("release mechanism", cover["failures"][0])

    def test_cover_trigger_must_match_title_formula(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            profile = copy_profiles.PROFILES[story["primary_shot"]]
            story["cover_hook"] = dict(profile["cover_variants"]["#22"])
            metadata["title_formula_id"] = "#36"
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertFalse(report["ok"])
        cover = next(item for item in report["patterns"] if item["id"] == "one-hook-cover")
        self.assertFalse(cover["ok"])

    def test_feature_caption_must_match_evidence_backed_reader_value(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            story["card_plan"][2]["caption"] = "CodeMirror 6 编辑器与实时预览左右分栏"
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertFalse(report["ok"])
        collectible = next(item for item in report["patterns"] if item["id"] == "collectible-clarity")
        self.assertFalse(collectible["ok"])

    def test_core_narrative_must_come_from_mechanism_profile(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            story["angle"] = "ReadMD 正在从 Markdown 阅读器变成完整本地文档工作台"
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertFalse(report["ok"])
        focus = next(item for item in report["patterns"] if item["id"] == "single-primary-feature")
        self.assertFalse(focus["ok"])

    def test_summary_hook_must_match_release_mechanism(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story, metadata, composition = self.make_inputs()
            story["summary_hook"] = {
                "title": "本地 Markdown 工作台",
                "caption": "阅读、编辑、转换、学术排版与共享在同一处完成。",
                "proof_points": ["阅读与编辑", "转换与学术排版", "演示与移动共享"],
            }
            self.write_package(package, story, metadata, composition)
            report = pattern_audit.audit_package(package, library_path=ROOT / "showcase/content/pattern-library.json")
        self.assertFalse(report["ok"])
        series = next(item for item in report["patterns"] if item["id"] == "consistent-series-lock")
        self.assertFalse(series["ok"])

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
            story = {
                "release": "v1.2.3",
                "angle": "Markdown 直接放映",
                "summary_hook": {
                    "title": "一条放映路",
                    "caption": "写作、修改和上台共用一份文件。",
                    "proof_points": ["同一份 MD", "真实排版", "直接放映"],
                },
            }
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
        self.assertIn("本轮可保存的三点", html)
        self.assertIn('<ul style="margin:0;padding:0;list-style:none">', html)
        self.assertEqual(html.count("<li style="), 3)
        for paragraph in re.findall(r"<p style=\"([^\"]+)\"", html):
            self.assertIn("font-size", paragraph)
            self.assertIn("line-height", paragraph)
            self.assertIn("color", paragraph)
        for item in re.findall(r"<li style=\"([^\"]+)\"", html):
            self.assertIn("font-size", item)
            self.assertIn("line-height", item)
            self.assertIn("color", item)
        self.assertIn("#GitHub #开源项目 #程序员 #效率工具 #Markdown", html)
        self.assertNotIn("PPT。<", html)

    def test_rejects_wechat_html_missing_inline_styles(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "broken.html"
            path.write_text('<!doctype html><html><body><p>缺行内样式</p></body></html>', encoding="utf-8")
            errors = export_wechat.validate_wechat_html(path)
        self.assertTrue(any("paragraph inline style" in error for error in errors))

    def test_rejects_wechat_list_item_missing_inline_styles(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "broken.html"
            path.write_text(
                '<!doctype html><html><body><ul style="margin:0"><li>缺行内样式</li></ul></body></html>',
                encoding="utf-8",
            )
            errors = export_wechat.validate_wechat_html(path)
        self.assertTrue(any("list item inline style" in error for error in errors))


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
        self.assertEqual(len(variants), 72)
        self.assertEqual(len({item["strategy"] for item in variants}), 3)
        self.assertEqual(len({item["variant_id"] for item in variants}), 72)
        self.assertEqual({item["copy_frame"] for item in variants}, {"core", "workflow", "decision", "source"})
        self.assertEqual(len({item["title"] for item in variants}), 6)
        self.assertEqual(len({item["body"] for item in variants}), 12)
        for hook_type in ("outcome-led", "identity-led", "mechanism-curiosity"):
            formulas = {item["title_formula_id"] for item in variants if item["hook_type"] == hook_type}
            self.assertEqual(formulas, set(copy_variants.TITLE_FORMULAS))
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
            {**self.history_record("v1.0.0", "identity-led", "#22"), "impressions": 2000, "likes": 180, "collects": 260, "comments": 70, "shares": 50, "copy_frame": "core"},
            {**self.history_record("v1.0.1", "identity-led", "#61"), "copy_frame": "workflow"},
            {**self.history_record("v1.0.2", "outcome-led", "#22"), "copy_frame": "decision"},
            {**self.history_record("v1.0.1", "mechanism-curiosity", "#9"), "copy_frame": "workflow"},
            {**self.history_record("v1.0.2", "outcome-led", "#36"), "impressions": 1000, "copy_frame": "core"},
        ]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertEqual(chosen["strategy"], "identity-led")
        self.assertEqual(chosen["variant_id"], "identity-led__22")
        self.assertEqual(chosen["copy_frame"], "core")
        self.assertEqual(report["chosen_variant_id"], "identity-led__22")
        self.assertEqual(report["chosen_copy_frame"], "core")
        self.assertTrue(report["frame_stats"]["core"]["confidence_ok"])
        self.assertTrue(any("historical frame performance bonus" in reason for reason in next(
            item for item in report["ranked"] if item["variant_id"] == "identity-led__22"
        )["reasons"]))
        self.assertTrue(report["hook_stats"]["identity-led"]["confidence_ok"])
        self.assertTrue(report["formula_stats"]["#22"]["confidence_ok"])
        self.assertTrue(report["ok"])

    def test_legacy_missing_copy_frame_is_not_frame_evidence(self) -> None:
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
            {**self.history_record("v1.0.0", "identity-led", "#22"), "impressions": 5000, "likes": 500, "collects": 500, "comments": 100, "shares": 100},
            {**self.history_record("v1.0.1", "outcome-led", "#36"), "impressions": 5000, "likes": 500, "collects": 500, "comments": 100, "shares": 100},
            {**self.history_record("v1.0.2", "identity-led", "#22"), "impressions": 1000, "copy_frame": "workflow"},
            {**self.history_record("v1.0.3", "outcome-led", "#36"), "impressions": 1000, "copy_frame": "workflow"},
        ]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertEqual(set(report["frame_stats"]), {"workflow"})
        self.assertTrue(report["frame_stats"]["workflow"]["confidence_ok"])
        self.assertEqual(chosen["copy_frame"], "workflow")

    def test_low_confidence_history_cannot_lock_selection(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            "likes": 500,
            "collects": 900,
            "comments": 300,
            "shares": 200,
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertFalse(report["hook_stats"]["identity-led"]["confidence_ok"])
        self.assertFalse(report["formula_stats"]["#22"]["confidence_ok"])
        self.assertEqual(chosen["variant_id"], "outcome-led__36")
        self.assertIn("insufficient evidence", report["selection_rule"])

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
        reused = next(item for item in variants if item["variant_id"] == "outcome-led__36")
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            "body_sha256": hashlib.sha256(reused["body"].encode("utf-8")).hexdigest(),
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertNotEqual(chosen["variant_id"], "outcome-led__36")
        outcome_report = next(item for item in report["ranked"] if item["variant_id"] == "outcome-led__36")
        self.assertFalse(outcome_report["ok"])
        self.assertTrue(any("body hash" in failure for failure in next(
            item for item in report["ranked"] if item["variant_id"] == "outcome-led__36"
        )["hard_failures"]))

    def test_originality_gate_rejects_reused_opening_and_closing(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        outcome = next(item for item in variants if item["variant_id"] == "outcome-led__36")
        fingerprints = copy_variants.text_fingerprints(outcome["body"])
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            **fingerprints,
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        self.assertNotEqual(chosen["variant_id"], "outcome-led__36")
        outcome_report = next(item for item in report["ranked"] if item["variant_id"] == "outcome-led__36")
        self.assertFalse(outcome_report["ok"])
        self.assertTrue(any("opening" in failure for failure in outcome_report["hard_failures"]))
        self.assertTrue(any("closing" in failure for failure in outcome_report["hard_failures"]))

    def test_originality_gate_rejects_near_duplicate_template(self) -> None:
        story = self.variant_story()
        base = write_copy.generate_copy(story, repository="Natsummerance/readMD", previous_release="v1.0.0")
        variants = copy_variants.build_variants(story=story, base_metadata=base)
        outcome = next(item for item in variants if item["variant_id"] == "outcome-led__36")
        lightly_edited = outcome["body"].replace("讲的时候还要复制进 PPT", "讲的时候还得复制到 PPT")
        history = [{
            **self.history_record("v1.0.0", "identity-led", "#22"),
            "body_trigrams": list(copy_variants.text_trigrams(lightly_edited)),
        }]
        chosen, report = copy_variants.choose_variant(variants, history)
        outcome_report = next(item for item in report["ranked"] if item["variant_id"] == "outcome-led__36")
        self.assertNotEqual(chosen["variant_id"], "outcome-led__36")
        self.assertFalse(outcome_report["ok"])
        self.assertGreaterEqual(outcome_report["max_body_similarity"], 0.85)
        self.assertTrue(any("near-duplicate body" in failure for failure in outcome_report["hard_failures"]))

    def test_copy_frame_pool_survives_twelve_releases(self) -> None:
        history: list[dict] = []
        used_openings: set[str] = set()
        used_closings: set[str] = set()

        for index in range(12):
            story = self.variant_story()
            story["release"] = f"v1.{index + 1}.0"
            base = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release=f"v1.{index}.0",
            )
            variants = copy_variants.build_variants(story=story, base_metadata=base)
            chosen, report = copy_variants.choose_variant(variants, history)
            fingerprints = copy_variants.text_fingerprints(chosen["body"])
            used_openings.add(fingerprints["opening"])
            used_closings.add(fingerprints["closing"])
            history.append({
                **self.history_record(
                    story["release"],
                    chosen["hook_type"],
                    chosen["title_formula_id"],
                ),
                "variant_id": chosen["variant_id"],
                **fingerprints,
                "body_trigrams": sorted(copy_variants.text_trigrams(chosen["body"])),
            })
            self.assertTrue(report["ok"], report["ranked"])

        self.assertEqual(len(used_openings), 12)
        self.assertEqual(len(used_closings), 12)

    def test_cross_mechanism_pool_avoids_premature_endpoint_reuse(self) -> None:
        primaries = (
            ("presentation.reveal", "放映器支持主题、字号和转场控制"),
            ("overview.editor", "编辑器和实时预览保持同屏同步"),
            ("editor.diagram-picker", "图表选择器能插入可维护的科学图形"),
            ("convert.home", "本地入口能收拢转换、AI 和网页资料"),
        )
        history: list[dict] = []
        seen_body_hashes: set[str] = set()
        used_openings: set[str] = set()
        used_closings: set[str] = set()
        recent_openings: list[str] = []
        recent_closings: list[str] = []
        premature_reuse = 0

        for index in range(24):
            primary_id, primary_value = primaries[index % len(primaries)]
            support_id, support_value = primaries[(index + 1) % len(primaries)]
            if support_id == "overview.reader":
                support_id, support_value = primaries[2]
            story = self.variant_story()
            story["release"] = f"v4.{index + 1}.0"
            story["previous_release"] = f"v4.{index}.0"
            story["primary_shot"] = primary_id
            story["angle"] = f"ReadMD 把{primary_value}放进同一条本地工作台"
            story["selected_shots"] = ["overview.reader", primary_id, support_id]
            story["claims"] = [
                {"id": "reader", "user_value": "完整界面", "shot_ids": ["overview.reader"], "sources": ["README.md"]},
                {"id": primary_id.replace(".", "-"), "user_value": primary_value, "shot_ids": [primary_id], "sources": ["release/release_notes.md"]},
                {"id": "invisible", "user_value": f"第 {index + 1} 条稳定性修复保持本地文件不变", "shot_ids": [], "sources": ["release/release_notes.md"]},
            ]
            base = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release=f"v4.{index}.0",
            )
            variants = copy_variants.build_variants(story=story, base_metadata=base)
            chosen, report = copy_variants.choose_variant(variants, history)
            self.assertTrue(report["ok"], report["ranked"])
            self.assertEqual(report["endpoint_cooldown_releases"], 8)

            fingerprints = copy_variants.text_fingerprints(chosen["body"])
            body_hash = fingerprints["body_sha256"]
            self.assertNotIn(body_hash, seen_body_hashes)
            seen_body_hashes.add(body_hash)
            if fingerprints["opening"] in recent_openings[-8:] or fingerprints["closing"] in recent_closings[-8:]:
                premature_reuse += 1
            used_openings.add(fingerprints["opening"])
            used_closings.add(fingerprints["closing"])
            recent_openings.append(fingerprints["opening"])
            recent_closings.append(fingerprints["closing"])
            del recent_openings[:-8]
            del recent_closings[:-8]
            history.append({
                **self.history_record(
                    story["release"],
                    chosen["hook_type"],
                    chosen["title_formula_id"],
                ),
                "variant_id": chosen["variant_id"],
                "copy_frame": chosen["copy_frame"],
                **fingerprints,
                "body_trigrams": sorted(copy_variants.text_trigrams(chosen["body"])),
            })

        self.assertEqual(premature_reuse, 0)
        self.assertGreaterEqual(len(used_openings), 24)
        self.assertGreaterEqual(len(used_closings), 24)


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
            self.complete("v4", "#22", "identity-led", 1000, 20, 30),
            {**self.complete("v3", "#9", "mechanism-curiosity", 500, 5, 5), "metrics_status": "pending"},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp)
            result = performance_report.generate_report(records, output)
            markdown = (output / "performance-report.md").read_text(encoding="utf-8")
            data = json.loads((output / "performance-report.json").read_text(encoding="utf-8"))
        self.assertTrue(result["ok"])
        self.assertEqual(data["learning_count"], 3)
        self.assertEqual(data["pending_count"], 1)
        self.assertNotIn("#9", data["formula_stats"])
        self.assertEqual(data["recommended_formula"], "#22")
        self.assertEqual(data["recommended_hook_type"], "identity-led")
        self.assertIn("Pending metrics", markdown)
        self.assertIn("#22", markdown)

    def test_low_confidence_dimensions_are_not_recommended(self) -> None:
        records = [{
            **self.complete("v1", "#36", "outcome-led", 1000, 400, 600),
            "comments": 100,
            "shares": 100,
            "follows": 100,
        }]
        with tempfile.TemporaryDirectory() as tmp:
            data = performance_report.generate_report(records, Path(tmp))
        self.assertEqual(data["formula_stats"]["#36"]["confidence"], "low")
        self.assertIsNone(data["recommended_formula"])
        self.assertIsNone(data["recommended_hook_type"])
        self.assertIsNone(data["recommended_copy_frame"])

    def test_legacy_records_without_copy_frame_are_excluded(self) -> None:
        records = [
            {**self.complete("v1", "#36", "outcome-led", 2000, 200, 300), "comments": 100, "shares": 100},
            {**self.complete("v2", "#22", "identity-led", 2000, 200, 300), "comments": 100, "shares": 100},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            data = performance_report.generate_report(records, Path(tmp))
        self.assertEqual(data["frame_stats"], {})
        self.assertIsNone(data["recommended_copy_frame"])

    def test_aggregates_confident_copy_frame_performance(self) -> None:
        records = [
            {**self.complete("v1", "#36", "outcome-led", 1000, 40, 60), "copy_frame": "core"},
            {**self.complete("v2", "#22", "identity-led", 2000, 130, 180), "copy_frame": "workflow"},
            {**self.complete("v3", "#9", "mechanism-curiosity", 1000, 40, 60), "copy_frame": "core"},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            result = performance_report.generate_report(records, Path(tmp))
            markdown = (Path(tmp) / "performance-report.md").read_text(encoding="utf-8")
        self.assertTrue(result["ok"])
        self.assertEqual(result["frame_stats"]["core"]["publications"], 2)
        self.assertEqual(result["frame_stats"]["core"]["confidence"], "medium")
        self.assertEqual(result["frame_stats"]["workflow"]["confidence"], "low")
        self.assertEqual(result["recommended_copy_frame"], "core")
        self.assertIn("## Copy frames", markdown)
        self.assertIn("| core | 2 | 2000 |", markdown)

    def test_aggregates_topic_sets_and_search_terms(self) -> None:
        records = [
            {
                **self.complete("v1", "#36", "outcome-led", 1000, 40, 60),
                "primary_shot": "presentation.reveal",
                "topics": ["Markdown", "PPT"],
                "topic_set_id": "ppt-set",
                "topic_set_label": "talk-core",
            },
            {
                **self.complete("v2", "#22", "identity-led", 2000, 120, 180),
                "primary_shot": "presentation.reveal",
                "topics": ["Markdown", "PPT"],
                "topic_set_id": "ppt-set",
            },
        ]
        with tempfile.TemporaryDirectory() as tmp:
            result = performance_report.generate_report(records, Path(tmp))
            markdown = (Path(tmp) / "performance-report.md").read_text(encoding="utf-8")

        self.assertEqual(result["topic_set_stats"]["ppt-set"]["publications"], 2)
        self.assertEqual(result["topic_set_stats"]["ppt-set"]["label"], "talk-core")
        self.assertEqual(result["topic_stats"]["PPT"]["weighted_engagement"], 740)
        self.assertEqual(result["recommended_topic_set"], "ppt-set")
        self.assertEqual(result["recommended_topic"], "Markdown")
        self.assertIn("## Topic sets", markdown)
        self.assertIn("talk-core", markdown)
        self.assertIn("## Topic search terms", markdown)

    def test_aggregates_comment_focus_across_releases(self) -> None:
        def insights(
            theme: str,
            mentions: int,
            weighted_score: int,
            intents: list[str] | None = None,
        ) -> dict:
            theme_stats = {"theme": theme, "mentions": mentions, "weighted_score": weighted_score}
            if intents is not None:
                theme_stats["intents"] = intents
            return {
                "schema_version": 1,
                "unique_count": mentions,
                "themes": [theme_stats],
                "top_theme": theme,
            }

        records = [
            {**self.complete("v1", "academic", "academic-led", 1000, 40, 60), "comment_insights": insights("academic", 2, 4, ["request"])},
            {**self.complete("v2", "outcome", "outcome-led", 2000, 120, 180), "comment_insights": insights("outcome", 3, 5)},
            {**self.complete("v4", "academic", "identity-led", 1000, 20, 30), "comment_insights": insights("academic", 1, 5, ["request", "question"])},
            {**self.complete("v3", "mechanism", "mechanism-curiosity", 500, 5, 5), "comment_insights": insights("presentation", 1, 1), "metrics_status": "pending"},
        ]
        with tempfile.TemporaryDirectory() as tmp:
            data = performance_report.generate_report(records, Path(tmp))
        focus = data["comment_focus"]["themes"]["academic"]
        self.assertEqual(focus["release_count"], 2)
        self.assertEqual(focus["mentions"], 3)
        self.assertEqual(focus["weighted_score"], 9)
        self.assertEqual(focus["top_intents"], ["request", "question"])
        self.assertEqual(data["comment_focus"]["recommended_theme"], "academic")
        self.assertEqual(data["comment_focus"]["confidence"], "medium")


class PackageContentTest(unittest.TestCase):
    def make_package(self, root: Path) -> Path:
        package = root / "package"
        (package / "images").mkdir(parents=True)
        (package / "raw").mkdir()
        (package / "wechat").mkdir()
        Image.new("RGB", (10, 10)).save(package / "images" / "xhs-01-cover.jpg")
        Image.new("RGB", (20, 10)).save(package / "raw" / "overview-reader.png")
        (package / "raw" / "capture.json").write_text(json.dumps({"schema_version": 1}), encoding="utf-8")
        for name in (
            "story.json",
            "title.txt",
            "body.txt",
            "topics.txt",
            "metadata.json",
            "composition.json",
            "variants.json",
            "qa.json",
            "copy-review.json",
            "pattern-audit.json",
            "dashboard-qa.json",
            "performance-report.json",
            "review-dashboard.html",
            "evidence/release-notes.md",
            "evidence/release.diff",
            "evidence/evidence-manifest.json",
            "wechat/readmd-wechat.html",
            "wechat/wechat-qa.json",
        ):
            path = package / name
            path.parent.mkdir(parents=True, exist_ok=True)
            value = {"ok": True} if name.endswith(".json") else f"{name}\n"
            path.write_text(json.dumps(value, ensure_ascii=False) if isinstance(value, dict) else value, encoding="utf-8")
        manifest = {"schema_version": 1, "artifacts": {}}
        for filename in ("release-notes.md", "release.diff"):
            payload = (package / "evidence" / filename).read_bytes()
            manifest["artifacts"][filename] = {
                "path": f"evidence/{filename}",
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        (package / "evidence" / "evidence-manifest.json").write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        return package

    def test_packages_complete_relative_contract_for_watcher(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = self.make_package(Path(tmp))
            output = Path(tmp) / "content-package.zip"
            report = package_content.package_content(package, output)
            with zipfile.ZipFile(output) as archive:
                names = set(archive.namelist())

        required = {
            "story.json", "metadata.json", "copy-review.json", "pattern-audit.json",
            "evidence/release-notes.md", "evidence/release.diff", "evidence/evidence-manifest.json",
            "dashboard-qa.json", "review-dashboard.html", "wechat/wechat-qa.json",
            "raw/capture.json", "raw/overview-reader.png", "images/xhs-01-cover.jpg",
        }
        self.assertTrue(report["ok"])
        self.assertEqual(report["file_count"], len(names))
        self.assertTrue(required <= names)
        self.assertTrue(all("\\" not in name and not Path(name).is_absolute() for name in names))
        self.assertRegex(report["sha256"], r"^[0-9a-f]{64}$")

    def test_rejects_red_or_incomplete_package(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = self.make_package(Path(tmp))
            package.joinpath("pattern-audit.json").unlink()
            with self.assertRaisesRegex(ValueError, "required package files are missing"):
                package_content.package_content(package, Path(tmp) / "incomplete.zip")

    def test_rejects_red_qa_before_archiving(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = self.make_package(Path(tmp))
            (package / "qa.json").write_text(json.dumps({"ok": False, "errors": ["broken"]}), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "refusing to package red QA"):
                package_content.package_content(package, Path(tmp) / "red.zip")

    def test_rejects_tampered_release_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = self.make_package(Path(tmp))
            evidence_path = package / "evidence" / "release.diff"
            evidence_path.write_bytes(b"x" * evidence_path.stat().st_size)
            with self.assertRaisesRegex(ValueError, "evidence sha256 mismatch: release.diff"):
                package_content.package_content(package, Path(tmp) / "tampered.zip")

    def test_rejects_missing_claim_evidence_snapshot(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = self.make_package(Path(tmp))
            story = json.loads((package / "story.json").read_text(encoding="utf-8"))
            story["claims"] = [{
                "id": "broken",
                "user_value": "无法追溯的修复",
                "shot_ids": [],
                "sources": ["evidence/missing.md"],
                "kind": "invisible",
            }]
            (package / "story.json").write_text(json.dumps(story, ensure_ascii=False), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "packaged evidence references are missing"):
                package_content.package_content(package, Path(tmp) / "missing-evidence.zip")


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
        self.assertIn("python showcase/scripts/package_content.py", showcase)
        self.assertNotIn("Compress-Archive", showcase)

    def test_release_evidence_resolves_highest_previous_semantic_version(self) -> None:
        self.assertEqual(
            resolve_previous_release.select_previous(
                "v2.3.7-beta.3",
                ["v2.3.8", "v2.3.7-beta.2", "v2.3.6", "not-a-version"],
            ),
            "v2.3.7-beta.2",
        )

    def test_release_resolver_ignores_future_and_equal_versions(self) -> None:
        self.assertEqual(
            resolve_previous_release.select_previous(
                "v2.4.0",
                ["v2.5.0", "v2.4.0+build.1", "v2.3.10", "v2.3.9-beta.1"],
            ),
            "v2.3.10",
        )

    def test_release_resolver_rejects_invalid_current_version(self) -> None:
        with self.assertRaisesRegex(ValueError, "current release is not semantic version"):
            resolve_previous_release.select_previous("release-abc", ["v1.0.0"])

    def test_release_workflow_uses_merged_semantic_tag_selection(self) -> None:
        workflow = Path(ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        showcase_start = workflow.index("product-showcase:")
        showcase = workflow[showcase_start:]
        self.assertIn("git tag --list --merged HEAD --sort=-creatordate", showcase)
        self.assertIn("resolve_previous_release.py --current $current @tags", showcase)
        self.assertNotIn("$previous = @($tags | Where-Object", showcase)

    def test_build_package_applies_selected_cover_variant(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            notes = Path(tmp) / "notes.md"
            package = Path(tmp) / "package"
            notes.write_text("- Reveal.js 演说模式放映\n", encoding="utf-8")
            selected_metadata = {
                "title": "给要上台讲文档的人做的MD工具",
                "body": "这是用于验证封面联动的正文。",
                "title_formula_id": "#22",
                "variant_id": "identity-led__22",
                "strategy": "identity-led",
                "hook_type": "identity-led",
                "copy_frame": "core",
                "topics": ["GitHub", "开源项目", "程序员", "效率工具", "Markdown"],
            }
            selection = {
                "ok": True,
                "chosen_strategy": "identity-led",
                "chosen_variant_id": "identity-led__22",
                "chosen_copy_frame": "core",
                "ranked": [],
            }
            original_select = build_package_module.select_variant
            original_report = build_package_module.performance_report.generate_report
            build_package_module.select_variant = lambda **kwargs: (selected_metadata, selection)
            build_package_module.performance_report.generate_report = lambda records, output_dir: {"ok": True}
            try:
                story, metadata = build_package_module.build_package(
                    release="v1.2.3",
                    previous_release="v1.2.2",
                    notes_text=notes.read_text(encoding="utf-8"),
                    diff_text="",
                    package_dir=package,
                    repo_root=ROOT,
                    memory_path=None,
                )
            finally:
                build_package_module.select_variant = original_select
                build_package_module.performance_report.generate_report = original_report

            persisted = json.loads((package / "story.json").read_text(encoding="utf-8"))
            self.assertEqual(metadata["title_formula_id"], "#22")
            self.assertEqual(story["cover_hook"]["formula_id"], "#22")
            self.assertEqual(persisted["cover_hook"]["title"], "上台讲文档的人")
            self.assertEqual(persisted["card_plan"][0]["title"], "上台讲文档的人")
            self.assertTrue((package / "evidence" / "release-notes.md").is_file())
            self.assertTrue((package / "evidence" / "release.diff").is_file())
            manifest = json.loads((package / "evidence" / "evidence-manifest.json").read_text(encoding="utf-8"))
            notes_artifact = manifest["artifacts"]["release-notes.md"]
            self.assertEqual(notes_artifact["path"], "evidence/release-notes.md")
            self.assertEqual(notes_artifact["bytes"], len(notes.read_text(encoding="utf-8").encode("utf-8")))
            reveal_claim = next(item for item in persisted["claims"] if item["id"] == "presentation-reveal")
            self.assertEqual(reveal_claim["sources"][-1], "evidence/release-notes.md")

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

    def test_dashboard_reads_provisional_qa_before_final_aggregate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            observed: dict[str, dict] = {}
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
                report = {"schema_version": 1, "ok": True, "passed_count": 10, "total_count": 10, "errors": []}
                (package_dir / "pattern-audit.json").write_text(json.dumps(report), encoding="utf-8")
                return report

            def run_dashboard(package_dir: Path) -> dict:
                observed["qa"] = json.loads((package_dir / "qa.json").read_text(encoding="utf-8"))
                (package_dir / "review-dashboard.html").write_text("preflight", encoding="utf-8")
                report = {"ok": True, "errors": []}
                (package_dir / "dashboard-qa.json").write_text(json.dumps(report), encoding="utf-8")
                return report

            build_package_module.pattern_audit.audit_package = run_pattern
            build_package_module.validate_package = lambda package_dir, repo_root=None: []
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

            final_qa = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertEqual(errors, [])
        self.assertTrue(observed["qa"]["ok"])
        self.assertTrue(final_qa["ok"])

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

    def test_composition_failure_still_writes_red_aggregate_qa(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            original_run = build_package_module.subprocess.run
            original_audit = build_package_module.audit_package

            def fail_run(*args, **kwargs):
                raise subprocess.CalledProcessError(1, ["node"])

            def fail_audit(package_dir):
                raise AssertionError("semantic gate requires a completed composition")

            try:
                build_package_module.subprocess.run = fail_run
                build_package_module.audit_package = fail_audit
                errors = build_package_module.compose_and_validate(package, ROOT)
            finally:
                build_package_module.subprocess.run = original_run
                build_package_module.audit_package = original_audit

            report = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertFalse(report["ok"])
        self.assertEqual(errors, report["errors"])
        self.assertEqual(errors, ["card composition failed with exit code 1"])


    def test_missing_node_still_writes_red_aggregate_qa(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            with unittest.mock.patch.object(
                build_package_module.subprocess,
                "run",
                side_effect=FileNotFoundError("node executable unavailable"),
            ):
                errors = build_package_module.compose_and_validate(package, ROOT)

            report = json.loads((package / "qa.json").read_text(encoding="utf-8"))
        self.assertFalse(report["ok"])
        self.assertEqual(errors, report["errors"])
        self.assertEqual(errors, ["card composition failed to start: node executable unavailable"])


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
            self.record(release="v1.0.2", formula="#36"),
        ]
        summary = content_memory.summarize(records)
        self.assertEqual(summary["record_count"], 3)
        self.assertEqual(summary["recommended_formula"], "#36")
        self.assertEqual(summary["formula_stats"]["#36"]["confidence"], "medium")
        self.assertEqual(summary["formula_stats"]["#61"]["confidence"], "low")
        self.assertGreater(summary["formula_stats"]["#36"]["score"], summary["formula_stats"]["#61"]["score"])

    def test_two_release_formula_reaches_medium_confidence(self) -> None:
        records = [
            self.record(release="v1.0.0", formula="#36"),
            self.record(release="v1.0.1", formula="#36"),
        ]
        summary = content_memory.summarize(records)
        self.assertEqual(summary["recommended_formula"], "#36")
        self.assertEqual(summary["formula_stats"]["#36"]["confidence"], "medium")

    def test_low_confidence_formula_is_not_recommended(self) -> None:
        records = [
            self.record(release="v1.0.0", formula="#22"),
            {**self.record(release="v1.0.1", formula="#9"), "impressions": 800, "likes": 500, "collects": 500},
        ]
        summary = content_memory.summarize(records)
        self.assertIsNone(summary["recommended_formula"])
        self.assertEqual(summary["formula_stats"]["#9"]["score"], 2.0375)
        self.assertEqual(summary["formula_stats"]["#9"]["confidence"], "low")

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
        self.assertEqual(result["title_formula_id"], "#9")
        self.assertIn("#36", result["title_selection"]["avoided_formulas"])
        self.assertIn("historical performance", result["title_selection"]["reasons"]["#36"])
        self.assertIn("recent fatigue penalty", result["title_selection"]["reasons"]["#36"])

    def test_low_confidence_title_history_cannot_win_selection(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            story = {
                "release": "v1.1.0",
                "version_state": "prerelease",
                "angle": "ReadMD 让同一份 Markdown 直接放映",
                "primary_shot": "presentation.reveal",
                "claims": [{"id": "reveal", "shot_ids": ["presentation.reveal"], "user_value": "放映"}],
            }
            history = [{
                "release": "v1.0.9",
                "title": "给反复改稿的人做的MD工具",
                "title_formula_id": "#22",
                "hook_type": "identity-led",
                "published_at": "2026-08-22T10:00:00Z",
                "impressions": 900,
                "likes": 500,
                "collects": 500,
                "comments": 100,
                "shares": 50,
            }]
            result = write_copy.generate_copy(
                story,
                repository="Natsummerance/readMD",
                previous_release="v1.0.9",
                history=history,
            )
        self.assertEqual(result["title_formula_id"], "#36")
        self.assertIn(
            "low-confidence evidence held as exploration",
            result["title_selection"]["reasons"]["#22"],
        )
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
        self.assertEqual(result["title_formula_id"], "#36")
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

    def seed_pending(self, store: Path) -> None:
        content_memory.append_record(store, {
            **self.record(release="v1.0.0"),
            "variant_id": "identity-led__61",
            "copy_frame": "core",
            "note_id": "note-1",
            "metrics_status": "pending",
        })

    def test_partial_metric_snapshot_keeps_provenance_and_pending_state(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            updated = content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                {"impressions": 1200, "likes": 60},
                source="xiaohongshu-web",
                captured_at="2026-08-23T10:00:00+08:00",
            )
            records = content_memory.load_records(store)
        self.assertEqual(updated["impressions"], 1200)
        self.assertEqual(updated["metrics_status"], "pending")
        self.assertEqual(updated["metrics_source"], "xiaohongshu-web")
        self.assertEqual(updated["likes"], 60)
        self.assertEqual(records[0]["title_formula_id"], "#61")
        self.assertEqual(records[0]["variant_id"], "identity-led__61")

    def test_complete_metric_snapshot_requires_all_platform_counts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            updated = content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                {
                    "impressions": 2400,
                    "likes": 130,
                    "collects": 190,
                    "comments": 45,
                    "shares": 22,
                    "follows": 18,
                },
                source="xiaohongshu-web",
                captured_at="2026-08-23T10:00:00+08:00",
            )
        self.assertEqual(updated["metrics_status"], "complete")

    def test_newer_partial_snapshot_preserves_complete_metrics(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                {
                    "impressions": 2400,
                    "likes": 130,
                    "collects": 190,
                    "comments": 45,
                    "shares": 22,
                    "follows": 18,
                },
                source="xiaohongshu-web",
                captured_at="2026-08-23T10:00:00+08:00",
            )
            updated = content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                {"likes": 140},
                source="xiaohongshu-web",
                captured_at="2026-08-24T10:00:00+08:00",
            )
        self.assertEqual(updated["metrics_status"], "complete")
        self.assertEqual(updated["impressions"], 2400)
        self.assertEqual(updated["likes"], 140)
        self.assertEqual(updated["collects"], 190)

    def test_metric_snapshot_rejects_counter_regression(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            baseline = {
                "impressions": 2400,
                "likes": 130,
                "collects": 190,
                "comments": 45,
                "shares": 22,
                "follows": 18,
            }
            content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                baseline,
                source="xiaohongshu-web",
                captured_at="2026-08-23T10:00:00+08:00",
            )
            before = store.read_text(encoding="utf-8")
            with self.assertRaises(ValueError):
                content_memory.import_metric_snapshot(
                    store,
                    "v1.0.0",
                    {**baseline, "impressions": 2000, "likes": 100},
                    source="xiaohongshu-web",
                    captured_at="2026-08-24T10:00:00+08:00",
                )
            after = store.read_text(encoding="utf-8")
        self.assertEqual(after, before)

    def test_metric_import_rejects_identity_conflicts_and_stale_snapshots(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            with self.assertRaises(ValueError):
                content_memory.import_metric_snapshot(
                    store,
                    "v1.0.0",
                    {"impressions": 100, "copy_frame": "workflow"},
                    source="xiaohongshu-web",
                    captured_at="2026-08-23T10:00:00+08:00",
                )
            content_memory.import_metric_snapshot(
                store,
                "v1.0.0",
                {
                    "impressions": 2000,
                    "likes": 60,
                    "collects": 80,
                    "comments": 40,
                    "shares": 20,
                    "follows": 10,
                },
                source="xiaohongshu-web",
                captured_at="2026-08-24T10:00:00+08:00",
            )
            before = store.read_text(encoding="utf-8")
            with self.assertRaises(ValueError):
                content_memory.import_metric_snapshot(
                    store,
                    "v1.0.0",
                    {"impressions": 10, **{field: 1 for field in ("likes", "collects", "comments", "shares", "follows")}},
                    source="manual",
                    captured_at="2026-08-23T10:00:00+08:00",
                )
            after = store.read_text(encoding="utf-8")
        self.assertEqual(after, before)

    def test_metric_import_preserves_topic_attribution_identity(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            content_memory.append_record(store, {
                **self.record(release="v1.0.0"),
                "primary_shot": "presentation.reveal",
                "topic_set_id": "7dd35d0592b9",
                "topics": ["Markdown", "PPT", "演讲", "程序员", "效率工具"],
                "metrics_status": "pending",
            })
            before = store.read_text(encoding="utf-8")
            for conflict in (
                {"primary_shot": "overview.editor"},
                {"topic_set_id": "tampered"},
                {"topics": ["Markdown", "LaTeX"]},
            ):
                with self.subTest(conflict=conflict):
                    with self.assertRaises(ValueError):
                        content_memory.import_metric_snapshot(
                            store,
                            "v1.0.0",
                            {"impressions": 100, **conflict},
                            source="xiaohongshu-web",
                            captured_at="2026-08-23T10:00:00+08:00",
                        )
            after = store.read_text(encoding="utf-8")
        self.assertEqual(after, before)

    def test_resonance_directive_is_immutable_publication_identity(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            directive = {
                "schema_version": 1,
                "applied": True,
                "evidence": {"focus": "code", "confidence": "medium"},
                "decisions": {"keep": "same core", "strengthen": "code first"},
            }
            self.seed_pending(store)
            content_memory.update_record(store, "v1.0.0", {"resonance_directive": directive})
            before = store.read_text(encoding="utf-8")
            with self.assertRaises(ValueError):
                content_memory.import_metric_snapshot(
                    store,
                    "v1.0.0",
                    {"impressions": 100, "resonance_directive": {**directive, "applied": False}},
                    source="xiaohongshu-web",
                    captured_at="2026-08-23T10:00:00+08:00",
                )
            after = store.read_text(encoding="utf-8")

        self.assertEqual(after, before)

    def test_comment_snapshot_imports_anonymized_resonance_themes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            updated = content_memory.import_comment_snapshot(
                store,
                "v1.0.0",
                {"comments": [
                    {
                        "id": "author-secret",
                        "author": "公开昵称",
                        "text": "希望能直接放映论文公式，很好用",
                        "likes": 4,
                    },
                    {"text": "表格导出会不会丢失格式？", "likes": 2},
                ]},
                source="xiaohongshu-web",
                captured_at="2026-08-24T10:00:00+08:00",
            )
            serialized = store.read_text(encoding="utf-8")
        insights = updated["comment_insights"]
        themes = {item["theme"]: item for item in insights["themes"]}
        self.assertEqual(insights["imported_count"], 2)
        self.assertEqual(insights["unique_count"], 2)
        self.assertEqual(themes["presentation"]["mentions"], 1)
        self.assertEqual(themes["presentation"]["weighted_score"], 5)
        self.assertIn("request", themes["presentation"]["intents"])
        self.assertIn("praise", themes["presentation"]["intents"])
        self.assertIn("question", themes["table"]["intents"])
        self.assertEqual(len(insights["evidence_hashes"]), 2)
        self.assertTrue(all(re.fullmatch(r"[0-9a-f]{16}", item) for item in insights["evidence_hashes"]))
        self.assertNotIn("希望能直接放映论文公式", serialized)
        self.assertNotIn("表格导出会不会丢失格式", serialized)
        self.assertNotIn("author-secret", serialized)

    def test_comment_snapshots_accumulate_anonymized_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            presentation_text = "希望能直接放映论文公式，很好用"
            table_text = "表格导出会不会丢失格式？"
            code_text = "代码块在长文里能保持可运行吗？"
            content_memory.import_comment_snapshot(
                store,
                "v1.0.0",
                {"comments": [
                    {"text": presentation_text, "likes": 4},
                    {"text": table_text, "likes": 2},
                ]},
                source="xiaohongshu-web",
                captured_at="2026-08-24T10:00:00+08:00",
            )
            updated = content_memory.import_comment_snapshot(
                store,
                "v1.0.0",
                {"comments": [
                    {"text": presentation_text, "likes": 9},
                    {"text": code_text, "likes": 2},
                ]},
                source="xiaohongshu-web",
                captured_at="2026-08-25T10:00:00+08:00",
            )
        insights = updated["comment_insights"]
        themes = {item["theme"]: item for item in insights["themes"]}
        presentation_hash = content_memory._comment_hash(presentation_text)
        self.assertEqual(insights["schema_version"], 2)
        self.assertEqual(insights["imported_count"], 2)
        self.assertEqual(insights["new_count"], 1)
        self.assertEqual(insights["unique_count"], 3)
        self.assertEqual(len(insights["evidence_hashes"]), 3)
        self.assertEqual(insights["evidence_weights"][presentation_hash], 10)
        self.assertEqual(themes["presentation"]["mentions"], 1)
        self.assertEqual(themes["presentation"]["weighted_score"], 10)
        self.assertEqual(themes["table"]["mentions"], 1)
        self.assertEqual(themes["table"]["weighted_score"], 3)
        self.assertEqual(themes["code"]["mentions"], 1)
        self.assertEqual(themes["code"]["weighted_score"], 3)

    def test_comment_snapshot_rejects_invalid_and_stale_data(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            store = Path(tmp) / "ledger.jsonl"
            self.seed_pending(store)
            with self.assertRaises(ValueError):
                content_memory.import_comment_snapshot(
                    store,
                    "v1.0.0",
                    {"comments": [{"text": "很好用", "likes": -1}]},
                    source="manual",
                    captured_at="2026-08-24T10:00:00+08:00",
                )
            content_memory.import_comment_snapshot(
                store,
                "v1.0.0",
                {"comments": [{"text": "很好用"}]},
                source="manual",
                captured_at="2026-08-24T10:00:00+08:00",
            )
            before = store.read_text(encoding="utf-8")
            with self.assertRaises(ValueError):
                content_memory.import_comment_snapshot(
                    store,
                    "v1.0.0",
                    {"comments": [{"text": "更新后更好用"}]},
                    source="xiaohongshu-web",
                    captured_at="2026-08-23T10:00:00+08:00",
                )
            after = store.read_text(encoding="utf-8")
        self.assertEqual(after, before)


class ReviewDashboardTest(unittest.TestCase):
    def sample_inputs(self) -> dict:
        return {
            "release": "v1.2.3",
            "title": "不用重做PPT，Markdown直接放映",
            "strategy": "outcome-led",
            "resonance_directive": {
                "schema_version": 1,
                "applied": True,
                "evidence": {
                    "focus": "code",
                    "confidence": "medium",
                    "release_count": 2,
                    "mentions": 5,
                    "weighted_score": 18,
                    "top_intents": ["request", "question"],
                },
                "decisions": {
                    "keep": "同一份 Markdown 从写作走到放映。",
                    "strengthen": "优先展示代码示例可以就地验证。",
                    "compress": "辅助能力最多保留两条。",
                    "delete": "不加入与代码焦点无关的新卖点。",
                },
            },
            "story": {
                "release": "v1.2.3",
                "primary_shot": "presentation.reveal",
                "angle": "ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映",
                "cover_hook": {
                    "formula_id": "#36",
                    "title": "写完就能讲",
                    "caption": "Markdown 直接放映，不用重做 PPT。",
                },
                "summary_hook": {
                    "title": "一条放映路",
                    "caption": "写作、修改和上台共用一份文件。",
                    "proof_points": ["同一份 MD", "真实排版", "直接放映"],
                },
                "card_plan": [
                    {"index": 2, "role": "pure_ui_hero", "shot_id": "overview.reader", "caption": "打开文档就能看到完整排版"},
                    {"index": 3, "role": "annotated_ui", "shot_id": "presentation.reveal", "caption": "写完的 Markdown 能直接上台放映"},
                ],
            },
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
                "chosen_variant_id": "outcome-led__36",
                "candidate_count": 60,
                "copy_frame_inventory": {
                    "outcome-led": 4,
                    "identity-led": 4,
                    "mechanism-curiosity": 4,
                },
                "ranked": [
                    {"strategy": "outcome-led", "variant_id": "outcome-led__36", "title": "#36 标题", "adjusted_score": 100, "semantic_score": 100},
                    {"strategy": "identity-led", "variant_id": "identity-led__22", "title": "#22 标题", "adjusted_score": 96, "semantic_score": 100},
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
                "recommended_copy_frame": "workflow",
                "recommended_topic_set": "academic-talk",
                "recommended_topic": "组会报告",
                "topic_set_stats": {
                    "academic-talk": {
                        "label": "academic-talk",
                        "publications": 2,
                        "impressions": 2400,
                        "confidence": "medium",
                    },
                },
                "comment_focus": {
                    "recommended_theme": "code",
                    "confidence": "medium",
                    "themes": {
                        "code": {
                            "release_count": 2,
                            "mentions": 5,
                            "weighted_score": 18,
                            "top_intents": ["request", "question"],
                            "confidence": "medium",
                        },
                        "presentation": {
                            "release_count": 1,
                            "mentions": 2,
                            "weighted_score": 4,
                            "confidence": "low",
                        },
                    },
                },
            },
            "topic_experiment": {
                "primary_shot": "presentation.reveal",
                "topics": ["Markdown", "PPT", "演讲", "程序员", "效率工具"],
                "topic_set_id": "7dd35d0592b9",
                "topic_set_label": "talk-core",
                "topic_set_selection": {
                    "strategy": "confidence-gated historical performance with fatigue and coverage balancing",
                    "sample_size": 4,
                    "avoided_topic_sets": [],
                },
            },
        }

    def test_builds_paste_ready_review_dashboard(self) -> None:
        html = review_dashboard.build_dashboard(self.sample_inputs())
        self.assertIn("不用重做PPT，Markdown直接放映", html)
        self.assertIn("Mechanism contract", html)
        self.assertIn("presentation.reveal", html)
        self.assertIn("ReadMD 让同一份 Markdown 从阅读、编辑直接走到上台放映", html)
        self.assertIn("写完就能讲", html)
        self.assertIn("一条放映路", html)
        self.assertIn("同一份 MD", html)
        self.assertIn("打开文档就能看到完整排版", html)
        self.assertIn("outcome-led", html)
        self.assertIn("outcome-led__36", html)
        self.assertIn("100 / 100", html)
        self.assertIn("Style resonance", html)
        self.assertIn("96 / 100", html)
        self.assertIn("Hot-post patterns", html)
        self.assertIn("10 / 10", html)
        self.assertIn("Recommended frame", html)
        self.assertIn("workflow", html)
        self.assertIn("Topic experiment", html)
        self.assertIn("talk-core", html)
        self.assertIn("7dd35d0592b9", html)
        self.assertIn("PPT", html)
        self.assertIn("演讲", html)
        self.assertIn("程序员", html)
        self.assertIn("效率工具", html)
        self.assertIn("confidence-gated historical performance", html)
        self.assertIn("Recommended topic set", html)
        self.assertIn("academic-talk", html)
        self.assertIn("Recommended search term", html)
        self.assertIn("组会报告", html)
        self.assertIn("Comment resonance", html)
        self.assertIn("code", html)
        self.assertIn("5 mentions", html)
        self.assertIn("weighted 18", html)
        self.assertIn("medium confidence", html)
        self.assertIn("request, question", html)
        self.assertIn("Next-draft directive", html)
        self.assertIn("Applied to this draft", html)
        self.assertIn("Anonymized intents", html)
        self.assertIn("Next-draft decisions", html)
        self.assertIn("优先展示代码示例可以就地验证。", html)
        self.assertIn("Pending metrics", html)
        for forbidden in ("<script", "class=", "id=", "<img", "<table", "http://", "https://"):
            self.assertNotIn(forbidden.lower(), html.lower())

    def test_comment_resonance_requires_confident_evidence(self) -> None:
        inputs = self.sample_inputs()
        inputs["performance"]["comment_focus"] = {
            "recommended_theme": None,
            "confidence": "low",
            "themes": {},
        }
        html = review_dashboard.build_dashboard(inputs)
        self.assertIn("No confident comment evidence yet", html)

    def test_resonance_directive_has_explicit_missing_state(self) -> None:
        inputs = self.sample_inputs()
        del inputs["resonance_directive"]
        html = review_dashboard.build_dashboard(inputs)
        self.assertIn("Resonance directive is unavailable", html)

    def test_mechanism_contract_has_explicit_missing_state(self) -> None:
        inputs = self.sample_inputs()
        del inputs["story"]
        html = review_dashboard.build_dashboard(inputs)
        self.assertIn("Mechanism contract is unavailable", html)

    def test_collect_inputs_loads_story_contract(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            package = Path(tmp)
            story = {"release": "v1.2.3", "primary_shot": "presentation.reveal"}
            (package / "story.json").write_text(json.dumps(story), encoding="utf-8")
            (package / "metadata.json").write_text("{}", encoding="utf-8")
            inputs = review_dashboard.collect_inputs(package)
        self.assertEqual(inputs["story"]["primary_shot"], "presentation.reveal")

    def test_curates_top_experiments_instead_of_dumping_sixty_cards(self) -> None:
        inputs = self.sample_inputs()
        ranked = [{
            "strategy": f"strategy-{index}",
            "variant_id": f"challenge-{index:02d}",
            "copy_frame": "workflow",
            "remaining_copy_frames": 4,
            "title": f"#12 挑战 {index}",
            "semantic_score": 100,
            "adjusted_score": 109 - index,
            "ok": True,
        } for index in range(1, 11)]
        ranked.append({
            "strategy": "outcome-led",
            "variant_id": "outcome-led__36",
            "copy_frame": "core",
            "remaining_copy_frames": 4,
            "title": "#36 标题",
            "semantic_score": 100,
            "adjusted_score": 80,
            "ok": True,
        })
        inputs["variants"]["ranked"] = ranked
        html = review_dashboard.build_dashboard(inputs)
        self.assertIn("Top experiments", html)
        self.assertIn("Showing 5 of 60 candidates", html)
        self.assertIn("Copy-frame inventory", html)
        self.assertIn("4 / hook", html)
        self.assertIn("outcome-led__36", html)
        self.assertIn("challenge-01", html)
        self.assertIn("challenge-04", html)
        self.assertNotIn("challenge-05", html)
        self.assertEqual(html.count('border:2px solid #d6482c'), 1)
        self.assertEqual(html.count('border:1px solid #d8dee6;padding:18px 20px'), 4)
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
    def make_package_zip(
        self,
        root: Path,
        *,
        pattern_ok: bool = True,
        wechat_ok: bool = True,
        variant_match: bool = True,
        frame_match: bool = True,
        publisher_match: bool = True,
        topic_match: bool = True,
        tamper_semantics: bool = False,
        tamper_assets: bool = False,
        tamper_directive: bool = False,
    ) -> Path:
        package = root / "package"
        (package / "images").mkdir(parents=True)
        (package / "raw").mkdir()
        notes = (
            "# ReadMD v1.2.3\n\n"
            "## Highlights\n\n"
            "- Markdown 写完以后可以直接进入 Reveal.js 放映模式。\n"
            "- 课程讲义、组会报告和技术分享都能沿用同一份源文件。\n"
        )
        story = build_story.build_story(
            release="v1.2.3",
            previous_release="v1.2.2",
            notes=notes,
            diff="diff --git a/a b/a\n",
            shot_library_path=ROOT / "showcase" / "shot_library.json",
            notes_source="evidence/release-notes.md",
        )
        metadata = write_copy.generate_copy(
            story,
            repository="Natsummerance/readMD",
            previous_release="v1.2.2",
        )
        metadata, variant_selection = copy_variants.select_variant(
            story=story,
            base_metadata=metadata,
            history=None,
        )
        story = build_story.apply_selected_cover(story, metadata)
        composition = copy_variants.projected_composition(story)
        for index, card in enumerate(story["card_plan"]):
            path = package / "images" / card["file"]
            Image.new("RGB", (1080, 1440), (18 + index * 29, 72 + index * 17, 120 + index * 11)).save(
                path, "JPEG", quality=92,
            )

        capture_shots = []
        for index, shot_id in enumerate(story["selected_shots"]):
            filename = f"{shot_id.replace('.', '-')}.png"
            path = package / "raw" / filename
            Image.new("RGB", (960, 1280), (24 + index * 37, 88 + index * 13, 150 - index * 19)).save(path, "PNG")
            capture_shots.append({
                "shot_id": shot_id,
                "file": f"raw/{filename}",
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            })
        (package / "raw" / "capture.json").write_text(json.dumps({
            "schema_version": 1,
            "release": story["release"],
            "shots": capture_shots,
        }), encoding="utf-8")
        if tamper_assets:
            Image.new("RGB", (960, 1280), "#123456").save(
                package / "raw" / capture_shots[0]["file"].removeprefix("raw/"), "PNG",
            )

        canvas_area = 1080 * 1440
        for card in composition["cards"]:
            if card["role"] == "cover":
                card["feed_readiness"] = {
                    "title_font_size": 84,
                    "title_width_ratio": 0.36,
                    "title_height_ratio": 0.07,
                    "caption_font_size": 31,
                }
                card["screenshot_box"] = {"x": 70, "y": 640, "width": 940, "height": 580}
                continue
            ratio = float(card.get("ui_area_ratio", 0))
            width = 940
            height = round(ratio * canvas_area / width)
            card["screenshot_box"] = {
                "x": 70,
                "y": 180,
                "width": width,
                "height": height,
            }
        semantic_report = audit_copy.audit_copy(
            story=story,
            metadata=metadata,
            composition=composition,
        )
        if not semantic_report["ok"]:
            raise AssertionError(json.dumps(semantic_report, ensure_ascii=False, indent=2))
        pattern_report = pattern_audit.audit_patterns(
            story=story,
            metadata=metadata,
            composition=composition,
            library_path=ROOT / "showcase" / "content" / "pattern-library.json",
        )
        if not pattern_report["ok"]:
            raise AssertionError(json.dumps(pattern_report, ensure_ascii=False, indent=2))
        if tamper_semantics:
            metadata["title"] = "ReadMD更新：文档变工作台"
            (package / "title.txt").write_text(metadata["title"], encoding="utf-8")

        (package / "story.json").write_text(json.dumps(story, ensure_ascii=False), encoding="utf-8")
        (package / "qa.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "copy-review.json").write_text(json.dumps({"ok": True, "total_score": 92}), encoding="utf-8")
        if variant_match and frame_match:
            persisted_variants = variant_selection
        else:
            persisted_variants = dict(variant_selection)
            persisted_variants["chosen_variant_id"] = (
                "identity-led__22" if not variant_match else "outcome-led__36"
            )
            persisted_variants["chosen_copy_frame"] = "workflow" if not frame_match else "core"
        (package / "variants.json").write_text(json.dumps(persisted_variants, ensure_ascii=False), encoding="utf-8")
        (package / "performance-report.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "performance-report.md").write_text("# Performance\n", encoding="utf-8")
        (package / "dashboard-qa.json").write_text(json.dumps({"ok": True}), encoding="utf-8")
        (package / "pattern-audit.json").write_text(json.dumps({
            "ok": pattern_ok,
            "passed_count": 10 if pattern_ok else 8,
            "total_count": 10,
            "errors": [] if pattern_ok else ["card two must be the pure overview.reader hero"],
        }), encoding="utf-8")
        (package / "wechat").mkdir()
        (package / "wechat" / "wechat-qa.json").write_text(json.dumps({
            "ok": wechat_ok,
            "errors": [] if wechat_ok else ["paragraph inline style incomplete"],
        }), encoding="utf-8")
        (package / "title.txt").write_text(metadata["title"], encoding="utf-8")
        (package / "body.txt").write_text(metadata["body"], encoding="utf-8")
        (package / "topics.txt").write_text("\n".join(metadata["topics"]), encoding="utf-8")
        (package / "composition.json").write_text(json.dumps(composition, ensure_ascii=False), encoding="utf-8")
        (package / "review-dashboard.html").write_text("<!doctype html><main>preflight</main>", encoding="utf-8")
        (package / "evidence").mkdir()
        (package / "evidence" / "release-notes.md").write_text("# Release\n", encoding="utf-8")
        (package / "evidence" / "release.diff").write_text("diff --git a/a b/a\n", encoding="utf-8")
        evidence_manifest = {"schema_version": 1, "artifacts": {}}
        for filename in ("release-notes.md", "release.diff"):
            payload = (package / "evidence" / filename).read_bytes()
            evidence_manifest["artifacts"][filename] = {
                "path": f"evidence/{filename}",
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        (package / "evidence" / "evidence-manifest.json").write_text(
            json.dumps(evidence_manifest, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        (package / "wechat" / "readmd-wechat.html").write_text("<p style=\"font-size:16px;line-height:1.75;color:#111\">article</p>", encoding="utf-8")
        metadata["images"] = [f"Z:/remote/package/images/{card['file']}" for card in story["card_plan"]]
        if not topic_match:
            metadata["topic_set_id"] = "tampered-topic-set"
        (package / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
        if tamper_directive:
            metadata["resonance_directive"] = {
                "schema_version": 1,
                "applied": False,
                "evidence": {
                    "focus": "general",
                    "confidence": "low",
                    "release_count": 0,
                    "mentions": 0,
                    "weighted_score": 0,
                    "top_intents": [],
                },
                "decisions": {},
            }
            (package / "metadata.json").write_text(json.dumps(metadata, ensure_ascii=False), encoding="utf-8")
        if not publisher_match:
            (package / "body.txt").write_text("这篇正文和元数据不一致，不能进入发布器。", encoding="utf-8")
        zip_path = root / "package.zip"
        report = package_content.package_content(package, zip_path)
        return Path(report["output"])

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

    def test_rejects_topic_identity_mismatch_despite_green_reports(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, topic_match=False)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    raise AssertionError("publisher must not click a mismatched topic set")
                if command[2] == "status":
                    return SimpleNamespace(returncode=0, stdout=json.dumps({}), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertFalse(published)
            self.assertEqual(calls, [])
            self.assertEqual(record["status"], "failed")
            self.assertIn("publisher input contract failed", record["error"])
            self.assertIn("topic_set_id does not match approved topic set", record["error"])

    def test_full_publish_records_status_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            expected_title = json.loads(
                (root / "package" / "metadata.json").read_text(encoding="utf-8")
            )["title"]
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    payload = {"published": True, "noteId": "note-1", "url": "https://example.com/note"}
                else:
                    prior_status_calls = sum(1 for item in calls if item[2] == "status")
                    payload = {} if prior_status_calls == 1 else {"status": "审核中"}
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
            self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
            self.assertEqual(record["release"], "v1.2.3")
            self.assertEqual(record["variant_id"], "outcome-led__36")
            self.assertEqual(record["title"], expected_title)
            self.assertEqual(record["note_id"], "note-1")
            self.assertEqual(record["published_url"], "https://example.com/note")
            self.assertEqual(record["audit_status"], "审核中")
            records = content_memory.load_records(root / "publication-ledger.jsonl")
            self.assertEqual(len(records), 1)
            feedback = records[0]
            self.assertEqual(feedback["release"], "v1.2.3")
            self.assertEqual(feedback["variant_id"], "outcome-led__36")
            self.assertEqual(feedback["copy_frame"], "core")
            self.assertEqual(feedback["title_formula_id"], "#36")
            self.assertEqual(feedback["hook_type"], "outcome-led")
            self.assertEqual(feedback["primary_shot"], "presentation.reveal")
            self.assertEqual(
                feedback["topic_set_id"],
                write_copy.topic_set_id(["Markdown", "PPT", "演讲", "程序员", "效率工具"]),
            )
            self.assertEqual(feedback["topic_set_label"], "talk-core")
            self.assertEqual(
                feedback["topics"],
                ["Markdown", "PPT", "演讲", "程序员", "效率工具"],
            )
            self.assertEqual(feedback["note_id"], "note-1")
            self.assertEqual(feedback["published_url"], "https://example.com/note")
            self.assertEqual(feedback["metrics_status"], "pending")
            self.assertEqual(feedback["impressions"], 0)
            token = next(
                key for key, value in state["packages"].items()
                if value.get("release") == "v1.2.3"
            )
            work_package = root / "work" / token
            self.assertTrue((work_package / "raw" / "capture.json").is_file())
            localized_image = Path(json.loads(
                (work_package / "metadata.json").read_text(encoding="utf-8")
            )["images"][0])
            self.assertTrue(localized_image.is_file())
            self.assertEqual(localized_image.parent, work_package / "images")
            self.assertIn("body_sha256", feedback)
            self.assertIn("opening", feedback)
            self.assertIn("closing", feedback)
            self.assertTrue(feedback["body_trigrams"])

    def test_recomputed_semantic_gate_blocks_forged_green_review(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, tamper_semantics=True)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    raise AssertionError("publisher must not trust a forged green review")
                return SimpleNamespace(returncode=0, stdout=json.dumps({}), stderr="")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertFalse(published)
            self.assertEqual(calls, [])
            self.assertEqual(record["status"], "failed")
            self.assertIn("recomputed publication gates failed", record["error"])
            self.assertIn("title formula #36 is missing a removal condition", record["error"])

    def test_asset_contract_blocks_tampered_authentic_capture(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, tamper_assets=True)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                raise AssertionError("publisher must not click tampered UI evidence")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertFalse(published)
            self.assertEqual(calls, [])
            self.assertEqual(record["status"], "failed")
            self.assertIn("publisher asset contract failed", record["error"])
            self.assertIn("SHA-256 mismatch in capture.json: overview.reader", record["error"])

    def test_directive_contract_blocks_tampered_next_draft(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, tamper_directive=True)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                raise AssertionError("publisher must not click a package without auditable edit decisions")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertFalse(published)
            self.assertEqual(calls, [])
            self.assertEqual(record["status"], "failed")
            self.assertIn("publisher directive contract failed", record["error"])
            self.assertIn("resonance directive missing decisions: compress, delete, keep, strengthen", record["error"])

    def test_successful_publish_survives_status_query_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    payload = {"published": True, "noteId": "note-status-failure", "url": "https://example.com/note"}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                if command[2] == "status":
                    prior_status_calls = sum(1 for item in calls if item[2] == "status")
                    if prior_status_calls == 1:
                        return SimpleNamespace(returncode=0, stdout=json.dumps({}), stderr="")
                    raise RuntimeError("status query unavailable")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
                second_published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            records = content_memory.load_records(root / "publication-ledger.jsonl")
            self.assertTrue(published)
            self.assertFalse(second_published)
            self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
            self.assertEqual(record["status"], "published")
            self.assertEqual(record["note_id"], "note-status-failure")
            self.assertEqual(record["audit_status"], "unknown")
            self.assertIn("status query unavailable", record["status_query_error"])
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]["note_id"], "note-status-failure")

    def test_repairs_failed_feedback_ledger_without_republishing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    payload = {"published": True, "noteId": "note-ledger-repair", "url": "https://example.com/note"}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                if command[2] == "status":
                    published_already = any(call[2] == "publish" for call in calls)
                    payload = {"status": "审核中"} if published_already else {}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            original_seed = watch_and_publish.seed_feedback_ledger
            watch_and_publish.subprocess.run = fake_run
            seed_calls = []

            def failing_then_real_seed(*args, **kwargs):
                seed_calls.append(kwargs)
                if len(seed_calls) == 1:
                    raise OSError("ledger unavailable")
                return original_seed(*args, **kwargs)

            watch_and_publish.seed_feedback_ledger = failing_then_real_seed
            try:
                first = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
                second = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
            finally:
                watch_and_publish.subprocess.run = original_run
                watch_and_publish.seed_feedback_ledger = original_seed

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            records = content_memory.load_records(root / "publication-ledger.jsonl")
            self.assertTrue(first)
            self.assertFalse(second)
            self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
            self.assertEqual(len(seed_calls), 2)
            self.assertEqual(record["ledger_status"], "seeded")
            self.assertNotIn("ledger_error", record)
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]["note_id"], "note-ledger-repair")

    def test_ledger_repair_preserves_existing_metrics(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            ledger = root / "publication-ledger.jsonl"
            content_memory.append_record(ledger, {
                "release": "v1.2.3",
                "title": "标题",
                "title_formula_id": "#36",
                "hook_type": "outcome-led",
                "published_at": "2026-08-20T10:00:00Z",
                "impressions": 120,
                "likes": 12,
                "collects": 9,
                "comments": 3,
                "shares": 2,
                "follows": 1,
                "metrics_status": "pending",
            })
            record = {
                "release": "v1.2.3",
                "title": "标题",
                "status": "published",
                "ledger_status": "seed_failed",
                "ledger_error": "ledger unavailable",
                "ledger_repair_error": "repair unavailable",
                "result": {"published": True, "noteId": "existing-note"},
            }
            watch_and_publish.repair_failed_feedback_ledger(
                record,
                zip_path,
                root / "package",
                ledger,
            )

            records = content_memory.load_records(ledger)
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]["impressions"], 120)
            self.assertEqual(records[0]["likes"], 12)
            self.assertEqual(record["ledger_status"], "seeded")
            self.assertNotIn("ledger_error", record)
            self.assertNotIn("ledger_repair_error", record)

    def test_failed_ledger_repair_keeps_published_guard(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    payload = {"published": True, "noteId": "note-repair-failure", "url": "https://example.com/note"}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                if command[2] == "status":
                    published_already = any(call[2] == "publish" for call in calls)
                    payload = {"status": "审核中"} if published_already else {}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            original_seed = watch_and_publish.seed_feedback_ledger
            watch_and_publish.subprocess.run = fake_run

            def failing_seed(*args, **kwargs):
                raise OSError("ledger still unavailable")

            watch_and_publish.seed_feedback_ledger = failing_seed
            try:
                first = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
                second = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
            finally:
                watch_and_publish.subprocess.run = original_run
                watch_and_publish.seed_feedback_ledger = original_seed

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertTrue(first)
            self.assertFalse(second)
            self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
            self.assertEqual(record["status"], "published")
            self.assertEqual(record["ledger_status"], "seed_failed")
            self.assertIn("ledger still unavailable", record["ledger_repair_error"])

    def test_preflight_platform_status_blocks_republish_without_local_memory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    raise AssertionError("publisher must not click an already accepted note")
                if command[2] == "status":
                    payload = {"status": "审核中", "noteId": "already-accepted"}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                first = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
                second = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            records = content_memory.load_records(root / "publication-ledger.jsonl")
            self.assertFalse(first)
            self.assertFalse(second)
            self.assertEqual([call[2] for call in calls], ["status"])
            self.assertEqual(record["status"], "published")
            self.assertTrue(record["reconciled"])
            self.assertTrue(record["preflight_reconciled"])
            self.assertEqual(record["note_id"], "already-accepted")
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]["note_id"], "already-accepted")

    def test_rejects_publisher_inputs_diverging_from_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, publisher_match=False)

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run when input contract fails")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = forbidden_publish
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                )
            finally:
                watch_and_publish.subprocess.run = original_run

            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
            self.assertFalse(published)
            self.assertIn("publisher input contract failed", record["error"])
            self.assertIn("body.txt", record["error"])
            self.assertEqual(record["status"], "failed")

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

    def test_rejects_failed_wechat_adapter_gate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, wechat_ok=False)

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run for a failed WeChat adapter gate")

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
        self.assertIn("wechat-qa.json is not green", record["error"])

    def test_reconciles_accepted_note_after_publisher_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    return SimpleNamespace(returncode=1, stdout="", stderr="publisher output timeout")
                if command[2] == "status":
                    prior_status_calls = sum(1 for item in calls if item[2] == "status")
                    payload = (
                        {}
                        if prior_status_calls == 1
                        else {"status": "审核中", "noteId": "recovered-note"}
                    )
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

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
            feedback_records = content_memory.load_records(root / "publication-ledger.jsonl")
        self.assertTrue(published)
        self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
        self.assertEqual(record["status"], "published")
        self.assertTrue(record["reconciled"])
        self.assertEqual(record["note_id"], "recovered-note")
        self.assertEqual(record["audit_status"], "审核中")
        self.assertEqual(len(feedback_records), 1)
        self.assertEqual(feedback_records[0]["note_id"], "recovered-note")

    def test_unknown_status_failure_is_not_treated_as_published(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            calls = []

            def fake_run(command, **kwargs):
                calls.append(command)
                if command[2] == "publish":
                    return SimpleNamespace(returncode=1, stdout="", stderr="publisher output timeout")
                if command[2] == "status":
                    prior_status_calls = sum(1 for item in calls if item[2] == "status")
                    payload = (
                        {}
                        if prior_status_calls == 1
                        else {"status": "未知", "noteId": None}
                    )
                    return SimpleNamespace(returncode=0, stdout=json.dumps(payload), stderr="")
                raise AssertionError(f"unexpected publisher command: {command}")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = fake_run
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 1, False,
                    ledger_path=root / "publication-ledger.jsonl",
                )
            finally:
                watch_and_publish.subprocess.run = original_run
            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
        self.assertFalse(published)
        self.assertEqual([call[2] for call in calls], ["status", "publish", "status"])
        self.assertEqual(record["status"], "failed")
        self.assertFalse(record.get("reconciled", False))
        self.assertFalse((root / "publication-ledger.jsonl").exists())

    def test_rejects_tampered_release_evidence_before_publish(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            extracted = root / "extracted"
            watch_and_publish.safe_extract(zip_path, extracted)
            evidence_path = extracted / "evidence" / "release.diff"
            evidence_path.write_bytes(b"x" * evidence_path.stat().st_size)
            tampered_zip = root / "tampered.zip"
            with zipfile.ZipFile(tampered_zip, "w") as archive:
                for file in extracted.rglob("*"):
                    if file.is_file():
                        archive.write(file, file.relative_to(extracted).as_posix())

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run for tampered release evidence")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = forbidden_publish
            try:
                published = watch_and_publish.process_package(
                    tampered_zip, root / "work", root / "state.json", Path("publisher.py"), 3, True,
                )
            finally:
                watch_and_publish.subprocess.run = original_run
            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
        self.assertFalse(published)
        self.assertIn("evidence sha256 mismatch: release.diff", record["error"])

    def test_rejects_variant_id_mismatch_before_publish(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, variant_match=False)

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run when selected variants disagree")

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
        self.assertIn("selected variant_id mismatch", record["error"])

    def test_rejects_release_already_present_in_publication_ledger(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root)
            ledger = root / "publication-ledger.jsonl"
            content_memory.append_record(ledger, {
                "release": "v1.2.3",
                "title": "旧标题",
                "title_formula_id": "#36",
                "hook_type": "outcome-led",
                "published_at": "2026-08-20T10:00:00Z",
                "impressions": 100,
                "likes": 10,
                "collects": 10,
                "comments": 10,
                "shares": 10,
                "follows": 1,
                "metrics_status": "pending",
            })

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run when the ledger already contains the release")

            original_run = watch_and_publish.subprocess.run
            watch_and_publish.subprocess.run = forbidden_publish
            try:
                published = watch_and_publish.process_package(
                    zip_path, root / "work", root / "state.json", Path("publisher.py"), 3, True,
                    ledger_path=ledger,
                )
            finally:
                watch_and_publish.subprocess.run = original_run
            state = json.loads((root / "state.json").read_text(encoding="utf-8"))
            record = next(iter(state["packages"].values()))
        self.assertFalse(published)
        self.assertEqual(record["status"], "retrying")
        self.assertIn("release already exists in publication ledger", record["error"])

    def test_rejects_copy_frame_mismatch_before_publish(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            zip_path = self.make_package_zip(root, frame_match=False)

            def forbidden_publish(*args, **kwargs):
                raise AssertionError("publisher must not run when selected frames disagree")

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
        self.assertIn("selected copy_frame mismatch", record["error"])


if __name__ == "__main__":
    unittest.main()
