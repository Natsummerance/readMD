# -*- coding: utf-8 -*-
"""HarmonyOS project metadata must follow the single release version."""

import json
import os
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)


class HarmonyVersionSyncTest(unittest.TestCase):
    def test_package_and_app_metadata_use_release_version(self):
        version = os.path.join(ROOT, "VERSION")
        with open(version, encoding="utf-8") as handle:
            expected = handle.read().strip()

        with open(os.path.join(ROOT, "packages", "harmonyos-app", "package.json"), encoding="utf-8") as handle:
            package = json.load(handle)
        self.assertEqual(package["version"], expected)

        app_path = os.path.join(ROOT, "packages", "harmonyos-app", "AppScope", "app.json5")
        with open(app_path, encoding="utf-8") as handle:
            self.assertIn(f'"versionName": "{expected}"', handle.read())


if __name__ == "__main__":
    unittest.main()
