# -*- coding: utf-8 -*-
"""Small shared ReadMD Core facade used by desktop, MCP and editor clients.

The facade intentionally contains no UI or process-control code.  It owns the
cross-surface data contracts (Skills and AI payload preparation) while each
client remains free to choose HTTP, stdio or an in-process transport.
"""
from __future__ import annotations

import json
from typing import Any, Mapping, Optional

from ..readmd_modules.skills import SkillRegistry, default_skill_roots
from .config import HISTORY_FILE


class ReadMDCoreService:
    """Long-lived, transport-neutral service object.

    A client can keep one instance alive and call ``reload`` when a project or
    user Skill changes.  No external process is spawned by this class and no
    credentials are accepted here; AI transport resolves credentials in the
    server-side AI module.
    """

    def __init__(self, project_dir: Optional[str] = None):
        self.project_dir = project_dir
        self.skills = SkillRegistry(default_skill_roots(project_dir))

    def reload(self) -> None:
        self.skills.reload()

    def list_skills(self):
        return self.skills.list()

    def get_skill(self, skill_id: str):
        return self.skills.get(skill_id)

    def render_skill(self, skill_id: str, variables: Mapping[str, Any]) -> str:
        return self.skills.render(skill_id, variables)

    def list_providers(self):
        """Return the shared, secret-free provider view for editor clients."""
        from ..readmd_modules import ai
        return ai.get_config()

    def ai_chat(self, payload: Mapping[str, Any]):
        """Resolve a Skill and credential through the canonical AI module."""
        from ..readmd_modules import ai
        return ai.chat(dict(payload))

    def list_history(self, limit: int = 50):
        """Read local AI history without exposing credential-like fields."""
        try:
            with open(HISTORY_FILE, encoding="utf-8") as handle:
                data = json.load(handle)
        except (OSError, ValueError, json.JSONDecodeError):
            data = {"sessions": []}
        sessions = data.get("sessions", []) if isinstance(data, dict) else []
        blocked = {"api_key", "key", "secret", "password", "token"}
        def scrub(value):
            if isinstance(value, dict):
                return {k: scrub(v) for k, v in value.items() if str(k).lower() not in blocked}
            if isinstance(value, list):
                return [scrub(v) for v in value]
            return value
        return scrub(sessions[:max(0, int(limit))])


__all__ = ["ReadMDCoreService"]
