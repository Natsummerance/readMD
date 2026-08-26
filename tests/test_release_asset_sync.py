# -*- coding: utf-8 -*-
"""Tests for staged GitHub Release asset synchronization."""

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
from tools import release_asset_sync as sync


VERSION = (ROOT / "VERSION").read_text(encoding="utf-8").strip()
TAG = f"v{VERSION}"
COMMIT = "abc123def4567890abcdef1234567890abcdef12"
PREFIX = sync.staging_prefix(COMMIT)


class FakeGh:
    def __init__(self):
        self.assets = {}
        self.next_id = 1000
        self.commands = []

    def __call__(self, command, **kwargs):
        self.commands.append(list(command))
        if command[0] == "api" and "releases/tags/" in command[1]:
            return self._json({"assets": list(self.assets.values())})
        if command[:2] == ["release", "upload"]:
            path = Path(command[3])
            self._upsert(path.name, path.stat().st_size)
            return self._json({})
        if command[:3] == ["api", "--method", "DELETE"]:
            asset_id = int(command[-1].rsplit("/", 1)[-1])
            self._remove_by_id(asset_id)
            return self._json({})
        if command[:3] == ["api", "--method", "PATCH"]:
            asset_id = int(command[3].rsplit("/", 1)[-1])
            name = json.loads(kwargs["input"])["name"]
            old_size = next(
                payload["size"] for payload in self.assets.values()
                if payload["id"] == asset_id
            )
            self._remove_by_id(asset_id)
            self._upsert(name, old_size, asset_id)
            return self._json({})
        raise AssertionError(f"unsupported gh command: {command}")

    def _upsert(self, name, size, asset_id=None):
        asset_id = self.next_id if asset_id is None else asset_id
        self.assets[name] = {
            "id": asset_id,
            "name": name,
            "size": size,
            "state": "uploaded",
            "url": f"https://api.github.com/assets/{asset_id}",
        }
        if asset_id == self.next_id:
            self.next_id += 1

    def _remove_by_id(self, asset_id):
        for name, payload in list(self.assets.items()):
            if payload["id"] == asset_id:
                del self.assets[name]
                return
        raise AssertionError(f"unknown asset id {asset_id}")

    @staticmethod
    def _json(payload):
        class Result:
            stdout = json.dumps(payload)
            returncode = 0
            stderr = ""
        return Result()


def make_assets(root):
    for index, name in enumerate(sorted(sync.payload_assets(VERSION))):
        (root / name).write_bytes(f"payload {index}".encode("utf-8"))


class ReleaseAssetSyncTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "assets"
        self.root.mkdir()
        make_assets(self.root)

    def test_prepare_validates_and_builds_plain_checksums(self):
        checksum = sync.prepare_assets(self.root, VERSION)
        lines = checksum.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 10)
        for line in lines:
            digest, name = line.split("  ", 1)
            self.assertNotIn("release-assets/", name)
            actual = hashlib.sha256((self.root / name).read_bytes()).hexdigest()
            self.assertEqual(digest, actual)

    def test_upload_requires_exact_candidate_set(self):
        (self.root / "unexpected.txt").write_text("bad")
        with self.assertRaises(RuntimeError):
            sync.upload_staged_assets(FakeGh(), TAG, self.root, VERSION, COMMIT, "Natsummerance/readMD")

    def test_missing_release_is_skipped_before_asset_preparation(self):
        class MissingReleaseGh:
            def __init__(self):
                self.calls = []

            def __call__(self, command, **kwargs):
                self.calls.append(list(command))

                class Result:
                    stdout = ""
                    stderr = "gh: Not Found (HTTP 404)"
                    returncode = 1

                return Result()

        gh = MissingReleaseGh()
        (self.root / "unexpected.txt").write_text("bad", encoding="utf-8")

        self.assertIsNone(sync.fetch_release(gh, TAG, "Natsummerance/readMD"))
        self.assertEqual(gh.calls, [["api", f"repos/Natsummerance/readMD/releases/tags/{TAG}"]])

    def test_stages_before_replacing_public_assets(self):
        gh = FakeGh()
        public_names = set(sync.expected_assets(VERSION))
        for index, name in enumerate(public_names):
            gh._upsert(name, 1, 900 + index)

        sync.prepare_assets(self.root, VERSION)
        prefix, staged = sync.upload_staged_assets(gh, TAG, self.root, VERSION, COMMIT, "Natsummerance/readMD")
        self.assertEqual(prefix, PREFIX)
        self.assertEqual({asset.name for asset in staged.values()}, {PREFIX + name for name in sync.expected_assets(VERSION)})

        public_after_upload = {
            name for name in gh.assets
            if not name.startswith(sync.STAGING_MARKER)
        }
        self.assertEqual(public_after_upload, public_names)

        sync.swap_staged_assets(gh, TAG, VERSION, COMMIT, "Natsummerance/readMD")
        final = {
            name for name in gh.assets
            if not name.startswith(sync.STAGING_MARKER)
        }
        self.assertEqual(final, sync.expected_assets(VERSION))
        self.assertFalse(any(name.startswith(sync.STAGING_MARKER) for name in gh.assets))


if __name__ == "__main__":
    unittest.main()
