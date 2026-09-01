import hashlib
import io
import json
import stat
import zipfile
from pathlib import Path
from types import SimpleNamespace

import pytest

from src.readmd_modules import skill_import
from src.readmd_modules.skills import SkillRegistry


def _write_skill_bundle(root: Path, skill_id: str = "note-helper") -> Path:
    root.mkdir(parents=True, exist_ok=True)
    (root / "LICENSE").write_text("MIT License\n\nPermission is hereby granted.\n", encoding="utf-8")
    folder = root / "skills" / skill_id
    (folder / "references").mkdir(parents=True)
    (folder / "tools").mkdir()
    (folder / "SKILL.md").write_text(
        "---\n"
        f"name: {skill_id}\n"
        "description: Use when a note needs cleanup.\n"
        "---\n\n"
        "Rewrite {{document}} for {{language}}.\n",
        encoding="utf-8",
    )
    (folder / "references" / "example.md").write_text("# Example\n", encoding="utf-8")
    (folder / "tools" / "run.py").write_text("print('disabled')\n", encoding="utf-8")
    return folder


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


def test_resolve_supports_slash_containing_branch_names(monkeypatch):
    source = skill_import.parse_github_url(
        "https://github.com/acme/readmd-skills/tree/feature/docs/skills"
    )

    def fake_json(url, token=""):
        if url.endswith("/acme/readmd-skills"):
            return {"default_branch": "main"}
        if "/commits/feature%2Fdocs%2Fskills" in url:
            raise skill_import.SkillImportError("github_not_found", "missing")
        if "/commits/feature%2Fdocs" in url:
            return {"sha": "a" * 40}
        if "/git/trees/" in url:
            return {"tree": [{"type": "blob", "path": "skills/note/SKILL.md"}]}
        raise AssertionError(url)

    monkeypatch.setattr(skill_import, "_json", fake_json)
    sha, _, _ = skill_import._resolve(source, "")
    assert sha == "a" * 40
    assert source["ref"] == "feature/docs"
    assert source["subdir"] == "skills"
    assert "/tree/feature%2Fdocs/skills" in source["canonical_url"]


def test_github_preview_uses_same_license_and_variable_validation(monkeypatch):
    entries = [{"type": "blob", "path": "skills/note/SKILL.md"}]
    monkeypatch.setattr(
        skill_import,
        "_resolve",
        lambda source, token: ("a" * 40, {"full_name": "acme/skills"}, entries),
    )
    monkeypatch.setattr(
        skill_import,
        "_content",
        lambda source, sha, path, token: (
            "---\nname: note\ndescription: Use when notes need work.\n---\n\nRewrite {{document}}.\n"
        ),
    )

    preview = skill_import.preview_import("https://github.com/acme/skills")

    assert preview["source"]["type"] == "github"
    assert preview["skills"][0]["valid"] is False
    assert "skill_license_missing" in preview["skills"][0]["error_codes"]

    entries.append({"type": "blob", "path": "LICENSE"})
    monkeypatch.setattr(
        skill_import,
        "_content",
        lambda source, sha, path, token: (
            "---\nname: note\ndescription: Use when notes need work.\n---\n\nRewrite {{private_prompt}}.\n"
        ),
    )
    preview = skill_import.preview_import("https://github.com/acme/skills")
    assert "skill_variables_invalid" in preview["skills"][0]["error_codes"]


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
        archive.writestr("owner-repo-sha/LICENSE", "MIT License\n")

    source = {"owner": "owner", "repo": "repo", "ref": "", "subdir": "", "canonical_url": "https://github.com/owner/repo"}
    monkeypatch.setattr(
        skill_import,
        "_resolve",
        lambda parsed, token: (
            "a" * 40,
            {"full_name": "owner/repo"},
            [
                {"type": "blob", "path": "SKILL.md"},
                {"type": "blob", "path": "tools/run.py"},
                {"type": "blob", "path": "LICENSE"},
            ],
        ),
    )
    monkeypatch.setattr(skill_import, "_request", lambda url, token="", **kwargs: archive_bytes.getvalue())
    preview = {
        "source_id": "gh-test",
        "source": {**source, "resolved_commit": "a" * 40},
        "license_files": ["LICENSE"],
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


def test_github_apply_revalidates_archive_license_at_apply_boundary(tmp_path, monkeypatch):
    root = tmp_path / "data"
    monkeypatch.setattr(skill_import, "DATA_DIR", str(root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(root / "skills.json"))
    archive_bytes = io.BytesIO()
    with zipfile.ZipFile(archive_bytes, "w") as archive:
        archive.writestr(
            "owner-repo-sha/note/SKILL.md",
            "---\nname: note\ndescription: Use when a note needs cleanup.\n---\n\nRewrite {{document}}.\n",
        )
    monkeypatch.setattr(
        skill_import,
        "_resolve",
        lambda parsed, token: (
            "a" * 40,
            {"full_name": "owner/repo"},
            [{"type": "blob", "path": "note/SKILL.md"}],
        ),
    )
    monkeypatch.setattr(skill_import, "_request", lambda url, token="", **kwargs: archive_bytes.getvalue())
    preview = {
        "source_id": "gh-test",
        "source": {
            "type": "github",
            "owner": "owner",
            "repo": "repo",
            "ref": "",
            "subdir": "",
            "canonical_url": "https://github.com/owner/repo",
            "resolved_commit": "a" * 40,
        },
        "skills": [{"id": "note", "path": "note/SKILL.md", "directory": "note", "valid": True}],
    }

    result = skill_import.apply_import(preview, preview["skills"], confirm=True)
    assert result["ok"] is True
    installed = root / "skills" / "note"
    metadata = json.loads((installed / "readmd.skill.json").read_text(encoding="utf-8"))
    assert metadata["enabled"] is False
    assert metadata["publishable"] is False


def test_local_directory_import_copies_complete_skill_and_records_source_manifest(tmp_path, monkeypatch):
    source_root = tmp_path / "bundle"
    _write_skill_bundle(source_root)
    data_root = tmp_path / "data"
    monkeypatch.setattr(skill_import, "DATA_DIR", str(data_root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(data_root / "skills.json"))

    preview = skill_import.preview_source("directory", source_root)

    assert preview["source"]["type"] == "directory"
    assert preview["skills"][0]["valid"] is True
    assert preview["skills"][0]["scripts_present"] is True
    assert len(preview["source"]["sha256"]) == 64

    result = skill_import.apply_source_import(preview, preview["skills"], confirm=True)

    assert result["ok"] is True
    installed = data_root / "skills" / "note-helper"
    assert (installed / "references" / "example.md").is_file()
    assert (installed / "tools" / "run.py").is_file()
    assert (installed / ".readmd-licenses" / "LICENSE").is_file()
    metadata = json.loads((installed / "readmd.skill.json").read_text(encoding="utf-8"))
    assert metadata["scripts_allowed"] is False
    assert metadata["source"] == "directory"
    assert {item["path"] for item in metadata["source_files"]} >= {
        "SKILL.md", "references/example.md", "tools/run.py"
    }
    assert SkillRegistry([data_root / "skills"]).get("note-helper") is not None
    source = skill_import.find_source(preview["source_id"])
    assert source["source_type"] == "directory"
    assert source["source_sha256"] == preview["source"]["sha256"]


def test_zip_import_previews_and_installs_complete_skill_directory(tmp_path, monkeypatch):
    archive_path = tmp_path / "skills.zip"
    with zipfile.ZipFile(archive_path, "w") as archive:
        archive.writestr("bundle/LICENSE", "MIT License\n")
        archive.writestr(
            "bundle/skills/outline/SKILL.md",
            "---\nname: outline\ndescription: Use when a document needs an outline.\n---\n\n"
            "Create an outline for {{document}}.\n",
        )
        archive.writestr("bundle/skills/outline/references/style.md", "# Style\n")
        archive.writestr("bundle/skills/outline/tools/run.sh", "exit 0\n")
    data_root = tmp_path / "data"
    monkeypatch.setattr(skill_import, "DATA_DIR", str(data_root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(data_root / "skills.json"))

    preview = skill_import.preview_source("zip", archive_path)

    assert preview["source"]["type"] == "zip"
    assert preview["source"]["archive_sha256"] == hashlib.sha256(archive_path.read_bytes()).hexdigest()
    assert preview["skills"][0]["valid"] is True

    result = skill_import.apply_source_import(preview, preview["skills"], confirm=True)

    installed = data_root / "skills" / "outline"
    assert result["source"]["source_type"] == "zip"
    assert (installed / "references" / "style.md").is_file()
    assert (installed / "tools" / "run.sh").is_file()
    metadata = json.loads((installed / "readmd.skill.json").read_text(encoding="utf-8"))
    assert metadata["scripts_allowed"] is False
    assert metadata["source"] == "zip"


def test_preview_marks_skill_invalid_without_license_or_with_unknown_variable(tmp_path):
    missing_license = tmp_path / "missing-license"
    folder = _write_skill_bundle(missing_license)
    (missing_license / "LICENSE").unlink()
    preview = skill_import.preview_source("directory", missing_license)
    assert preview["skills"][0]["error_codes"] == ["skill_license_missing"]

    unknown_variable = tmp_path / "unknown-variable"
    folder = _write_skill_bundle(unknown_variable)
    (folder / "SKILL.md").write_text(
        "---\nname: note-helper\ndescription: Use when a note needs cleanup.\n---\n\n"
        "Rewrite {{secret_prompt}}.\n",
        encoding="utf-8",
    )
    preview = skill_import.preview_source("directory", unknown_variable)
    assert "skill_variables_invalid" in preview["skills"][0]["error_codes"]


def test_preview_accepts_localized_skill_description(tmp_path):
    source_root = tmp_path / "localized"
    folder = _write_skill_bundle(source_root)
    skill_file = folder / "SKILL.md"
    skill_file.write_text(
        "---\nname: note-helper\ndescription: 当笔记需要整理时使用。\n---\n\nRewrite {{document}}.\n",
        encoding="utf-8",
    )
    preview = skill_import.preview_source("directory", source_root)
    assert preview["skills"][0]["valid"] is True


def test_preview_rejects_non_utf8_skill_text_resources(tmp_path):
    source_root = tmp_path / "bundle"
    folder = _write_skill_bundle(source_root)
    (folder / "references" / "invalid.md").write_bytes(b"\xff\xfe\xfa")

    preview = skill_import.preview_source("directory", source_root)

    assert preview["skills"][0]["valid"] is False
    assert "skill_resource_not_utf8" in preview["skills"][0]["error_codes"]


def test_preview_rejects_invalid_skill_metadata(tmp_path):
    source_root = tmp_path / "bundle"
    folder = _write_skill_bundle(source_root)
    (folder / "readmd.skill.json").write_text(
        json.dumps({"id": "different-id", "variables": ["document"], "required_variables": ["secret"]}),
        encoding="utf-8",
    )

    preview = skill_import.preview_source("directory", source_root)

    assert preview["skills"][0]["valid"] is False
    assert "skill_metadata_invalid" in preview["skills"][0]["error_codes"]


def test_zip_preview_rejects_special_files_and_unsafe_paths(tmp_path):
    special_path = tmp_path / "special.zip"
    with zipfile.ZipFile(special_path, "w") as archive:
        info = zipfile.ZipInfo("bundle/device")
        info.create_system = 3
        info.external_attr = ((0o010000 | 0o644) << 16)
        archive.writestr(info, "device")
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("zip", special_path)
    assert exc.value.code == "archive_special_file"

    traversal_path = tmp_path / "traversal.zip"
    with zipfile.ZipFile(traversal_path, "w") as archive:
        archive.writestr("../outside/SKILL.md", "bad")
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("zip", traversal_path)
    assert exc.value.code == "archive_path_invalid"


def test_directory_preview_enforces_symlink_file_count_size_and_depth_limits(tmp_path, monkeypatch):
    source_root = tmp_path / "bundle"
    folder = _write_skill_bundle(source_root)

    monkeypatch.setattr(skill_import, "MAX_FILE_BYTES", 4)
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("directory", source_root)
    assert exc.value.code == "source_file_too_large"

    monkeypatch.setattr(skill_import, "MAX_FILE_BYTES", 16 * 1024 * 1024)
    monkeypatch.setattr(skill_import, "MAX_FILES", 2)
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("directory", source_root)
    assert exc.value.code == "source_too_many_files"

    monkeypatch.setattr(skill_import, "MAX_FILES", 2000)
    monkeypatch.setattr(skill_import, "MAX_PATH_DEPTH", 2)
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("directory", source_root)
    assert exc.value.code == "source_path_too_deep"

    monkeypatch.setattr(skill_import, "MAX_PATH_DEPTH", 16)
    link = folder / "linked.md"
    link.write_text("link placeholder", encoding="utf-8")
    original_lstat = Path.lstat
    monkeypatch.setattr(
        Path,
        "lstat",
        lambda self: SimpleNamespace(st_mode=stat.S_IFLNK) if self == link else original_lstat(self),
    )
    with pytest.raises(skill_import.SkillImportError) as exc:
        skill_import.preview_source("directory", source_root)
    assert exc.value.code == "source_symlink"


def test_conflicts_skip_by_default_and_support_replace_with_backup_and_rename(tmp_path, monkeypatch):
    source_root = tmp_path / "bundle"
    source_folder = _write_skill_bundle(source_root)
    data_root = tmp_path / "data"
    installed = data_root / "skills" / "note-helper"
    _write_skill_bundle(data_root / "skills", "note-helper")
    # _write_skill_bundle nests under skills/, so install the fixture at the runtime root.
    nested = data_root / "skills" / "skills" / "note-helper"
    installed.parent.mkdir(parents=True, exist_ok=True)
    nested.replace(installed)
    (data_root / "skills" / "LICENSE").unlink()
    (data_root / "skills" / "skills").rmdir()
    (installed / "references" / "marker.md").write_text("old\n", encoding="utf-8")
    (source_folder / "references" / "marker.md").write_text("new\n", encoding="utf-8")
    monkeypatch.setattr(skill_import, "DATA_DIR", str(data_root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(data_root / "skills.json"))
    preview = skill_import.preview_source("directory", source_root)

    skipped = skill_import.apply_source_import(preview, preview["skills"], confirm=True)
    assert skipped["skills"] == []
    assert skipped["skipped"] == [{"id": "note-helper", "reason": "conflict"}]
    assert (installed / "references" / "marker.md").read_text(encoding="utf-8") == "old\n"

    replace_selection = [{**preview["skills"][0], "conflict_action": "replace"}]
    replaced = skill_import.apply_source_import(preview, replace_selection, confirm=True)
    assert replaced["skills"][0]["id"] == "note-helper"
    assert (installed / "references" / "marker.md").read_text(encoding="utf-8") == "new\n"
    backups = list((data_root / "skills" / ".versions" / "note-helper").glob("directory-*"))
    assert len(backups) == 1
    assert (backups[0] / "references" / "marker.md").read_text(encoding="utf-8") == "old\n"

    rename_selection = [{**preview["skills"][0], "conflict_action": "rename"}]
    renamed = skill_import.apply_source_import(preview, rename_selection, confirm=True)
    assert renamed["skills"][0]["id"] == "note-helper-imported"
    assert SkillRegistry([data_root / "skills"]).get("note-helper-imported") is not None


def test_failed_replace_restores_the_previously_installed_skill(tmp_path, monkeypatch):
    source_root = tmp_path / "bundle"
    source_folder = _write_skill_bundle(source_root)
    (source_folder / "references" / "marker.md").write_text("new\n", encoding="utf-8")
    data_root = tmp_path / "data"
    _write_skill_bundle(data_root / "skills", "note-helper")
    installed = data_root / "skills" / "note-helper"
    nested = data_root / "skills" / "skills" / "note-helper"
    nested.replace(installed)
    (data_root / "skills" / "LICENSE").unlink()
    (data_root / "skills" / "skills").rmdir()
    (installed / "references" / "marker.md").write_text("old\n", encoding="utf-8")
    monkeypatch.setattr(skill_import, "DATA_DIR", str(data_root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(data_root / "skills.json"))
    preview = skill_import.preview_source("directory", source_root)
    selection = [{**preview["skills"][0], "conflict_action": "replace"}]
    monkeypatch.setattr(SkillRegistry, "validate", lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("invalid")))

    with pytest.raises(RuntimeError):
        skill_import.apply_source_import(preview, selection, confirm=True)

    assert (installed / "references" / "marker.md").read_text(encoding="utf-8") == "old\n"


def test_saved_local_source_check_uses_original_adapter(tmp_path):
    source_root = tmp_path / "bundle"
    _write_skill_bundle(source_root)
    preview = skill_import.preview_source("directory", source_root)
    saved = {
        "source_type": "directory",
        "source_path": str(source_root.resolve()),
        "source_sha256": preview["source"]["sha256"],
    }
    refreshed = skill_import.preview_saved_source(saved)
    assert skill_import.source_preview_changed(saved, refreshed) is False

    (source_root / "skills" / "note-helper" / "references" / "example.md").write_text(
        "# Changed\n", encoding="utf-8"
    )
    changed = skill_import.preview_saved_source(saved)
    assert skill_import.source_preview_changed(saved, changed) is True


def _write_installed_skill(skills_dir: Path, skill_id: str = "note-helper") -> Path:
    folder = skills_dir / skill_id
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "references").mkdir(parents=True, exist_ok=True)
    (folder / "tools").mkdir(parents=True, exist_ok=True)
    (folder / "SKILL.md").write_text(
        "---\n"
        f"name: {skill_id}\n"
        "description: Use when a note needs cleanup.\n"
        "---\n\n"
        "Rewrite {{document}} for {{language}}.\n",
        encoding="utf-8",
    )
    (folder / "references" / "marker.md").write_text("old\n", encoding="utf-8")
    (folder / "tools" / "run.py").write_text("print('disabled')\n", encoding="utf-8")
    return folder


def test_failed_local_replace_copytree_restores_installed_skill(tmp_path, monkeypatch):
    source_root = tmp_path / "bundle"
    source_folder = _write_skill_bundle(source_root)
    (source_folder / "references" / "marker.md").write_text("new\n", encoding="utf-8")
    data_root = tmp_path / "data"
    installed = _write_installed_skill(data_root / "skills", "note-helper")
    monkeypatch.setattr(skill_import, "DATA_DIR", str(data_root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(data_root / "skills.json"))
    preview = skill_import.preview_source("directory", source_root)
    selection = [{**preview["skills"][0], "conflict_action": "replace"}]
    real_copytree = skill_import.shutil.copytree
    resolved_source = source_folder.resolve()

    def failing_copytree(*args, **kwargs):
        if Path(args[0]).resolve() == resolved_source:
            raise OSError("disk full during install")
        return real_copytree(*args, **kwargs)

    monkeypatch.setattr(skill_import.shutil, "copytree", failing_copytree)

    with pytest.raises(OSError):
        skill_import.apply_source_import(preview, selection, confirm=True)

    assert installed.is_dir(), "previously installed skill must be restored after install failure"
    assert (installed / "references" / "marker.md").read_text(encoding="utf-8") == "old\n"
    assert (installed / "SKILL.md").is_file()


def test_failed_github_replace_copytree_restores_installed_skill(tmp_path, monkeypatch):
    root = tmp_path / "data"
    installed = _write_installed_skill(root / "skills", "note-helper")
    monkeypatch.setattr(skill_import, "DATA_DIR", str(root))
    monkeypatch.setattr(skill_import, "SKILLS_FILE", str(root / "skills.json"))
    archive_bytes = io.BytesIO()
    with zipfile.ZipFile(archive_bytes, "w") as archive:
        archive.writestr(
            "owner-repo-sha/SKILL.md",
            "---\nname: note-helper\ndescription: Use when a note needs cleanup.\n---\n\nRewrite {{document}}.\n",
        )
        archive.writestr("owner-repo-sha/tools/run.py", "print('disabled')\n")
        archive.writestr("owner-repo-sha/LICENSE", "MIT License\n")
    monkeypatch.setattr(
        skill_import,
        "_resolve",
        lambda parsed, token: (
            "a" * 40,
            {"full_name": "owner/repo"},
            [
                {"type": "blob", "path": "SKILL.md"},
                {"type": "blob", "path": "tools/run.py"},
                {"type": "blob", "path": "LICENSE"},
            ],
        ),
    )
    monkeypatch.setattr(skill_import, "_request", lambda url, token="", **kwargs: archive_bytes.getvalue())
    source = {"type": "github", "owner": "owner", "repo": "repo", "ref": "", "subdir": "", "canonical_url": "https://github.com/owner/repo"}
    preview = {
        "source_id": "gh-test",
        "source": {**source, "resolved_commit": "a" * 40},
        "license_files": ["LICENSE"],
        "skills": [{"id": "note-helper", "path": "SKILL.md", "directory": "", "valid": True}],
    }
    selection = [{**preview["skills"][0], "conflict_action": "replace"}]
    real_copytree = skill_import.shutil.copytree

    def failing_copytree(*args, **kwargs):
        if "owner-repo-sha" in Path(args[0]).name:
            raise OSError("disk full during install")
        return real_copytree(*args, **kwargs)

    monkeypatch.setattr(skill_import.shutil, "copytree", failing_copytree)

    with pytest.raises(OSError):
        skill_import.apply_import(preview, selection, confirm=True)

    assert installed.is_dir(), "previously installed skill must be restored after install failure"
    assert (installed / "references" / "marker.md").read_text(encoding="utf-8") == "old\n"
    assert (installed / "SKILL.md").is_file()
