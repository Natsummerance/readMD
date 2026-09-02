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
import stat
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
MAX_PATH_DEPTH = 16
MAX_FILE_BYTES = 16 * 1024 * 1024
_ID_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
_VARIABLE_RE = re.compile(r"\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}")
_ALLOWED_VARIABLES = {"document", "selection", "request", "language", "context", "output_format"}
_SCRIPT_SUFFIXES = {".py", ".js", ".mjs", ".cjs", ".sh", ".ps1", ".bat", ".cmd"}
_TEXT_SUFFIXES = {
    ".md", ".markdown", ".txt", ".json", ".yaml", ".yml", ".toml", ".ini", ".cfg",
    ".py", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx", ".sh", ".ps1", ".bat",
    ".cmd", ".html", ".css", ".xml", ".csv",
}


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


def _is_license_path(path: str) -> bool:
    name = posixpath.basename(path).lower()
    return name.startswith(("license", "licence", "copying")) or name == "notice" or name.startswith("notice.")


def _manifest_sha256(files: Iterable[Mapping[str, Any]]) -> str:
    digest = hashlib.sha256()
    for item in sorted(files, key=lambda value: str(value.get("path") or "")):
        digest.update(str(item.get("path") or "").encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(item.get("sha256") or "").encode("ascii"))
        digest.update(b"\0")
    return digest.hexdigest()


def _scan_directory(root: Path) -> List[Dict[str, Any]]:
    """Return a deterministic regular-file manifest for an untrusted tree."""
    root = root.expanduser()
    if root.is_symlink():
        raise SkillImportError("source_symlink", "Skill 来源目录不能是符号链接")
    if not root.is_dir():
        raise SkillImportError("source_not_found", "Skill 来源目录不存在")
    root = root.resolve()
    files: List[Dict[str, Any]] = []
    total = 0
    for current, directories, filenames in os.walk(root, topdown=True, followlinks=False):
        current_path = Path(current)
        for name in list(directories):
            candidate = current_path / name
            mode = candidate.lstat().st_mode
            if stat.S_ISLNK(mode):
                raise SkillImportError("source_symlink", "Skill 来源包含不允许的符号链接")
            if not stat.S_ISDIR(mode):
                raise SkillImportError("source_special_file", "Skill 来源包含不支持的特殊目录项")
            relative = candidate.relative_to(root).as_posix()
            if len(relative) > MAX_PATH_LENGTH:
                raise SkillImportError("source_path_too_long", "Skill 来源路径过长")
            if len(Path(relative).parts) > MAX_PATH_DEPTH:
                raise SkillImportError("source_path_too_deep", "Skill 来源目录层级超过限制")
        for name in filenames:
            candidate = current_path / name
            mode = candidate.lstat().st_mode
            if stat.S_ISLNK(mode):
                raise SkillImportError("source_symlink", "Skill 来源包含不允许的符号链接")
            if not stat.S_ISREG(mode):
                raise SkillImportError("source_special_file", "Skill 来源包含不支持的特殊文件")
            relative = candidate.relative_to(root).as_posix()
            if len(relative) > MAX_PATH_LENGTH:
                raise SkillImportError("source_path_too_long", "Skill 来源路径过长")
            if len(Path(relative).parts) > MAX_PATH_DEPTH:
                raise SkillImportError("source_path_too_deep", "Skill 来源目录层级超过限制")
            size = candidate.stat().st_size
            if size > MAX_FILE_BYTES:
                raise SkillImportError("source_file_too_large", "Skill 来源包含超过大小限制的文件")
            total += size
            if total > MAX_EXTRACTED_BYTES:
                raise SkillImportError("source_too_large", "Skill 来源总大小超过限制")
            if len(files) >= MAX_FILES:
                raise SkillImportError("source_too_many_files", "Skill 来源文件数量超过限制")
            digest = hashlib.sha256()
            with open(candidate, "rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
            files.append({"path": relative, "size": size, "sha256": digest.hexdigest()})
    return sorted(files, key=lambda item: item["path"])


def _applicable_licenses(directory: str, license_paths: Iterable[str]) -> List[str]:
    directory = directory.strip("/")
    result = []
    for path in license_paths:
        parent = posixpath.dirname(path).strip("/")
        if not parent or directory == parent or directory.startswith(parent + "/"):
            result.append(path)
    return sorted(result)


def _scan_skill_candidates(root: Path, manifest: Iterable[Mapping[str, Any]]) -> Tuple[List[Dict[str, Any]], List[str]]:
    manifest_items = [dict(item) for item in manifest]
    paths = [str(item.get("path") or "") for item in manifest_items]
    by_path = {str(item.get("path") or ""): item for item in manifest_items}
    license_paths = sorted(path for path in paths if _is_license_path(path))
    skills: List[Dict[str, Any]] = []
    for path in paths:
        if posixpath.basename(path).lower() != "skill.md":
            continue
        directory = posixpath.dirname(path)
        skill_file = root / Path(path)
        errors: List[str] = []
        try:
            text = skill_file.read_text(encoding="utf-8")
        except (OSError, UnicodeError):
            text = ""
            errors.append("skill_not_utf8")
        name, description = _frontmatter(text)
        if not _ID_RE.fullmatch(name):
            errors.append("skill_name_invalid")
        if not description:
            errors.append("skill_description_invalid")
        if set(_VARIABLE_RE.findall(text)) - _ALLOWED_VARIABLES:
            errors.append("skill_variables_invalid")
        sidecar_path = posixpath.join(directory, "readmd.skill.json") if directory else "readmd.skill.json"
        if sidecar_path in by_path:
            try:
                sidecar = json.loads((root / Path(sidecar_path)).read_text(encoding="utf-8"))
                if not isinstance(sidecar, dict):
                    raise ValueError
                declared = sidecar.get("variables")
                required = sidecar.get("required_variables")
                metadata_id = str(sidecar.get("id") or name)
                invalid_declared = declared is not None and (
                    not isinstance(declared, list) or set(map(str, declared)) - _ALLOWED_VARIABLES
                )
                invalid_required = required is not None and (
                    not isinstance(required, list) or set(map(str, required)) - _ALLOWED_VARIABLES
                )
                if isinstance(declared, list) and isinstance(required, list):
                    invalid_required = invalid_required or bool(set(map(str, required)) - set(map(str, declared)))
                if metadata_id != name or not _ID_RE.fullmatch(metadata_id) or invalid_declared or invalid_required:
                    errors.append("skill_metadata_invalid")
            except (OSError, UnicodeError, json.JSONDecodeError, ValueError, TypeError):
                errors.append("skill_metadata_invalid")
        applicable_licenses = _applicable_licenses(directory, license_paths)
        if not applicable_licenses:
            errors.append("skill_license_missing")
        file_items = [dict(item) for item in manifest_items if _under(str(item.get("path") or ""), directory)]
        text_paths = {str(item.get("path") or "") for item in file_items if Path(str(item.get("path") or "")).suffix.lower() in _TEXT_SUFFIXES}
        text_paths.update(applicable_licenses)
        for text_path in sorted(text_paths):
            try:
                (root / Path(text_path)).read_text(encoding="utf-8")
            except (OSError, UnicodeError):
                errors.append("skill_resource_not_utf8")
                break
        skill_id = name if _ID_RE.fullmatch(name) else _slug(posixpath.basename(directory) or "root")
        # A missing license is reviewable data, not an unsafe archive.  Keep
        # the preview selectable as a disabled draft; malformed metadata,
        # unknown variables and unreadable files remain hard blockers.
        blocking_errors = [item for item in errors if item != "skill_license_missing"]
        skills.append({
            "id": skill_id,
            "path": path,
            "directory": directory,
            "name": name or skill_id,
            "description": description,
            "files": [item["path"] for item in file_items],
            "source_files": file_items,
            "license_files": applicable_licenses,
            "valid": not errors,
            "draft_allowed": bool(blocking_errors == [] and errors),
            "publishable": not errors,
            "error_code": errors[0] if errors else "",
            "error_codes": errors,
            "scripts_present": any(Path(item["path"]).suffix.lower() in _SCRIPT_SUFFIXES for item in file_items),
        })
    return skills, license_paths


def _preview_directory(path: os.PathLike[str] | str) -> Dict[str, Any]:
    root = Path(path).expanduser()
    manifest = _scan_directory(root)
    root = root.resolve()
    skills, license_paths = _scan_skill_candidates(root, manifest)
    if not skills:
        raise SkillImportError("skill_not_found", "来源中没有可导入的 SKILL.md")
    source_hash = _manifest_sha256(manifest)
    identity = str(root)
    return {
        "source_id": "dir-" + hashlib.sha256(identity.encode("utf-8")).hexdigest()[:20],
        "source": {"type": "directory", "path": identity, "sha256": source_hash},
        "license_files": license_paths,
        "skills": skills,
        "offline_copy": True,
        "credential_required": False,
    }


def _read_local_archive(path: os.PathLike[str] | str) -> Tuple[Path, bytes, str]:
    archive_path = Path(path).expanduser()
    if archive_path.is_symlink():
        raise SkillImportError("source_symlink", "ZIP 来源不能是符号链接")
    if not archive_path.is_file():
        raise SkillImportError("source_not_found", "ZIP 来源文件不存在")
    archive_path = archive_path.resolve()
    if archive_path.stat().st_size > MAX_RESPONSE_BYTES:
        raise SkillImportError("archive_too_large", "ZIP 文件超过安全大小限制")
    try:
        blob = archive_path.read_bytes()
    except OSError as exc:
        raise SkillImportError("source_unreadable", "无法读取 ZIP 来源文件") from exc
    return archive_path, blob, hashlib.sha256(blob).hexdigest()


def _preview_zip(path: os.PathLike[str] | str) -> Dict[str, Any]:
    archive_path, blob, archive_hash = _read_local_archive(path)
    extracted = _extract(blob)
    try:
        root = _archive_root(extracted)
        manifest = _scan_directory(root)
        skills, license_paths = _scan_skill_candidates(root, manifest)
        if not skills:
            raise SkillImportError("skill_not_found", "ZIP 中没有可导入的 SKILL.md")
        content_hash = _manifest_sha256(manifest)
        identity = str(archive_path)
        return {
            "source_id": "zip-" + hashlib.sha256(identity.encode("utf-8")).hexdigest()[:20],
            "source": {
                "type": "zip",
                "path": identity,
                "archive_sha256": archive_hash,
                "sha256": content_hash,
            },
            "license_files": license_paths,
            "skills": skills,
            "offline_copy": True,
            "credential_required": False,
        }
    finally:
        shutil.rmtree(extracted, ignore_errors=True)


def preview_source(source_type: str, source: os.PathLike[str] | str, credential_id: str = "") -> Dict[str, Any]:
    """Preview a GitHub URL, local Skill directory, or ZIP archive."""
    kind = str(source_type or "").strip().lower()
    if kind in {"directory", "folder", "local"}:
        return _preview_directory(source)
    if kind in {"zip", "archive"}:
        return _preview_zip(source)
    if kind == "github":
        return preview_import(str(source), credential_id)
    raise SkillImportError("source_type_invalid", "不支持的 Skill 来源类型")


def preview_import(url: str, credential_id: str = "") -> Dict[str, Any]:
    source = parse_github_url(url)
    token = _token(credential_id)
    sha, repo, entries = _resolve(source, token)
    prefix = source.get("subdir", "").strip("/")
    skills: List[Dict[str, Any]] = []
    paths = [str(item.get("path") or "") for item in entries if item.get("type") == "blob"]
    license_paths = [p for p in paths if _is_license_path(p)]
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
        errors: List[str] = []
        if not _ID_RE.fullmatch(name):
            errors.append("skill_name_invalid")
        if not description:
            errors.append("skill_description_invalid")
        if set(_VARIABLE_RE.findall(text)) - _ALLOWED_VARIABLES:
            errors.append("skill_variables_invalid")
        applicable_licenses = _applicable_licenses(folder, license_paths)
        if not applicable_licenses:
            errors.append("skill_license_missing")
        skill_id = name if _ID_RE.fullmatch(name) else _slug(posixpath.basename(folder) or "root")
        file_list = sorted(p for p in paths if _under(p, folder))
        blocking_errors = [item for item in errors if item != "skill_license_missing"]
        skills.append({
            "id": skill_id,
            "path": path,
            "directory": folder,
            "name": name or skill_id,
            "description": description,
            "files": file_list,
            "license_files": applicable_licenses,
            "valid": not errors,
            "draft_allowed": bool(blocking_errors == [] and errors),
            "publishable": not errors,
            "error_code": errors[0] if errors else "",
            "error_codes": errors,
            "scripts_present": any(Path(p).suffix.lower() in _SCRIPT_SUFFIXES for p in file_list),
        })
    if not skills:
        raise SkillImportError("skill_not_found", "仓库中没有可导入的 SKILL.md")
    source_id = "gh-" + hashlib.sha256(source["canonical_url"].encode("utf-8")).hexdigest()[:20]
    return {
        "source_id": source_id,
        "source": {**source, "type": "github", "resolved_commit": sha, "repository_name": repo.get("full_name", "")},
        "license_files": license_paths,
        "skills": skills,
        "offline_copy": False,
        "credential_required": bool(repo.get("private")) and not bool(token),
    }


def _load_sources() -> Dict[str, Any]:
    data = load_json(SKILLS_FILE, {})
    if not isinstance(data, dict):
        return {"schema_version": 2, "sources": []}
    # Migrate the old source list while stripping persisted absolute paths.
    # Local sources can still be re-imported from a fresh preview; their old
    # path is intentionally not retained in the user-facing config.
    if data.get("schema_version") not in (1, 2):
        return {"schema_version": 2, "sources": []}
    if not isinstance(data.get("sources"), list):
        data["sources"] = []
    data["schema_version"] = 2
    migrated = False
    for source in data["sources"]:
        if not isinstance(source, dict):
            continue
        path = source.pop("source_path", "")
        migrated = migrated or bool(path)
        if path and not source.get("source_label"):
            source["source_label"] = os.path.basename(str(path))
        provenance = source.get("provenance")
        if isinstance(provenance, dict):
            path = provenance.pop("source_path", "")
            migrated = migrated or bool(path)
            if path and not source.get("source_label"):
                source["source_label"] = os.path.basename(str(path))
    # Persist the privacy migration immediately.  Keeping the sanitisation in
    # memory only would leave legacy absolute paths (including usernames and
    # drive letters) on disk and restore them on the next process start.
    if migrated:
        _save_sources(data)
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
    if len(Path(name).parts) > MAX_PATH_DEPTH:
        raise SkillImportError("archive_path_too_deep", "归档路径层级超过限制")
    return name


def _skill_destination(skill_id: str) -> Path:
    """Return a confined user-Skill destination without following symlinks.

    Import is a write operation.  Checking only ``Path.resolve`` at the end is
    not sufficient when an attacker swaps the ``skills`` directory (or a
    conflicting Skill) for a symlink between validation and ``copytree``.
    Keep the directory itself and the final target boring: an ordinary
    directory below ``DATA_DIR/skills``.  Existing symlink targets are refused
    rather than replaced, so the caller can report a deterministic conflict.
    """
    if not _ID_RE.fullmatch(str(skill_id or "")):
        raise SkillImportError("skill_id_invalid", "Skill ID 必须使用小写 kebab-case")
    root = Path(DATA_DIR) / "skills"
    if root.exists() and (root.is_symlink() or not root.is_dir()):
        raise SkillImportError("config_path_invalid", "Skill 数据目录不可用")
    root.mkdir(parents=True, exist_ok=True)
    root_real = root.resolve()
    destination = root / str(skill_id)
    if destination.is_symlink():
        raise SkillImportError("skill_conflict", "Skill 目标是符号链接")
    resolved = destination.resolve()
    if resolved.parent != root_real or resolved == root_real:
        raise SkillImportError("skill_path_invalid", "Skill 目标路径无效")
    return destination


def _extract(blob: bytes) -> Path:
    temp = Path(tempfile.mkdtemp(prefix="readmd-skill-import-"))
    try:
        with zipfile.ZipFile(io.BytesIO(blob)) as archive:
            infos = archive.infolist()
            if len(infos) > MAX_FILES:
                raise SkillImportError("archive_too_many_files", "归档文件数量超过限制")
            total = 0
            seen_names = set()
            for info in infos:
                name = _safe_member(info.filename.rstrip("/")) if info.filename.rstrip("/") else ""
                if not name:
                    continue
                folded = name.casefold()
                if folded in seen_names:
                    raise SkillImportError("archive_duplicate_path", "归档包含重复或大小写冲突的路径")
                seen_names.add(folded)
                if info.flag_bits & 0x1:
                    raise SkillImportError("archive_encrypted", "不支持加密的 ZIP 文件")
                # UNIX mode bits identify symlinks and device/special files.
                file_type = (info.external_attr >> 16) & 0o170000
                if file_type == 0o120000:
                    raise SkillImportError("archive_symlink", "归档包含不允许的符号链接")
                if file_type not in (0, stat.S_IFREG, stat.S_IFDIR):
                    raise SkillImportError("archive_special_file", "归档包含不允许的特殊文件")
                if int(info.file_size or 0) > MAX_FILE_BYTES:
                    raise SkillImportError("archive_file_too_large", "归档包含超过大小限制的文件")
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


def _restore_skill_backup(backup: Optional[Path], destination: Path) -> None:
    shutil.rmtree(destination, ignore_errors=True)
    if backup is not None and backup.is_dir():
        shutil.copytree(backup, destination)


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
    manifest = _scan_directory(root)
    manifest_paths = {str(item.get("path") or "") for item in manifest}
    if not manifest_paths.issubset(allowed_paths):
        shutil.rmtree(extracted, ignore_errors=True)
        raise SkillImportError("source_changed", "GitHub 归档与已解析的提交目录不一致")
    scanned_skills, scanned_license_paths = _scan_skill_candidates(root, manifest)
    scanned_by_path = {str(item.get("path") or ""): item for item in scanned_skills}
    selections = list(selections)
    imported: List[Dict[str, Any]] = []
    skipped: List[Dict[str, str]] = []
    config = _load_sources()
    try:
        for selected in selections:
            if not isinstance(selected, Mapping):
                continue
            path = str(selected.get("path") or "").replace("\\", "/")
            declared = scanned_by_path.get(path)
            if not isinstance(declared, Mapping):
                raise SkillImportError("skill_path_invalid", "选中的 Skill 路径不在提交清单中")
            if not _declaration_importable(declared):
                code = str(declared.get("error_code") or "skill_invalid")
                raise SkillImportError(code, "选中的 Skill 未通过安全校验")
            if declared.get("valid") is True and not declared.get("license_files") and declared.get("publishable") is not False:
                raise SkillImportError("skill_license_missing", "Skill 许可证文件不可用")
            directory = str(declared.get("directory") or posixpath.dirname(path)).strip("/")
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
            skill_id = str(selected.get("target_id") or selected.get("id") or declared.get("id") or "")
            if not _ID_RE.fullmatch(skill_id):
                raise SkillImportError("skill_id_invalid", "Skill ID 必须使用小写 kebab-case")
            destination = _skill_destination(skill_id)
            backup: Optional[Path] = None
            action = str(selected.get("conflict_action") or "skip").lower()
            if destination.exists():
                if action == "skip":
                    skipped.append({"id": skill_id, "reason": "conflict"})
                    continue
                if action not in ("replace", "rename"):
                    raise SkillImportError("skill_conflict", "Skill 已存在，请选择跳过、重命名或替换")
                if action == "rename":
                    base = skill_id + "-imported"
                    skill_id = base
                    counter = 2
                    while _skill_destination(skill_id).exists():
                        skill_id = "%s-%d" % (base, counter)
                        counter += 1
                    destination = _skill_destination(skill_id)
                else:
                    backup = Path(DATA_DIR) / "skills" / ".versions" / skill_id / ("github-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ"))
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(destination), str(backup))
            destination.parent.mkdir(parents=True, exist_ok=True)
            try:
                shutil.copytree(source_dir, destination)
                imported_skill_file = _skill_file(destination)
                if imported_skill_file is None:
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
                for license_path in declared.get("license_files", []):
                    license_path = str(license_path or "").replace("\\", "/")
                    if not license_path or _under(license_path, directory):
                        continue
                    _safe_member(license_path)
                    license_source = root / Path(license_path)
                    if not license_source.is_file() or license_source.is_symlink():
                        raise SkillImportError("skill_license_missing", "Skill 许可证文件不可用")
                    license_destination = destination / ".readmd-licenses" / Path(license_path)
                    license_destination.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(license_source, license_destination)
                source_files = []
                for source_file in sorted(p for p in destination.rglob("*") if p.is_file()):
                    if source_file.name == "readmd.skill.json":
                        continue
                    digest = hashlib.sha256(source_file.read_bytes()).hexdigest()
                    source_files.append({"path": source_file.relative_to(destination).as_posix(), "sha256": digest})
                source_hash = _directory_sha256(destination)
                enabled = bool(declared.get("publishable", True))
                metadata = {
                    "id": skill_id,
                    "scope": "user",
                    "enabled": enabled,
                    "publishable": enabled,
                    "scripts_allowed": False,
                    "source": "github",
                    "provenance": {"repository": parsed["canonical_url"], "commit": sha, "path": path},
                    "source_files": source_files,
                    "source_sha256": source_hash,
                    "license": ", ".join(str(x) for x in declared.get("license_files", [])),
                    "adaptation_notes": [
                        "Imported from GitHub as data; scripts remain disabled."
                    ] + (["License review required before publishing or running."] if not enabled else []),
                }
                save_text_atomic(str(destination / "readmd.skill.json"), json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
                # Validate the on-disk structure directly after writing metadata.
                try:
                    check = SkillRegistry([destination.parent]).validate(destination)
                except Exception as exc:
                    if isinstance(exc, SkillError):
                        raise SkillImportError("skill_invalid", "导入后 Skill 未通过结构校验") from exc
                    raise
            except BaseException:
                _restore_skill_backup(backup, destination)
                raise
            imported.append({"id": skill_id, "path": path, "sha256": source_hash, "source_files": source_files})
        if not imported and skipped:
            return {"ok": True, "source": None, "skills": [], "skipped": skipped}
        if not imported:
            raise SkillImportError("nothing_imported", "没有导入任何 Skill")
        source_id = preview.get("source_id") or "gh-" + hashlib.sha256(parsed["canonical_url"].encode()).hexdigest()[:20]
        previous = next((s for s in config.get("sources", []) if s.get("source_id") == source_id), None)
        merged_skills = {str(item.get("id")): item for item in (previous or {}).get("skills", []) if isinstance(item, Mapping)}
        merged_skills.update({str(item.get("id")): item for item in imported})
        source_record = {
            "source_id": source_id,
            "source_type": "github",
            "source_sha256": _manifest_sha256(manifest),
            "repository_url": parsed["canonical_url"],
            "owner": parsed["owner"], "repo": parsed["repo"], "subdir": parsed.get("subdir", ""),
            "requested_ref": parsed.get("ref", ""), "resolved_commit": sha,
            "credential_id": credential_id or "", "update_policy": "manual",
            "skills": list(merged_skills.values()), "imported_at": datetime.now(timezone.utc).isoformat(),
        }
        config["sources"] = [s for s in config.get("sources", []) if s.get("source_id") != source_record["source_id"]]
        config["sources"].append(source_record)
        _save_sources(config)
        return {"ok": True, "source": source_record, "skills": imported, "skipped": skipped}
    finally:
        shutil.rmtree(extracted, ignore_errors=True)


def _apply_filesystem_import(
    preview: Mapping[str, Any],
    selections: Iterable[Mapping[str, Any]],
    root: Path,
    actual_hash: str,
    source_type: str,
    source_path: str,
    archive_hash: str = "",
) -> Dict[str, Any]:
    preview_skills = {
        str(item.get("path") or ""): item
        for item in preview.get("skills", [])
        if isinstance(item, Mapping)
    }
    imported: List[Dict[str, Any]] = []
    skipped: List[Dict[str, str]] = []
    config = _load_sources()
    for selected in selections:
        if not isinstance(selected, Mapping):
            continue
        path = str(selected.get("path") or "").replace("\\", "/")
        declared = preview_skills.get(path)
        if not isinstance(declared, Mapping) or not _declaration_importable(declared):
            raise SkillImportError("skill_path_invalid", "选中的 Skill 不在有效预览清单中")
        if declared.get("valid") is True and not declared.get("license_files") and declared.get("publishable") is not False:
            raise SkillImportError("skill_license_missing", "Skill 许可证文件不可用")
        directory = str(declared.get("directory") or "").strip("/")
        if path.lower() != (posixpath.join(directory, "skill.md") if directory else "skill.md").lower():
            raise SkillImportError("skill_path_invalid", "选中的 Skill 目录与入口文件不匹配")
        source_dir = root / Path(directory)
        source_skill_file = _skill_file(source_dir)
        if source_skill_file is None:
            raise SkillImportError("skill_missing", "选中的 Skill 目录不完整")
        skill_id = str(selected.get("target_id") or selected.get("id") or declared.get("id") or "")
        if not _ID_RE.fullmatch(skill_id):
            raise SkillImportError("skill_id_invalid", "Skill ID 必须使用小写 kebab-case")
        destination = _skill_destination(skill_id)
        backup: Optional[Path] = None
        action = str(selected.get("conflict_action") or "skip").lower()
        if destination.exists():
            if action == "skip":
                skipped.append({"id": skill_id, "reason": "conflict"})
                continue
            if action not in {"replace", "rename"}:
                raise SkillImportError("skill_conflict", "Skill 已存在，请选择跳过、重命名或替换")
            if action == "rename":
                base = skill_id + "-imported"
                skill_id = base
                counter = 2
                while _skill_destination(skill_id).exists():
                    skill_id = "%s-%d" % (base, counter)
                    counter += 1
                destination = _skill_destination(skill_id)
            else:
                backup = Path(DATA_DIR) / "skills" / ".versions" / skill_id / (
                    source_type + "-" + datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
                )
                backup.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(destination), str(backup))
        destination.parent.mkdir(parents=True, exist_ok=True)
        try:
            shutil.copytree(source_dir, destination)
            imported_skill_file = _skill_file(destination)
            if imported_skill_file is None:
                raise SkillImportError("skill_missing", "选中的 Skill 目录不完整")
            if imported_skill_file.name != "SKILL.md":
                imported_skill_file.rename(destination / "SKILL.md")
            skill_file = destination / "SKILL.md"
            text = skill_file.read_text(encoding="utf-8")
            original_id = str(declared.get("id") or "")
            if skill_id != original_id:
                match = re.search(r"^(name:\s*)([^\r\n]+)", text, re.M)
                if match:
                    text = text[:match.start(2)] + skill_id + text[match.end(2):]
                    skill_file.write_text(text, encoding="utf-8", newline="\n")
            for license_path in declared.get("license_files", []):
                license_path = str(license_path or "").replace("\\", "/")
                if not license_path or _under(license_path, directory):
                    continue
                _safe_member(license_path)
                license_source = root / Path(license_path)
                if not license_source.is_file() or license_source.is_symlink():
                    raise SkillImportError("skill_license_missing", "Skill 许可证文件不可用")
                license_destination = destination / ".readmd-licenses" / Path(license_path)
                license_destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(license_source, license_destination)
            source_files = []
            for source_file in sorted(p for p in destination.rglob("*") if p.is_file()):
                if source_file.name == "readmd.skill.json":
                    continue
                source_files.append({
                    "path": source_file.relative_to(destination).as_posix(),
                    "sha256": hashlib.sha256(source_file.read_bytes()).hexdigest(),
                })
            source_hash = _directory_sha256(destination)
            enabled = bool(declared.get("publishable", True))
            metadata = {
                "id": skill_id,
                "scope": "user",
                "enabled": enabled,
                "publishable": enabled,
                "scripts_allowed": False,
                "source": source_type,
                "provenance": {
                    "type": source_type,
                    "source_label": os.path.basename(str(source_path)),
                    "source_sha256": actual_hash,
                    "archive_sha256": archive_hash,
                    "skill_path": path,
                },
                "source_files": source_files,
                "source_sha256": source_hash,
                "license": ", ".join(str(item) for item in declared.get("license_files", [])),
                "adaptation_notes": [
                    "Imported from a local %s as data; scripts remain disabled." % source_type
                ] + (["License review required before publishing or running."] if not enabled else []),
            }
            save_text_atomic(str(destination / "readmd.skill.json"), json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
            try:
                SkillRegistry([destination.parent]).validate(destination)
            except Exception as exc:
                if isinstance(exc, SkillError):
                    raise SkillImportError("skill_invalid", "导入后 Skill 未通过结构校验") from exc
                raise
        except BaseException:
            _restore_skill_backup(backup, destination)
            raise
        imported.append({"id": skill_id, "path": path, "sha256": source_hash, "source_files": source_files})
    if not imported and skipped:
        return {"ok": True, "source": None, "skills": [], "skipped": skipped}
    if not imported:
        raise SkillImportError("nothing_imported", "没有导入任何 Skill")
    prefix = "dir" if source_type == "directory" else source_type
    source_id = str(preview.get("source_id") or prefix + "-" + actual_hash[:20])
    source_record = {
        "source_id": source_id,
        "source_type": source_type,
        "source_label": os.path.basename(str(source_path)),
        "source_sha256": actual_hash,
        "archive_sha256": archive_hash,
        "update_policy": "manual",
        "skills": imported,
        "imported_at": datetime.now(timezone.utc).isoformat(),
    }
    config["sources"] = [item for item in config.get("sources", []) if item.get("source_id") != source_id]
    config["sources"].append(source_record)
    _save_sources(config)
    return {"ok": True, "source": source_record, "skills": imported, "skipped": skipped}


def _apply_directory_import(preview: Mapping[str, Any], selections: Iterable[Mapping[str, Any]]) -> Dict[str, Any]:
    source = preview.get("source") if isinstance(preview.get("source"), Mapping) else {}
    root = Path(str(source.get("path") or "")).expanduser()
    manifest = _scan_directory(root)
    root = root.resolve()
    actual_hash = _manifest_sha256(manifest)
    if not source.get("sha256") or actual_hash != str(source.get("sha256")):
        raise SkillImportError("source_changed", "预览后 Skill 来源已发生变化，请重新预览")
    return _apply_filesystem_import(preview, selections, root, actual_hash, "directory", str(root))


def _apply_zip_import(preview: Mapping[str, Any], selections: Iterable[Mapping[str, Any]]) -> Dict[str, Any]:
    source = preview.get("source") if isinstance(preview.get("source"), Mapping) else {}
    archive_path, blob, archive_hash = _read_local_archive(str(source.get("path") or ""))
    if not source.get("archive_sha256") or archive_hash != str(source.get("archive_sha256")):
        raise SkillImportError("source_changed", "预览后 ZIP 来源已发生变化，请重新预览")
    extracted = _extract(blob)
    try:
        root = _archive_root(extracted)
        manifest = _scan_directory(root)
        actual_hash = _manifest_sha256(manifest)
        if not source.get("sha256") or actual_hash != str(source.get("sha256")):
            raise SkillImportError("source_changed", "预览后 ZIP 内容已发生变化，请重新预览")
        return _apply_filesystem_import(
            preview, selections, root, actual_hash, "zip", str(archive_path), archive_hash
        )
    finally:
        shutil.rmtree(extracted, ignore_errors=True)


def apply_source_import(
    preview: Mapping[str, Any],
    selections: Iterable[Mapping[str, Any]],
    credential_id: str = "",
    confirm: bool = False,
) -> Dict[str, Any]:
    """Apply a preview produced by :func:`preview_source`."""
    if not confirm:
        raise SkillImportError("confirmation_required", "导入 Skill 需要明确确认")
    source = preview.get("source") if isinstance(preview.get("source"), Mapping) else {}
    kind = str(source.get("type") or "github").lower()
    if kind == "directory":
        return _apply_directory_import(preview, selections)
    if kind == "zip":
        return _apply_zip_import(preview, selections)
    if kind == "github":
        return apply_import(preview, selections, credential_id, confirm=True)
    raise SkillImportError("source_type_invalid", "不支持的 Skill 来源类型")


def _directory_sha256(directory: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(p for p in directory.rglob("*") if p.is_file() and p.name != "readmd.skill.json"):
        digest.update(path.relative_to(directory).as_posix().encode("utf-8"))
        with open(path, "rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def _declaration_importable(declared: Mapping[str, Any]) -> bool:
    """Allow a license-less Skill only as a disabled, reviewable draft."""
    if declared.get("valid") is True:
        return True
    codes = {str(code) for code in (declared.get("error_codes") or []) if code}
    if not codes and declared.get("error_code"):
        codes.add(str(declared.get("error_code")))
    return declared.get("draft_allowed") is True and codes == {"skill_license_missing"}


def list_sources() -> List[Dict[str, Any]]:
    return list(_load_sources().get("sources", []))


def find_source(source_id: str) -> Optional[Dict[str, Any]]:
    return next((s for s in list_sources() if s.get("source_id") == source_id), None)


def preview_saved_source(source: Mapping[str, Any], credential_id: str = "") -> Dict[str, Any]:
    """Re-preview a persisted source through its original adapter."""
    kind = str(source.get("source_type") or "github").strip().lower()
    if kind == "github":
        value = str(source.get("repository_url") or "")
    elif kind in {"directory", "zip"}:
        # New persisted records intentionally omit absolute paths.  Legacy
        # callers may still provide one explicitly for a one-off re-check.
        value = str(source.get("source_path") or "")
    else:
        raise SkillImportError("source_type_invalid", "不支持的 Skill 来源类型")
    if not value:
        raise SkillImportError("source_not_found", "Skill 来源不可用")
    return preview_source(kind, value, credential_id)


def source_preview_changed(source: Mapping[str, Any], preview: Mapping[str, Any]) -> bool:
    kind = str(source.get("source_type") or "github").strip().lower()
    next_source = preview.get("source") if isinstance(preview.get("source"), Mapping) else {}
    if kind == "github":
        return str(next_source.get("resolved_commit") or "") != str(source.get("resolved_commit") or "")
    return str(next_source.get("sha256") or "") != str(source.get("source_sha256") or "")


def remove_source(source_id: str) -> bool:
    """Remove only the source binding; imported Skill files remain intact."""
    data = _load_sources()
    before = len(data.get("sources", []))
    data["sources"] = [s for s in data.get("sources", []) if s.get("source_id") != source_id]
    if len(data["sources"]) == before:
        return False
    _save_sources(data)
    return True


__all__ = [
    "SkillImportError", "parse_github_url", "preview_source", "apply_source_import",
    "preview_import", "apply_import", "list_sources", "find_source", "preview_saved_source",
    "source_preview_changed", "remove_source",
]
