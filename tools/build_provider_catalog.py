# -*- coding: utf-8 -*-
"""Generate the offline ReadMD provider catalog from the vendored CC-SWITCH snapshot.

The upstream project is TypeScript, so importing it at build time would make
the release depend on Node, aliases and third-party packages.  This generator
uses a deliberately conservative lexer instead: it records every literal
provider name and nearby declarative fields, while retaining the exact source
file and hash for provenance.  The runtime catalog never executes upstream
code and strips promotion/affiliate fields by construction.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "assets" / "providers" / "provider-catalog.json"
UPSTREAM = ROOT / "assets" / "upstream" / "farion1231-cc-switch" / "6243e20ad6f1835f9ac94ab39ea0eb62a6795bc0"
COMMIT = "6243e20ad6f1835f9ac94ab39ea0eb62a6795bc0"
_STRING = r"[\"']([^\"']{1,300})[\"']"
_NAME = re.compile(r"\bname\s*:\s*" + _STRING)
_FIELD = {
    "base_url": re.compile(r"\b(?:baseUrl|base_url|apiUrl|apiBaseUrl|websiteUrl)\s*:\s*" + _STRING),
    "format": re.compile(r"\b(?:format|protocol|providerType)\s*:\s*" + _STRING, re.I),
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_files() -> list[Path]:
    return sorted(p for p in (UPSTREAM / "src" / "config").glob("*.ts") if p.is_file())


def extract() -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    for path in source_files():
        text = path.read_text(encoding="utf-8")
        source_hash = sha256(path)
        for index, match in enumerate(_NAME.finditer(text)):
            name = match.group(1).strip()
            if not name or name in {"string", "name"}:
                continue
            line = text.count("\n", 0, match.start()) + 1
            nearby = text[match.end():match.end() + 1800]
            fields: dict[str, str] = {}
            for field, pattern in _FIELD.items():
                field_match = pattern.search(nearby)
                if field_match:
                    fields[field] = field_match.group(1).strip()
            fmt = fields.get("format", "openai").lower()
            if "anthropic" in fmt or "claude" in path.name.lower():
                fmt = "anthropic"
            elif "gemini" in fmt or "gemini" in path.name.lower():
                fmt = "gemini"
            else:
                fmt = "openai"
            base_url = fields.get("base_url", "")
            if not base_url.startswith(("http://", "https://")):
                base_url = ""
            entries.append({
                "id": "cc-switch:%s:%04d" % (path.stem, index),
                "name": name,
                "base_url": base_url,
                "format": fmt,
                "models": [],
                "category": "upstream",
                "source_only": True,
                "source_ref": "assets/upstream/farion1231-cc-switch/%s/src/config/%s" % (COMMIT, path.name),
                "source_sha256": source_hash,
                "source_line": line,
                "upstream_commit": COMMIT,
                "capabilities": {"chat": True, "stream": True, "models": False},
                "adaptation_notes": [
                    "ReadMD stores this entry as an offline reference; promotion and affiliate fields are not runtime configuration.",
                    "Complete endpoint and credentials are selected by the ReadMD Provider v3 editor.",
                ],
            })
    return entries


def build() -> dict[str, Any]:
    if not UPSTREAM.is_dir():
        raise SystemExit("vendored CC-SWITCH snapshot is missing: %s" % UPSTREAM)
    existing = json.loads(CATALOG.read_text(encoding="utf-8")) if CATALOG.is_file() else {}
    providers = [dict(p) for p in existing.get("providers", []) if isinstance(p, dict) and p.get("name")]
    # Existing hand-tuned entries remain the stable ReadMD runtime presets.
    # Generated entries are source-only and are available for search/import.
    return {
        "schema_version": 2,
        "source": "https://github.com/farion1231/cc-switch",
        "upstream_commit": COMMIT,
        "license": "MIT",
        "attribution": "Provider catalog fields adapted from the offline CC-SWITCH snapshot; runtime excludes promotion and affiliate fields.",
        "snapshot_manifest": "assets/upstream/manifest.json",
        "generated_by": "tools/build_provider_catalog.py",
        "providers": providers,
        "upstream_entries": extract(),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="verify generated output is reproducible")
    args = parser.parse_args()
    data = build()
    encoded = json.dumps(data, ensure_ascii=False, indent=2, sort_keys=False) + "\n"
    if args.check:
        current = CATALOG.read_text(encoding="utf-8").replace("\r\n", "\n") if CATALOG.is_file() else ""
        if current != encoded:
            raise SystemExit("provider catalog is stale; run tools/build_provider_catalog.py")
        print("provider catalog verified (%d presets, %d upstream entries)" % (len(data["providers"]), len(data["upstream_entries"])))
        return 0
    CATALOG.parent.mkdir(parents=True, exist_ok=True)
    CATALOG.write_text(encoded, encoding="utf-8", newline="\n")
    print("wrote %s (%d presets, %d upstream entries)" % (CATALOG, len(data["providers"]), len(data["upstream_entries"])))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
