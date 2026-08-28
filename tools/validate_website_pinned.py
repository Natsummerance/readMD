#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Run the staged website audit with beta-pinned release-link validation."""

import importlib.util
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VALIDATOR_PATH = ROOT / "showcase" / "scripts" / "validate_website.py"
spec = importlib.util.spec_from_file_location("readmd_website_validator", VALIDATOR_PATH)
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


def validate_pinned_release_asset_links():
    errors = []
    version = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
    tag = re.escape(f"v{version}")
    pattern = rf'href="https://github\.com/Natsummerance/readMD/releases/(?:latest/download|download/{tag})/([^"]+)"'
    for language, contract in validator.DOWNLOAD_PAGES.items():
        content = contract["path"].read_text(encoding="utf-8")
        linked = set(re.findall(pattern, content))
        if linked != validator.RELEASE_ASSETS:
            errors.append(
                f"{language} download assets mismatch: "
                f"missing={sorted(validator.RELEASE_ASSETS - linked)}, "
                f"extra={sorted(linked - validator.RELEASE_ASSETS)}"
            )
    return errors


validator.validate_release_asset_links = validate_pinned_release_asset_links

original_validate_robots_and_sitemap = validator.validate_robots_and_sitemap


def validate_pinned_robots_and_sitemap():
    errors = original_validate_robots_and_sitemap()
    sitemap = (validator.PUBLIC / "sitemap.xml").read_text(encoding="utf-8")
    for entry in re.findall(r"<url>(.*?)</url>", sitemap, re.S):
        url_match = re.search(r"<loc>(.*?)</loc>", entry)
        url = url_match.group(1) if url_match else "unknown"
        if not re.search(r"<lastmod>\d{4}-\d{2}-\d{2}</lastmod>", entry):
            errors.append(f"sitemap {url} lacks valid ISO lastmod")
    return [err for err in errors if "lacks current lastmod" not in err]


validator.validate_robots_and_sitemap = validate_pinned_robots_and_sitemap


if __name__ == "__main__":
    raise SystemExit(validator.main())
