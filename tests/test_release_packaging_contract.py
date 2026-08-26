# -*- coding: utf-8 -*-
"""Release contracts that prevent misleading or unsafe distribution assets."""

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
DOWNLOAD_PAGES = (
    "README.md",
    "README.en.md",
    "README.ja.md",
    "README.zh-TW.md",
    "website/public/download/index.html",
    "website/public/ja/download/index.html",
    "website/public/zh-cn/download/index.html",
    "website/public/zh-tw/download/index.html",
)
WEBSITE_DOWNLOAD_PAGES = (
    "website/public/download/index.html",
    "website/public/ja/download/index.html",
    "website/public/zh-cn/download/index.html",
    "website/public/zh-tw/download/index.html",
)


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

    def test_prerelease_download_links_are_pinned_to_current_tag(self) -> None:
        asset_names = (
            f"ReadMDSetup-v{VERSION}.exe",
            f"ReadMD-portable-v{VERSION}.exe",
            f"ReadMD-macos-arm64-v{VERSION}.zip",
            f"ReadMD-macos-x64-v{VERSION}.zip",
            f"ReadMD-linux-x86_64-v{VERSION}.AppImage",
            f"ReadMD-linux-aarch64-v{VERSION}.AppImage",
            f"readmd_{VERSION}_amd64.deb",
            f"readmd_{VERSION}_arm64.deb",
            f"readmd-vscode-{VERSION}.vsix",
            f"readmd-mcp-server-{VERSION}.zip",
            "SHA256SUMS.txt",
        )
        for name in DOWNLOAD_PAGES:
            text = (ROOT / name).read_text(encoding="utf-8")
            self.assertNotIn("releases/latest/download/", text)
            for asset in asset_names:
                self.assertNotIn(f"releases/latest/download/{asset}", text)
                self.assertIn(asset, text)
            self.assertIn(
                f"releases/download/v{VERSION}/ReadMD-linux-aarch64-v{VERSION}.AppImage",
                text,
            )

        for name in WEBSITE_DOWNLOAD_PAGES:
            compact = (ROOT / name).read_text(encoding="utf-8").replace(" ", "")
            self.assertNotIn(
                '"downloadUrl":"https://github.com/Natsummerance/readMD/releases/latest"',
                compact,
            )
            self.assertIn(
                f'"downloadUrl":"https://github.com/Natsummerance/readMD/releases/download/v{VERSION}/ReadMDSetup-v{VERSION}.exe"',
                compact,
            )


if __name__ == "__main__":
    unittest.main()
