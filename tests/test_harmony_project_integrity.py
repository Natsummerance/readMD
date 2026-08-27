# -*- coding: utf-8 -*-
"""Keep the HarmonyOS source scaffold structurally buildable and honest."""

import json
import os
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HARMONY = ROOT / "packages" / "harmonyos-app"


class HarmonyProjectIntegrityTest(unittest.TestCase):
    def test_stage_model_build_files_exist(self) -> None:
        required = (
            "build-profile.json5",
            "hvigorfile.ts",
            "oh-package.json5",
            "entry/build-profile.json5",
            "entry/hvigorfile.ts",
            "entry/oh-package.json5",
            "entry/src/main/module.json5",
            "entry/src/main/resources/base/profile/main_pages.json",
        )
        for relative in required:
            self.assertTrue((HARMONY / relative).is_file(), relative)

    def test_module_resource_references_resolve(self) -> None:
        resources = HARMONY / "entry/src/main/resources/base"
        strings = json.loads((resources / "element/string.json").read_text(encoding="utf-8"))
        colors = json.loads((resources / "element/color.json").read_text(encoding="utf-8"))
        names = {item["name"] for item in strings["string"]}
        self.assertLessEqual({"module_desc", "EntryAbility_desc", "EntryAbility_label"}, names)
        self.assertIn("start_window_background", {item["name"] for item in colors["color"]})
        self.assertTrue((resources / "media/icon.png").is_file())
        self.assertTrue((HARMONY / "AppScope/resources/base/media/app_icon.png").is_file())

    def test_web_asset_sync_contract(self) -> None:
        package = json.loads((HARMONY / "package.json").read_text(encoding="utf-8"))
        self.assertEqual(package["scripts"]["sync:web"], "node scripts/sync-web-assets.mjs")
        script = (HARMONY / "scripts/sync-web-assets.mjs").read_text(encoding="utf-8")
        self.assertIn("repositoryRoot, 'assets'", script)
        self.assertIn("resources/rawfile", script)
        # Generated bundles must never become source; npm run sync:web creates them.
        self.assertFalse((HARMONY / "entry/src/main/resources/rawfile").exists())


if __name__ == "__main__":
    unittest.main()
