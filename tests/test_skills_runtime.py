import json
from pathlib import Path

import pytest


@pytest.fixture()
def skill_root(tmp_path):
    root = tmp_path / "skills"
    (root / "builtin" / "summarize").mkdir(parents=True)
    (root / "builtin" / "summarize" / "SKILL.md").write_text(
        "---\nname: summarize\ndescription: Use when a document needs a concise summary.\n---\n"
        "Summarize {{document}} for {{language}}. Request: {{request}}.\n",
        encoding="utf-8",
    )
    (root / "builtin" / "summarize" / "readmd.skill.json").write_text(
        json.dumps({"id": "summarize", "capabilities": ["read"], "locales": ["en"]}),
        encoding="utf-8",
    )
    return root


def test_skill_registry_loads_and_renders_variables(skill_root):
    from src.readmd_modules.skills import SkillRegistry

    registry = SkillRegistry([skill_root / "builtin"])
    skill = registry.get("summarize")
    assert skill is not None
    assert "Summarize a document" in registry.render(
        "summarize",
        {"document": "a document", "language": "English", "request": "short"},
    )


def test_skill_registry_rejects_missing_required_variable(skill_root):
    from src.readmd_modules.skills import SkillError, SkillRegistry

    registry = SkillRegistry([skill_root / "builtin"])
    with pytest.raises(SkillError, match="document"):
        registry.render("summarize", {"language": "English", "request": "short"})


def test_skill_registry_precedence_project_over_user_over_builtin(tmp_path):
    from src.readmd_modules.skills import SkillRegistry

    roots = []
    for scope, text in (("builtin", "builtin"), ("user", "user"), ("project", "project")):
        folder = tmp_path / scope / "same"
        folder.mkdir(parents=True)
        (folder / "SKILL.md").write_text(
            f"---\nname: same\ndescription: Use when testing {scope}.\n---\n{text} {{document}}",
            encoding="utf-8",
        )
        roots.append(tmp_path / scope)

    registry = SkillRegistry(roots)
    assert registry.render("same", {"document": "doc"}).startswith("project")


def test_skill_registry_accepts_localized_trigger_description(tmp_path):
    from src.readmd_modules.skills import SkillRegistry

    folder = tmp_path / "localized" / "zh-writer"
    folder.mkdir(parents=True)
    (folder / "SKILL.md").write_text(
        "---\nname: zh-writer\ndescription: 当需要润色中文文档时使用。\n---\n润色 {{document}}。\n",
        encoding="utf-8",
    )
    skill = SkillRegistry([tmp_path / "localized"]).get("zh-writer")
    assert skill is not None
    assert skill.description == "当需要润色中文文档时使用。"


def test_skill_registry_blocks_path_traversal_and_scripts(skill_root, tmp_path):
    from src.readmd_modules.skills import SkillError, SkillRegistry

    outside = tmp_path / "secret.md"
    outside.write_text("secret", encoding="utf-8")
    with pytest.raises(SkillError):
        SkillRegistry([outside])


def test_disabled_user_skill_is_not_resolved(skill_root):
    from src.readmd_modules.skills import SkillRegistry

    meta = skill_root / "builtin" / "summarize" / "readmd.skill.json"
    meta.write_text(json.dumps({"id": "summarize", "enabled": False}), encoding="utf-8")
    assert SkillRegistry([skill_root / "builtin"]).get("summarize") is None


def test_disabled_skill_is_visible_only_to_read_only_workbench(skill_root):
    from src.readmd_modules.skills import SkillRegistry

    meta = skill_root / "builtin" / "summarize" / "readmd.skill.json"
    meta.write_text(json.dumps({"id": "summarize", "enabled": False}), encoding="utf-8")
    registry = SkillRegistry([skill_root / "builtin"])
    assert registry.get("summarize") is None
    assert [item.id for item in registry.list(include_disabled=True)] == ["summarize"]


def test_ai_payload_resolves_skill_without_raw_system_prompt():
    from src.readmd_modules.ai import _skill_messages

    messages = _skill_messages({
        "skill_id": "readmd-summary",
        "skill_variables": {"document": "# Title\nBody", "language": "English"},
        "messages": [{"role": "user", "content": "summarize this"}],
    })
    assert messages[0]["role"] == "system"
    assert "summarize" in messages[0]["content"].lower()
    assert "# Title" in messages[0]["content"]


def test_shared_core_service_exposes_same_registry():
    from src.readmd_core import ReadMDCoreService

    service = ReadMDCoreService()
    assert service.get_skill("readmd-summary") is not None
    rendered = service.render_skill("readmd-summary", {"document": "# Title"})
    assert rendered.startswith("Summarize the document")
    assert "# Title" in rendered


def test_skill_render_round_trips_windows_paths_without_regex_escape_error(skill_root):
    """Regression: document content containing C:\\Users paths used to crash
    SkillRegistry.render with re.error: bad escape \\U (v2.3.8 item 7)."""
    from src.readmd_modules.skills import SkillRegistry

    registry = SkillRegistry([skill_root / "builtin"])
    document = (
        "# Notes\n"
        "See C:\\Users\\Natsumer\\Documents\\notes.md and\n"
        "D:\\Data\\实验结果\\run-\\d1\\output.csv for details.\n"
        "Backslash sequences: \\n \\t \\x41 \\g<1> \\1"
    )
    rendered = registry.render(
        "summarize",
        {"document": document, "language": "English", "request": "short"},
    )
    assert document in rendered


def test_api_skill_evaluate_replaces_windows_paths_verbatim():
    """The HTTP Skill evaluation path must not treat backslashes as regex
    replacement escapes (the registry path had already been fixed)."""
    import re

    rendered = "Document: {{document}}"
    value = "C:\\Users\\Natsumer\\notes\\draft.md\nD:\\Data\\run-\\d1"
    result = re.sub(
        r'\{\{\s*document\s*\}\}',
        lambda _match, replacement=value: replacement,
        rendered,
    )
    assert result == "Document: " + value
