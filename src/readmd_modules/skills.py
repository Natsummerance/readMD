# -*- coding: utf-8 -*-
"""Portable, data-only runtime for ReadMD Skills."""
from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional


class SkillError(ValueError):
    """Raised when a Skill is invalid, unsafe or cannot be rendered."""


_NAME_RE = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
_VARIABLE_RE = re.compile(r"\{\{\s*([a-zA-Z0-9_-]+)\s*\}\}")
_ALLOWED_VARIABLES = {"document", "selection", "request", "language", "context", "output_format"}
_MAX_SKILL_BYTES = 512 * 1024
_MAX_METADATA_BYTES = 128 * 1024


@dataclass(frozen=True)
class Skill:
    id: str
    name: str
    description: str
    instructions: str
    scope: str
    root: str
    path: str
    metadata: Mapping[str, Any] = field(default_factory=dict)

    @property
    def variables(self) -> List[str]:
        return sorted(set(_VARIABLE_RE.findall(self.instructions)))


def _inside(child: Path, parent: Path) -> bool:
    try:
        child.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def _parse_frontmatter(text: str) -> tuple[Dict[str, Any], str]:
    if not text.startswith("---"):
        raise SkillError("SKILL.md must start with YAML frontmatter")
    match = re.match(r"^---\s*\r?\n(.*?)\r?\n---\s*\r?\n?(.*)$", text, re.S)
    if not match:
        raise SkillError("SKILL.md frontmatter is not closed")
    values: Dict[str, Any] = {}
    for raw in match.group(1).splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or ":" not in line:
            continue
        key, value = line.split(":", 1)
        value = value.strip().strip("'\"")
        if value.lower() in ("true", "false"):
            values[key.strip()] = value.lower() == "true"
        elif value.startswith("[") and value.endswith("]"):
            values[key.strip()] = [x.strip().strip("'\"") for x in value[1:-1].split(",") if x.strip()]
        else:
            values[key.strip()] = value
    return values, match.group(2).strip()


def _read_metadata(folder: Path) -> Dict[str, Any]:
    sidecar = folder / "readmd.skill.json"
    if not sidecar.exists():
        return {}
    if not sidecar.is_file() or sidecar.stat().st_size > _MAX_METADATA_BYTES:
        raise SkillError("invalid or oversized readmd.skill.json")
    try:
        value = json.loads(sidecar.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise SkillError("invalid readmd.skill.json") from exc
    if not isinstance(value, dict):
        raise SkillError("readmd.skill.json must contain an object")
    return value


def load_skill(folder: Path, scope: str, root: Path) -> Skill:
    folder, root = folder.resolve(), root.resolve()
    if not _inside(folder, root) or folder == root:
        raise SkillError("Skill path escapes its configured root")
    if not _NAME_RE.fullmatch(folder.name):
        raise SkillError("Skill directory name must be lowercase kebab-case")
    skill_file = folder / "SKILL.md"
    if not skill_file.is_file() or skill_file.stat().st_size > _MAX_SKILL_BYTES:
        raise SkillError("Skill requires a readable SKILL.md within the size limit")
    try:
        frontmatter, instructions = _parse_frontmatter(skill_file.read_text(encoding="utf-8"))
    except (OSError, UnicodeError) as exc:
        raise SkillError("cannot read SKILL.md") from exc
    name = str(frontmatter.get("name") or folder.name).strip()
    description = str(frontmatter.get("description") or "").strip()
    if not _NAME_RE.fullmatch(name):
        raise SkillError("Skill name must be lowercase kebab-case")
    if not description:
        raise SkillError("Skill description is required")
    if not description.lower().startswith("use when"):
        raise SkillError("Skill description must start with 'Use when'")
    unknown = set(_VARIABLE_RE.findall(instructions)) - _ALLOWED_VARIABLES
    if unknown:
        raise SkillError("unsupported Skill variables: " + ", ".join(sorted(unknown)))
    metadata = _read_metadata(folder)
    metadata = dict(metadata)
    metadata.setdefault("id", name)
    metadata.setdefault("scope", scope)
    metadata.setdefault("entrypoint", "SKILL.md")
    return Skill(name, name, description, instructions, scope, str(root), str(folder), metadata)


class SkillRegistry:
    """Resolve builtin, user and project Skills without executing code."""

    def __init__(self, roots: Iterable[os.PathLike[str] | str]):
        self.roots = [Path(root).expanduser().resolve() for root in roots]
        for root in self.roots:
            if root.exists() and not root.is_dir():
                raise SkillError(f"Skill root is not a directory: {root}")
        self._skills: Dict[str, Skill] = {}
        self.reload()

    def reload(self) -> None:
        self._skills.clear()
        for index, root in enumerate(self.roots):
            if not root.is_dir():
                continue
            scope = ("builtin", "user", "project")[index] if index < 3 else f"root-{index}"
            for folder in sorted(root.iterdir(), key=lambda p: p.name):
                if not folder.is_dir() or folder.name.startswith("."):
                    continue
                try:
                    skill = load_skill(folder, scope, root)
                except SkillError:
                    continue
                if skill.metadata.get("enabled") is False:
                    continue
                self._skills[skill.id] = skill

    def list(self) -> List[Skill]:
        return sorted(self._skills.values(), key=lambda item: item.id)

    def get(self, skill_id: str) -> Optional[Skill]:
        return self._skills.get(str(skill_id or "").strip())

    def render(self, skill_id: str, variables: Mapping[str, Any]) -> str:
        skill = self.get(skill_id)
        if skill is None:
            raise SkillError(f"Skill not found: {skill_id}")
        required = skill.metadata.get("required_variables") or ["document"]
        missing = [name for name in required if name in skill.variables and not str(variables.get(name, "")).strip()]
        if missing:
            raise SkillError("missing required Skill variables: " + ", ".join(missing))
        rendered = skill.instructions
        for name in skill.variables:
            rendered = re.sub(r"\{\{\s*" + re.escape(name) + r"\s*\}\}", str(variables.get(name, "")), rendered)
        return rendered.strip()

    def validate(self, folder: os.PathLike[str] | str) -> Skill:
        folder_path = Path(folder).expanduser().resolve()
        return load_skill(folder_path, "draft", folder_path.parent)


def default_skill_roots(project_dir: Optional[os.PathLike[str] | str] = None) -> List[Path]:
    from ..readmd_core.config import DATA_DIR

    package_root = Path(__file__).resolve().parents[2]
    roots = [package_root / "assets" / "skills", Path(DATA_DIR) / "skills"]
    if project_dir:
        roots.append(Path(project_dir).expanduser() / ".readmd" / "skills")
    return roots


__all__ = ["Skill", "SkillError", "SkillRegistry", "default_skill_roots", "load_skill"]
