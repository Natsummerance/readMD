from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VALIDATOR = ROOT / "showcase" / "scripts" / "validate_website.py"


class WebsitePipelineTest(unittest.TestCase):
    def test_staged_website_passes_geo_contract(self) -> None:
        result = subprocess.run(
            [sys.executable, str(VALIDATOR)],
            cwd=ROOT,
            text=True,
            capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["languages"], ["en", "zh-CN", "zh-TW", "ja"])

    def test_website_release_requires_three_approved_rounds(self) -> None:
        approval = json.loads((ROOT / "showcase/reports/website_approval.json").read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(approval["rounds"]), 3)
        self.assertTrue(all(item["status"] == "approved" for item in approval["rounds"]))
        self.assertTrue(all(item["score"] >= 9.7 for item in approval["rounds"]))
        meeting = approval["final_decision_meeting"]
        self.assertEqual(meeting["status"], "approved_for_staged_publication")
        self.assertGreaterEqual(meeting["score"], 9.7)


if __name__ == "__main__":
    unittest.main()
