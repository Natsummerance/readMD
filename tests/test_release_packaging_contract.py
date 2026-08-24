# -*- coding: utf-8 -*-
"""Release contracts that prevent misleading or unsafe distribution assets."""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()


class ReleasePackagingContractTest(unittest.TestCase):
    def test_invalid_hap_source_archive_is_not_published(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        linux_build = (ROOT / "scripts/linux/build_linux.sh").read_text(encoding="utf-8")
        notes = (ROOT / "release/release_notes.md").read_text(encoding="utf-8")
        readmes = "\n".join(
            (ROOT / name).read_text(encoding="utf-8")
            for name in ("README.md", "README.en.md", "README.ja.md", "README.zh-TW.md")
        )
        self.assertNotIn("ReadMD-harmonyos", workflow)
        self.assertNotIn(".hap", linux_build)
        self.assertNotIn(".hap", notes)
        self.assertNotIn("releases/latest/download/ReadMD-harmonyos", readmes)

    def test_mcp_zip_contains_runtime_source_and_requirements(self) -> None:
        workflow = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("'packages/mcp-server/readmd_mcp_server.py'", workflow)
        self.assertIn("'src/", workflow)
        self.assertIn("'config/requirements-common.txt'", workflow)
        self.assertIn("glob.glob('assets/**/*'", workflow)

    def test_vsix_documentation_matches_ci_asset_name(self) -> None:
        expected = f"readmd-vscode-{VERSION}.vsix"
        for name in ("README.md", "README.en.md", "README.ja.md"):
            text = (ROOT / name).read_text(encoding="utf-8")
            self.assertIn(expected, text)
            self.assertNotIn(f"readmd-{VERSION}.vsix", text)


if __name__ == "__main__":
    unittest.main()
