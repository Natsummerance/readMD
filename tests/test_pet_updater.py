# -*- coding: utf-8 -*-
"""Unit tests for desktop pet auto-updater and directory confinement."""

from __future__ import annotations

import json
import os
import shutil
import tempfile
import zipfile
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest

from src.readmd_modules.pet.hermes_adapter import (
    HermesPetBridge,
    HermesPetLauncher,
    HermesPetPluginInstaller,
    get_app_install_dir,
    get_default_pet_install_root,
)
from src.readmd_modules.pet.updater import (
    apply_pet_update,
    check_pet_update,
    clean_legacy_pet_installations,
    find_bundled_candidate_archives,
)


def _make_dummy_zip(zip_path: Path, manifest_data: dict) -> Path:
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, 'w') as zf:
        zf.writestr('readmd-pet-plugin.json', json.dumps(manifest_data))
        zf.writestr('app/main.js', 'console.log("hello")')
    return zip_path


def test_default_pet_install_root_not_in_c_appdata():
    """Verify that default pet install root resides under app install dir, not %APPDATA%."""
    install_root = get_default_pet_install_root()
    app_dir = get_app_install_dir()
    
    # Must be under app install dir
    assert str(install_root).startswith(str(app_dir))
    assert 'plugins' in install_root.parts


def test_clean_legacy_pet_installations_removes_appdata_dirs(monkeypatch):
    with tempfile.TemporaryDirectory(prefix='readmd-test-legacy-') as tmpdir:
        fake_appdata = Path(tmpdir) / 'AppData' / 'Roaming'
        fake_target = Path(tmpdir) / 'AppInstall' / 'plugins' / 'pet' / 'hermes-adapter'
        fake_target.mkdir(parents=True, exist_ok=True)

        legacy1 = fake_appdata / 'ReadMD' / 'plugins' / 'pet' / 'hermes-adapter'
        legacy2 = fake_appdata / 'ReadMD' / 'pet' / 'hermes-adapter'
        legacy1.mkdir(parents=True, exist_ok=True)
        legacy2.mkdir(parents=True, exist_ok=True)
        (legacy1 / 'dummy.txt').write_text('old')
        (legacy2 / 'dummy2.txt').write_text('old2')

        monkeypatch.setenv('APPDATA', str(fake_appdata))

        cleaned = clean_legacy_pet_installations(fake_target)
        assert len(cleaned) == 2
        assert not legacy1.exists()
        assert not legacy2.exists()
        assert fake_target.exists()


def test_clean_legacy_does_not_remove_active_target_if_same(monkeypatch):
    with tempfile.TemporaryDirectory(prefix='readmd-test-legacy-same-') as tmpdir:
        fake_appdata = Path(tmpdir) / 'AppData' / 'Roaming'
        target = fake_appdata / 'ReadMD' / 'plugins' / 'pet' / 'hermes-adapter'
        target.mkdir(parents=True, exist_ok=True)
        (target / 'keep.txt').write_text('keep')

        monkeypatch.setenv('APPDATA', str(fake_appdata))
        cleaned = clean_legacy_pet_installations(target)
        assert len(cleaned) == 0
        assert target.exists()


def test_check_pet_update_detects_bundled_archive_change():
    with tempfile.TemporaryDirectory(prefix='readmd-test-updater-') as tmpdir:
        work_dir = Path(tmpdir)
        target_dir = work_dir / 'plugins' / 'pet' / 'hermes-adapter'
        target_dir.mkdir(parents=True, exist_ok=True)
        
        # Current installed manifest
        old_manifest = {'name': 'test-pet', 'version': '0.1.0', 'manifest_hash': 'old_hash'}
        (target_dir / 'readmd-pet-plugin.json').write_text(json.dumps(old_manifest))

        installer = HermesPetPluginInstaller(str(work_dir / 'plugins'))
        launcher = MagicMock(spec=HermesPetLauncher)
        launcher.status.return_value = {'available': True, 'running': False}

        # Create bundled candidate zip with different content
        cand_zip = work_dir / 'dist' / 'ReadMD-Desktop-Pet.zip'
        new_manifest = {'name': 'test-pet', 'version': '0.2.0', 'manifest_hash': 'new_hash'}
        _make_dummy_zip(cand_zip, new_manifest)

        check_res = check_pet_update(
            installer=installer,
            launcher=launcher,
            candidate_archives=[cand_zip],
            allow_network=False,
        )

        assert check_res.get('ok') is True
        assert check_res.get('has_update') is True
        assert check_res.get('source') == 'bundled'
        assert check_res.get('archive_path') == str(cand_zip.resolve())


def test_apply_pet_update_bundled_and_restarts_if_running():
    with tempfile.TemporaryDirectory(prefix='readmd-test-apply-') as tmpdir:
        work_dir = Path(tmpdir)
        installer = HermesPetPluginInstaller(str(work_dir / 'plugins'))
        launcher = MagicMock(spec=HermesPetLauncher)
        # Mock currently running
        launcher.status.side_effect = [
            {'running': True, 'available': True},
            {'running': False, 'available': True},
            {'running': True, 'available': True},
        ]

        cand_zip = work_dir / 'ReadMD-Desktop-Pet.zip'
        manifest = {'name': 'test-pet', 'version': '0.2.0', 'main': 'app/main.js', 'files': {'app/main.js': 'dummy'}}
        installer.install_archive = MagicMock(return_value={'ok': True})

        update_info = {
            'ok': True,
            'has_update': True,
            'source': 'bundled',
            'archive_path': str(cand_zip),
        }
        cand_zip.touch()

        res = apply_pet_update(installer, launcher, update_info)
        assert res.get('ok') is True
        assert res.get('updated') is True
        assert launcher.stop.called
        assert launcher.start.called


def test_check_pet_update_github_release():
    installer = MagicMock(spec=HermesPetPluginInstaller)
    installer.target = Path('/fake/path')
    installer.get_installed_manifest_hash.return_value = 'same_hash'
    installer.get_release_info.return_value = {'release_tag': 'v0.1.0'}

    launcher = MagicMock(spec=HermesPetLauncher)
    launcher.status.return_value = {'available': True, 'running': False}

    fake_release = {
        'tag_name': 'v0.2.0',
        'name': 'ReadMD Desktop Pet v0.2.0',
        'body': 'New features added',
        'assets': [
            {
                'name': 'ReadMD-Desktop-Pet.zip',
                'browser_download_url': 'https://github.com/dummy/ReadMD-Desktop-Pet.zip',
                'size': 140000000,
                'updated_at': '2026-09-12T00:00:00Z',
            }
        ]
    }

    with patch('src.readmd_modules.pet.updater._fetch_github_json', return_value=fake_release):
        res = check_pet_update(installer, launcher, candidate_archives=[], allow_network=True)
        assert res.get('ok') is True
        assert res.get('has_update') is True
        assert res.get('source') == 'github'
        assert res.get('version') == 'v0.2.0'
        assert res.get('download_url') == 'https://github.com/dummy/ReadMD-Desktop-Pet.zip'


def test_api_pet_update_methods():
    from readmd import Api
    with patch('threading.Thread'):
        api = Api()
        status = api.get_pet_update_status()
        assert status.get('ok') is True
        assert 'install_path' in status
        assert 'plugins' in status['install_path']
        assert not ('AppData' in status['install_path'] and 'Roaming' in status['install_path'])

        runtime_status = api.get_pet_runtime_status()
        assert 'update' in runtime_status
        assert runtime_status['update']['ok'] is True
