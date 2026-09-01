from pathlib import Path


ROOT = Path(__file__).parents[1]


def test_every_frontend_ai_chat_entrypoint_forwards_the_shared_credential():
    files = [
        ROOT / "assets" / "js" / "features" / "ai.js",
        ROOT / "assets" / "js" / "features" / "export.js",
        ROOT / "assets" / "js" / "editor" / "editor.js",
        ROOT / "assets" / "js" / "reader" / "fixes.js",
    ]
    calls = 0
    for path in files:
        text = path.read_text(encoding="utf-8")
        marker = "apiFetch('/api/ai/chat'"
        cursor = 0
        while True:
            start = text.find(marker, cursor)
            if start < 0:
                break
            calls += 1
            payload = text[start:start + 1800]
            assert "credential_id:" in payload, path
            assert "provider:" in payload, path
            cursor = start + len(marker)
    assert calls == 4


def test_export_schema_instruction_lives_in_the_skill_not_javascript():
    export_js = (ROOT / "assets" / "js" / "features" / "export.js").read_text(encoding="utf-8")
    skill = (ROOT / "assets" / "skills" / "readmd-export-style" / "SKILL.md").read_text(encoding="utf-8")
    assert "typography.font/size" not in export_js
    assert "typography.font" in skill
    assert "table.headerBg" in skill
