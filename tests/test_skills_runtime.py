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
