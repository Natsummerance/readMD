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
    for language, contract in validator.DOWNLOAD_PAGES.items():
        content = contract["path"].read_text(encoding="utf-8")
        linked = {
            name
            for _, name in re.findall(
                rf'href="(https://github\.com/Natsummerance/readMD/releases/download/{tag}/([^"]+))"',
                content,
            )
        }
        if linked != validator.RELEASE_ASSETS:
            errors.append(
                f"{language} download assets mismatch: "
                f"missing={sorted(validator.RELEASE_ASSETS - linked)}, "
                f"extra={sorted(linked - validator.RELEASE_ASSETS)}"
            )
    return errors


validator.validate_release_asset_links = validate_pinned_release_asset_links


if __name__ == "__main__":
    raise SystemExit(validator.main())
