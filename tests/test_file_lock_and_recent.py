import os
import tempfile
import json
import pytest
import readmd
from readmd import Api, read_text
import src.readmd_core.config as config


def test_api_recent_operations(monkeypatch):
    """Test get_recent, add_recent, remove_recent, clear_recent."""
    with tempfile.TemporaryDirectory() as tmpdir:
        recent_json = os.path.join(tmpdir, 'recent.json')
        monkeypatch.setattr(config, 'RECENT_FILE', recent_json)
        monkeypatch.setattr(readmd, 'RECENT_FILE', recent_json)

        api = Api()
        file1 = os.path.join(tmpdir, 'doc1.md')
        file2 = os.path.join(tmpdir, 'doc2.md')
        file3 = os.path.join(tmpdir, 'doc3.md')
        for f in (file1, file2, file3):
            with open(f, 'w', encoding='utf-8') as handle:
                handle.write('# Test')

        # Add entries
        api.add_recent(file1)
        api.add_recent(file2)
        api.add_recent(file3)

        entries = api.get_recent()
        assert len(entries) == 3
        assert entries[0] == file3

        # Remove single entry
        assert api.remove_recent(file2) is True
        entries_after = api.get_recent()
        assert len(entries_after) == 2
        assert file2 not in entries_after
        assert file1 in entries_after
        assert file3 in entries_after

        # Clear all
        api.clear_recent()
        assert api.get_recent() == []


def test_api_check_recent_status_exists_moved_deleted():
    """Test check_recent_status detecting existing, moved, and deleted files."""
    with tempfile.TemporaryDirectory() as tmpdir:
        # 1. Existing file
        existing_file = os.path.join(tmpdir, 'existing.md')
        with open(existing_file, 'w', encoding='utf-8') as handle:
            handle.write('# Existing')

        # 2. Moved file: original path doesn't exist, but moved into subfolder
        subdir = os.path.join(tmpdir, 'subfolder')
        os.makedirs(subdir, exist_ok=True)
        old_moved_path = os.path.join(tmpdir, 'moved_file.md')
        new_moved_path = os.path.join(subdir, 'moved_file.md')
        with open(new_moved_path, 'w', encoding='utf-8') as handle:
            handle.write('# Moved File')

        # 3. Completely deleted file
        deleted_file = os.path.join(tmpdir, 'nonexistent_deleted.md')

        api = Api()
        res = api.check_recent_status([existing_file, old_moved_path, deleted_file])
        assert res.get('ok') is True
        items = res.get('items', [])
        assert len(items) == 3

        item_map = {it['path']: it for it in items}

        # Check existing
        assert item_map[existing_file]['status'] == 'exists'
        assert item_map[existing_file]['resolved_path'] == existing_file

        # Check moved
        assert item_map[old_moved_path]['status'] == 'moved'
        assert os.path.normpath(item_map[old_moved_path]['resolved_path']) == os.path.normpath(new_moved_path)

        # Check deleted
        assert item_map[deleted_file]['status'] == 'deleted'


def test_file_lock_release_and_deletion():
    """Verify that reading a file and processing it releases any OS file lock immediately."""
    with tempfile.TemporaryDirectory() as tmpdir:
        test_file = os.path.join(tmpdir, 'locked_test.md')
        with open(test_file, 'w', encoding='utf-8') as handle:
            handle.write('# Hello Lock Test\n\nContent to read.')

        # Perform read_text like ReadMD does on open
        content, enc = read_text(test_file)
        assert 'Hello Lock Test' in content

        # Verify the file can be immediately removed or renamed without permission error
        os.remove(test_file)
        assert not os.path.exists(test_file)
