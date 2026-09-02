import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_vscode_readme_matches_manifest_and_release_boundaries():
    manifest = json.loads((ROOT / "packages" / "vscode-extension" / "package.json").read_text(encoding="utf-8"))
    readme = (ROOT / "packages" / "vscode-extension" / "README.md").read_text(encoding="utf-8")
    assert "v2.3.6" not in readme.lower()
    version = str(manifest.get("version") or "")
    assert version and version in readme
    for command in manifest["contributes"]["commands"]:
        assert f"`{command['command']}`" in readme, command["command"]
    folded = readme.casefold()
    for marker in ("github", "credential_id", "mcp", "离线", "offline", "skills"):
        assert marker in folded
