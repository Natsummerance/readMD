# -*- coding: utf-8 -*-
"""Safe, offline-first GitHub Skill importer.

The importer deliberately treats a repository as untrusted data.  It never
executes git, hooks or repository scripts; it resolves a commit through the
GitHub API, previews the discovered ``SKILL.md`` files, then copies only the
user-confirmed Skill directories into ReadMD's user scope.
"""
from __future__ import annotations

import base64
import hashlib
import io
import json
import os
import posixpath
import re
import shutil
import tempfile
import urllib.error
import urllib.parse
import urllib.request
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional, Tuple

from ..readmd_core.config import DATA_DIR, SKILLS_FILE
from ..readmd_core.file_writer import save_text_atomic
from ..readmd_core.utils import load_json, save_json
from .crypto import load_credential
from .skills import SkillError, SkillRegistry


GITHUB_HOSTS = frozenset({"github.com", "www.github.com"})
GITHUB_API_HOST = "api.github.com"
MAX_RESPONSE_BYTES = 64 * 1024 * 1024
MAX_EXTRACTED_BYTES = 128 * 1024 * 1024
MAX_FILES = 2000
MAX_PATH_LENGTH = 240
_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")


class SkillImportError(ValueError):
    """Raised for invalid URLs, unsafe archives or rejected imports."""

    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


def _slug(value: str) -> str:
    value = re.sub(r"[^a-z0-9]+", "-", str(value or "").lower()).strip("-")
    return value[:64].rstrip("-") or "imported-skill"


def parse_github_url(value: str) -> Dict[str, str]:
    """Normalize repository/tree/blob URLs without accepting credentials."""
    raw = str(value or "").strip()
    parsed = urllib.parse.urlparse(raw)
    if parsed.scheme.lower() != "https" or (parsed.hostname or "").lower() not in GITHUB_HOSTS:
        raise SkillImportError("github_host_not_allowed", "仅允许 HTTPS GitHub 仓库链接")
    if parsed.username or parsed.password or parsed.query:
        raise SkillImportError("github_url_has_credentials", "仓库链接不得包含凭据或查询参数")
    segments = [urllib.parse.unquote(x) for x in parsed.path.split("/") if x]
    if len(segments) < 2:
        raise SkillImportError("github_repo_invalid", "GitHub 链接必须包含 owner/repository")
    owner, repo = segments[0], segments[1]
    if repo.endswith(".git"):
        repo = repo[:-4]
    if not re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", owner) or not re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", repo):
        raise SkillImportError("github_repo_invalid", "GitHub owner 或仓库名称无效")
    ref, subdir = "", ""
    marker = ""
    ref_tail: List[str] = []
    if len(segments) > 2:
        marker = segments[2].lower()
        if marker not in ("tree", "blob") or len(segments) < 4:
            raise SkillImportError("github_url_invalid", "仅支持仓库、tree 或 blob 链接")
        # GitHub does not encode where a slash-containing branch name ends.
        # Keep the complete tail so _resolve can probe ref candidates from
        # longest to shortest and then derive the actual subdirectory.
        ref_tail = segments[3:]
        ref = ref_tail[0]
        remaining = ref_tail[1:]
        subdir = "/".join(remaining)
        if marker == "blob" and remaining and remaining[-1].lower() == "skill.md":
            subdir = "/".join(remaining[:-1])
    if subdir.startswith("/") or ".." in subdir.split("/"):
        raise SkillImportError("github_path_invalid", "仓库子路径无效")
    canonical = "https://github.com/%s/%s" % (owner, repo)
    if ref:
        canonical += "/tree/" + urllib.parse.quote(ref, safe="")
        if subdir:
            canonical += "/" + "/".join(urllib.parse.quote(x, safe="") for x in subdir.split("/"))
    return {
        "owner": owner,
        "repo": repo,
        "ref": ref,
        "subdir": subdir,
        "canonical_url": canonical,
        "_ref_tail": ref_tail,
        "_marker": marker,
    }


def _token(credential_id: str) -> str:
    if not credential_id:
        return ""
    try:
        return load_credential(credential_id)
    except Exception as exc:
        raise SkillImportError("credential_invalid", "GitHub 凭据不可用") from exc


def _safe_url(url: str, api: bool = False) -> str:
    parsed = urllib.parse.urlparse(url)
    allowed = {GITHUB_API_HOST} if api else {GITHUB_API_HOST, "codeload.github.com"}
    if parsed.scheme != "https" or (parsed.hostname or "").lower() not in allowed:
        raise SkillImportError("github_redirect_blocked", "GitHub 请求发生了不受信任的跳转")
    return url


class _SafeRedirectHandler(urllib.request.HTTPRedirectHandler):
    """Reject redirects before urllib opens an untrusted host."""

    def __init__(self, api: bool):
        super().__init__()
        self.api = api

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        _safe_url(newurl, api=self.api)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def _request(url: str, token: str = "", *, api: bool = True, limit: int = MAX_RESPONSE_BYTES) -> bytes:
    _safe_url(url, api=api)
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "ReadMD-Skill-Importer"}
    if token:
        headers["Authorization"] = "Bearer " + token
    req = urllib.request.Request(url, headers=headers)
    try:
        opener = urllib.request.build_opener(_SafeRedirectHandler(api))
        with opener.open(req, timeout=25) as response:
            _safe_url(response.geturl(), api=api)
            data = response.read(limit + 1)
    except urllib.error.HTTPError as exc:
        if exc.code == 429 or (exc.code == 403 and exc.headers.get("X-RateLimit-Remaining") == "0"):
            raise SkillImportError("github_rate_limited", "GitHub 请求已达到速率限制，请稍后重试") from exc
        if exc.code in (401, 403):
            raise SkillImportError("github_auth_failed", "GitHub 凭据无权读取此仓库") from exc
        if exc.code in (404, 422):
            raise SkillImportError("github_not_found", "仓库、分支或文件不存在，或仓库为私有") from exc
        raise SkillImportError("github_http_error", "GitHub 请求失败（HTTP %s）" % exc.code) from exc
    except (urllib.error.URLError, TimeoutError) as exc:
        raise SkillImportError("github_network_error", "无法连接 GitHub，请检查网络后重试") from exc
    if len(data) > limit:
        raise SkillImportError("github_response_too_large", "GitHub 响应超过安全大小限制")
    return data


def _json(url: str, token: str = "") -> Any:
    try:
        return json.loads(_request(url, token).decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise SkillImportError("github_response_invalid", "GitHub 返回了无效数据") from exc


def _api_base(source: Mapping[str, str]) -> str:
    return "https://api.github.com/repos/%s/%s" % (source["owner"], source["repo"])


def _resolve(source: Mapping[str, str], token: str) -> Tuple[str, Dict[str, Any], List[Dict[str, Any]]]:
    base = _api_base(source)
    repo = _json(base, token)
    ref = source.get("ref") or str(repo.get("default_branch") or "main")
    ref_tail = list(source.get("_ref_tail") or [])
    candidates = []
    if ref_tail:
        # Prefer the longest existing ref, which makes URLs such as
        # tree/feature/docs/skills resolve to branch feature/docs when it
        # exists, while still falling back to branch feature + subdirectory
        # docs/skills.  A missing candidate is expected and is not surfaced.
        candidates = ["/".join(ref_tail[:index]) for index in range(len(ref_tail), 0, -1)]
    else:
        candidates = [ref]
    commit = None
    resolved_ref = ref
    for candidate in candidates:
        try:
            commit = _json(base + "/commits/" + urllib.parse.quote(candidate, safe=""), token)
            if isinstance(commit, dict) and commit.get("sha"):
                resolved_ref = candidate
                break
        except SkillImportError as exc:
            if exc.code == "github_not_found":
                continue
            raise
    if not isinstance(commit, dict):
        raise SkillImportError("github_commit_invalid", "无法解析仓库提交")
    if isinstance(source, dict):
        source["ref"] = resolved_ref
        if ref_tail:
            remainder = ref_tail[len(resolved_ref.split("/")):]
            if source.get("_marker") == "blob" and remainder and remainder[-1].lower() == "skill.md":
                remainder = remainder[:-1]
            source["subdir"] = "/".join(remainder)
            canonical = "https://github.com/%s/%s/tree/%s" % (
                source["owner"], source["repo"], urllib.parse.quote(resolved_ref, safe=""))
            if source["subdir"]:
                canonical += "/" + "/".join(urllib.parse.quote(item, safe="") for item in source["subdir"].split("/"))
            source["canonical_url"] = canonical
    sha = str(commit.get("sha") or "")
    if not re.fullmatch(r"[0-9a-fA-F]{7,64}", sha):
        raise SkillImportError("github_commit_invalid", "无法解析仓库提交")
    tree = _json(base + "/git/trees/" + sha + "?recursive=1", token)
    entries = tree.get("tree") if isinstance(tree, dict) else None
    if not isinstance(entries, list):
        raise SkillImportError("github_tree_invalid", "GitHub 仓库目录树无效")
    if tree.get("truncated"):
        raise SkillImportError("github_tree_truncated", "仓库过大，无法安全扫描全部 Skill")
    return sha, repo, entries


def _under(path: str, prefix: str) -> bool:
    return not prefix or path == prefix or path.startswith(prefix.rstrip("/") + "/")


def _content(source: Mapping[str, str], sha: str, path: str, token: str) -> str:
    url = _api_base(source) + "/contents/" + "/".join(urllib.parse.quote(x, safe="") for x in path.split("/"))
    data = _json(url + "?ref=" + urllib.parse.quote(sha, safe=""), token)
    encoded = data.get("content") if isinstance(data, dict) else None
    if not isinstance(encoded, str) or data.get("encoding") != "base64":
        raise SkillImportError("github_skill_unreadable", "无法读取 SKILL.md")
    try:
        return base64.b64decode(encoded.encode("ascii"), validate=False).decode("utf-8")
    except (ValueError, UnicodeDecodeError) as exc:
        raise SkillImportError("github_skill_unreadable", "SKILL.md 不是有效 UTF-8 文本") from exc


def _frontmatter(text: str) -> Tuple[str, str]:
    match = re.match(r"^---\s*\r?\n(.*?)\r?\n---\s*\r?\n", text, re.S)
    values: Dict[str, str] = {}
    if match:
        for line in match.group(1).splitlines():
            if ":" in line:
                key, value = line.split(":", 1)
                values[key.strip().lower()] = value.strip().strip("'\"")
    return values.get("name", ""), values.get("description", "")


def preview_import(url: str, credential_id: str = "") -> Dict[str, Any]:
    source = parse_github_url(url)
    token = _token(credential_id)
    sha, repo, entries = _resolve(source, token)
    prefix = source.get("subdir", "").strip("/")
    skills: List[Dict[str, Any]] = []
    paths = [str(item.get("path") or "") for item in entries if item.get("type") == "blob"]
    for path in paths:
        if not path.lower().endswith("/skill.md") and path.lower() != "skill.md":
            continue
        if not _under(path, prefix):
            continue
        folder = posixpath.dirname(path)
        try:
            text = _content(source, sha, path, token)
            name, description = _frontmatter(text)
        except SkillImportError as exc:
            skills.append({"path": path, "id": _slug(posixpath.basename(folder)), "valid": False, "error_code": exc.code})
            continue
        skill_id = name if _ID_RE.fullmatch(name) else _slug(posixpath.basename(folder) or "root")
        file_list = sorted(p for p in paths if _under(p, folder))
        skills.append({
            "id": skill_id,
            "path": path,
            "directory": folder,
            "name": name or skill_id,
            "description": description,
            "files": file_list,
            "valid": bool(name and description),
            "scripts_present": any(Path(p).suffix.lower() in {".py", ".js", ".mjs", ".sh", ".ps1", ".bat", ".cmd"} for p in file_list),
        })
    if not skills:
        raise SkillImportError("skill_not_found", "仓库中没有可导入的 SKILL.md")
    license_paths = [p for p in paths if posixpath.basename(p).lower().startswith("license") or posixpath.basename(p).lower() == "notice"]
    source_id = "gh-" + hashlib.sha256(source["canonical_url"].encode("utf-8")).hexdigest()[:20]
    return {
        "source_id": source_id,
        "source": {**source, "resolved_commit": sha, "repository_name": repo.get("full_name", "")},
        "license_files": license_paths,
        "skills": skills,
        "offline_copy": False,
        "credential_required": bool(repo.get("private")) and not bool(token),
    }


def _load_sources() -> Dict[str, Any]:
    data = load_json(SKILLS_FILE, {})
    if not isinstance(data, dict) or data.get("schema_version") != 1:
        return {"schema_version": 1, "sources": []}
    if not isinstance(data.get("sources"), list):
        data["sources"] = []
    return data


def _save_sources(data: Mapping[str, Any]) -> None:
    if not save_json(SKILLS_FILE, dict(data)):
        raise SkillImportError("config_write_failed", "无法保存 Skill 来源配置")


def _safe_member(name: str) -> str:
    name = name.replace("\\", "/")
    if not name or name.startswith("/") or ":" in name or any(part in ("", ".", "..") for part in name.split("/")):
        raise SkillImportError("archive_path_invalid", "归档包含不安全路径")
    if len(name) > MAX_PATH_LENGTH:
        raise SkillImportError("archive_path_too_long", "归档路径过长")
    return name


def _extract(blob: bytes) -> Path:
    temp = Path(tempfile.mkdtemp(prefix="readmd-skill-import-"))
    try:
        with zipfile.ZipFile(io.BytesIO(blob)) as archive:
            infos = archive.infolist()
            if len(infos) > MAX_FILES:
                raise SkillImportError("archive_too_many_files", "归档文件数量超过限制")
            total = 0
            for info in infos:
                name = _safe_member(info.filename.rstrip("/")) if info.filename.rstrip("/") else ""
                if not name:
                    continue
                # ZIP mode bits mark symlinks; never materialize them.
                if ((info.external_attr >> 16) & 0o170000) == 0o120000:
                    raise SkillImportError("archive_symlink", "归档包含不允许的符号链接")
                total += int(info.file_size or 0)
                if total > MAX_EXTRACTED_BYTES:
                    raise SkillImportError("archive_too_large", "归档展开后超过安全大小限制")
                target = temp / name
                target.parent.mkdir(parents=True, exist_ok=True)
                if info.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                with archive.open(info, "r") as src, open(target, "wb") as dst:
                    shutil.copyfileobj(src, dst, length=1024 * 1024)
        return temp
    except Exception:
        shutil.rmtree(temp, ignore_errors=True)
        raise


def _archive_root(root: Path) -> Path:
    children = [p for p in root.iterdir() if p.name != "__MACOSX"]
    if len(children) == 1 and children[0].is_dir():
        return children[0]
    return root


def _skill_file(folder: Path) -> Optional[Path]:
    """Find the conventional Skill entry file without trusting case."""
    for item in folder.iterdir() if folder.is_dir() else ():
        if item.is_file() and item.name.lower() == "skill.md":
            return item
    return None


def apply_import(preview: Mapping[str, Any], selections: Iterable[Mapping[str, Any]], credential_id: str = "", confirm: bool = False) -> Dict[str, Any]:
    if not confirm:
        raise SkillImportError("confirmation_required", "导入 Skill 需要明确确认")
    source = preview.get("source") if isinstance(preview.get("source"), dict) else {}
    parsed = parse_github_url(str(source.get("canonical_url") or ""))
    token = _token(credential_id)
    sha, _, entries = _resolve(parsed, token)
    expected = str(source.get("resolved_commit") or "")
    if expected and sha != expected:
        raise SkillImportError("source_changed", "预览后仓库已发生变化，请重新预览")
    archive_url = _api_base(parsed) + "/zipball/" + sha
    extracted = _extract(_request(archive_url, token, api=False))
    root = _archive_root(extracted)
    allowed_paths = {
        str(item.get("path") or "").replace("\\", "/")
        for item in entries
        if item.get("type") == "blob"
    }
    imported: List[Dict[str, Any]] = []
    config = _load_sources()
    try:
        for selected in selections:
            if not isinstance(selected, Mapping) or selected.get("valid") is False:
                continue
            path = str(selected.get("path") or "").replace("\\", "/")
            directory = str(selected.get("directory") or posixpath.dirname(path)).strip("/")
            if not directory:
                directory = ""
            if path not in allowed_paths or not path.lower().endswith("/skill.md") and path.lower() != "skill.md":
                raise SkillImportError("skill_path_invalid", "选中的 Skill 路径不在预览清单中")
            if directory and (".." in directory.split("/") or not _under(path, directory)):
                raise SkillImportError("skill_path_invalid", "选中的 Skill 目录无效")
            if directory != posixpath.dirname(path):
                raise SkillImportError("skill_path_invalid", "选中的 Skill 目录与文件路径不匹配")
            source_dir = root / directory
            source_skill_file = _skill_file(source_dir)
            if not source_dir.is_dir() or source_skill_file is None:
                raise SkillImportError("skill_missing", "选中的 Skill 目录不完整")
            skill_id = str(selected.get("id") or _slug(source_dir.name))
            if not _ID_RE.fullmatch(skill_id):
                skill_id = _slug(skill_id)
            destination = Path(DATA_DIR) / "skills" / skill_id
            action = str(selected.get("conflict_action") or "skip").lower()
            if destination.exists():
                if action == "skip":
                    continue
                if action not in ("replace", "rename"):
                    raise SkillImportError("skill_conflict", "Skill 已存在，请选择跳过、重命名或替换")
                if action == "rename":
                    skill_id = _slug(skill_id + "-imported")
                    destination = Path(DATA_DIR) / "skills" / skill_id
                else:
                    backup = Path(DATA_DIR) / "skills" / ".versions" / skill_id / ("github-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(destination), str(backup))
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copytree(source_dir, destination)
            imported_skill_file = _skill_file(destination)
            if imported_skill_file is None:
                shutil.rmtree(destination, ignore_errors=True)
                raise SkillImportError("skill_missing", "选中的 Skill 目录不完整")
            if imported_skill_file.name != "SKILL.md":
                imported_skill_file.rename(destination / "SKILL.md")
            # A renamed destination must also have a matching frontmatter id;
            # otherwise SkillRegistry correctly indexes it under the upstream
            # name and the imported record would point at a non-existent id.
            skill_file = destination / "SKILL.md"
            original_skill_text = skill_file.read_text(encoding="utf-8")
            if skill_id != str(selected.get("id") or ""):
                original_name = re.search(r"^(name:\s*)([^\r\n]+)", original_skill_text, re.M)
                if original_name:
                    skill_file.write_text(
                        original_skill_text[:original_name.start(2)] + skill_id + original_skill_text[original_name.end(2):],
                        encoding="utf-8", newline="\n",
                    )
            source_files = []
            for source_file in sorted(p for p in destination.rglob("*") if p.is_file()):
                if source_file.name == "readmd.skill.json":
                    continue
                digest = hashlib.sha256(source_file.read_bytes()).hexdigest()
                source_files.append({"path": source_file.relative_to(destination).as_posix(), "sha256": digest})
            source_hash = _directory_sha256(destination)
            metadata = {
                "id": skill_id,
                "scope": "user",
                "enabled": True,
                "scripts_allowed": False,
                "source": "github",
                "provenance": {"repository": parsed["canonical_url"], "commit": sha, "path": path},
                "source_files": source_files,
                "source_sha256": source_hash,
                "license": ", ".join(str(x) for x in preview.get("license_files", [])),
                "adaptation_notes": ["Imported from GitHub as data; scripts remain disabled."],
            }
            save_text_atomic(str(destination / "readmd.skill.json"), json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
            # Validate the on-disk structure directly after writing metadata.
            try:
                check = SkillRegistry([destination.parent]).validate(destination)
            except SkillError:
                check = None
            if check is None:
                shutil.rmtree(destination, ignore_errors=True)
                raise SkillImportError("skill_invalid", "导入后 Skill 未通过结构校验")
            imported.append({"id": skill_id, "path": path, "sha256": source_hash})
        if not imported:
            raise SkillImportError("nothing_imported", "没有导入任何 Skill")
        source_id = preview.get("source_id") or "gh-" + hashlib.sha256(parsed["canonical_url"].encode()).hexdigest()[:20]
        previous = next((s for s in config.get("sources", []) if s.get("source_id") == source_id), None)
        merged_skills = {str(item.get("id")): item for item in (previous or {}).get("skills", []) if isinstance(item, Mapping)}
        merged_skills.update({str(item.get("id")): item for item in imported})
        source_record = {
            "source_id": source_id,
            "repository_url": parsed["canonical_url"],
            "owner": parsed["owner"], "repo": parsed["repo"], "subdir": parsed.get("subdir", ""),
            "requested_ref": parsed.get("ref", ""), "resolved_commit": sha,
            "credential_id": credential_id or "", "update_policy": "manual",
            "skills": list(merged_skills.values()), "imported_at": datetime.now(timezone.utc).isoformat(),
        }
        config["sources"] = [s for s in config.get("sources", []) if s.get("source_id") != source_record["source_id"]]
        config["sources"].append(source_record)
        _save_sources(config)
        return {"ok": True, "source": source_record, "skills": imported}
    finally:
        shutil.rmtree(extracted, ignore_errors=True)


def _directory_sha256(directory: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(p for p in directory.rglob("*") if p.is_file() and p.name != "readmd.skill.json"):
        digest.update(path.relative_to(directory).as_posix().encode("utf-8"))
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def list_sources() -> List[Dict[str, Any]]:
    return list(_load_sources().get("sources", []))


def find_source(source_id: str) -> Optional[Dict[str, Any]]:
    return next((s for s in list_sources() if s.get("source_id") == source_id), None)


def remove_source(source_id: str) -> bool:
    """Remove only the source binding; imported Skill files remain intact."""
    data = _load_sources()
    before = len(data.get("sources", []))
    data["sources"] = [s for s in data.get("sources", []) if s.get("source_id") != source_id]
    if len(data["sources"]) == before:
        return False
    _save_sources(data)
    return True


__all__ = ["SkillImportError", "parse_github_url", "preview_import", "apply_import", "list_sources", "find_source", "remove_source"]
