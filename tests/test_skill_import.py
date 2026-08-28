import io
import json
import zipfile
from pathlib import Path

import pytest

from src.readmd_modules import skill_import
from src.readmd_modules.skills import SkillRegistry


def test_parse_github_urls_support_repo_tree_and_blob():
    repo = skill_import.parse_github_url("https://github.com/acme/readmd-skills.git")
    assert repo["owner"] == "acme"
    assert repo["repo"] == "readmd-skills"
    assert repo["ref"] == ""
    tree = skill_import.parse_github_url("https://github.com/acme/readmd-skills/tree/release%2Fv2/writing")
    assert tree["ref"] == "release/v2"
    assert tree["subdir"] == "writing"
    blob = skill_import.parse_github_url("https://github.com/acme/readmd-skills/blob/main/SKILL.md")
    assert blob["subdir"] == ""
    assert blob["canonical_url"].endswith("/tree/main")


@pytest.mark.parametrize(
    "url, code",
    [
        ("http://github.com/acme/skills", "github_host_not_allowed"),
        ("https://github.com/acme/skills?token=secret", "github_url_has_credentials"),
        ("https://github.com/acme/skills/tree/main/../private", "github_path_invalid"),
        ("https://gitlab.com/acme/skills", "github_host_not_allowed"),
    ],
)
def test_parse_rejects_unsafe_urls(url, code):
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.parse_github_url(url)
    assert exc.value.code == code


def test_archive_member_rejects_zip_slip_and_symlink():
    for name in ("../escape", "/absolute", "C:/drive", "folder//file"):
        with pytest.raises(skill_import.SkillImportError):
            skill_import._safe_member(name)
    stream = io.BytesIO()
    with zipfile.ZipFile(stream, "w") as archive:
        info = zipfile.ZipInfo("bundle/link")
        info.external_attr = (0o120777 << 16)
        archive.writestr(info, "target")
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import._extract(stream.getvalue())
    assert exc.value.code == "archive_symlink"


def test_apply_import_persists_pinned_source_and_keeps_scripts_disabled(tmp_path, monkeypatch):
    root = tmp_path / "data"
    monkeypatch.setattr(skill_import, "DATA_DIR", str(root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(root / "skills.json"))
    archive_bytes = io.BytesIO()
    with zipfile.ZipFile(archive_bytes, "w") as archive:
        archive.writestr(
            "owner-repo-sha/SKILL.md",
            "---\nname: note-helper\ndescription: Use when a note needs cleanup.\n---\n\nRewrite {{document}}.\n",
        )
        archive.writestr("owner-repo-sha/tools/run.py", "print('disabled')\n")

    source = {"owner": "owner", "repo": "repo", "ref": "", "subdir": "", "canonical_url": "https://github.com/owner/repo"}
    monkeypatch.setattr(skill_import, "_resolve", lambda parsed, token: ("a" * 40, {"full_name": "owner/repo"}, [{"type": "blob", "path": "SKILL.md"}]))
    monkeypatch.setattr(skill_import, "_request", lambda url, token="", **kwargs: archive_bytes.getvalue())
    preview = {
        "source_id": "gh-test",
        "source": {**source, "resolved_commit": "a" * 40},
        "license_files": [],
        "skills": [{"id": "note-helper", "path": "SKILL.md", "directory": "", "valid": True}],
    }
    result = skill_import.apply_import(preview, preview["skills"], confirm=True)
    assert result["ok"] is True
    skill_dir = root / "skills" / "note-helper"
    assert (skill_dir / "SKILL.md").is_file()
    metadata = json.loads((skill_dir / "readmd.skill.json").read_text(encoding="utf-8"))
    assert metadata["provenance"]["commit"] == "a" * 40
    assert metadata["scripts_allowed"] is False
    assert SkillRegistry([root / "skills"]).get("note-helper") is not None
    assert skill_import.find_source("gh-test")["resolved_commit"] == "a" * 40
    assert skill_import.remove_source("gh-test") is True
    assert skill_import.find_source("gh-test") is None
