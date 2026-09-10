"""Regression coverage for automatic runtime setup and exclusive OCR selection."""
import json

import pytest
import readmd
from src.readmd_modules import plugin_manager as pm, ocr


@pytest.fixture
def plugin_state(monkeypatch, tmp_path):
    monkeypatch.setattr(pm, 'PLUGINS_MANIFEST', str(tmp_path / 'plugins.json'))
    monkeypatch.setattr(pm, '_ensure_dirs', lambda: None)
    monkeypatch.setattr(pm, 'is_plugin_installed', lambda pid: pid in pm.PLUGIN_SPECS)
    monkeypatch.setattr(pm, '_check_model_cached', lambda pid: False)


def test_switch_ocr_excludes_previous_engine_but_keeps_table(plugin_state):
    pm.save_manifest({'rapidocr': {'enabled': True}, 'rapid_table': {'enabled': True}})
    assert pm.set_plugin_enabled('easyocr', True)
    state = pm.load_manifest()
    assert state['easyocr']['enabled']
    assert not state['rapidocr']['enabled']
    assert state['rapid_table']['enabled']
    assert pm.set_plugin_enabled('rapidocr', True)
    assert not pm.load_manifest()['easyocr']['enabled']


def test_new_install_and_legacy_manifest_are_exclusive(plugin_state):
    pm.save_manifest({'rapidocr': {'enabled': True}, 'easyocr': {'enabled': True}})
    assert sum(pm.load_manifest()[pid]['enabled'] for pid in ('easyocr', 'rapidocr')) == 1
    pm._mark_installed('easyocr', '1.0')
    assert pm.load_manifest()['easyocr']['enabled']
    assert not pm.load_manifest()['rapidocr']['enabled']


def test_enabled_ocr_runs_before_native_engine(plugin_state, monkeypatch):
    assert pm.set_plugin_enabled('easyocr', True)
    monkeypatch.setattr(ocr, '_ocr_easyocr', lambda data: ' selected text ')
    monkeypatch.setattr(ocr, '_ocr_bytes', lambda data: pytest.fail('native engine bypassed selection'))
    assert ocr._ocr_cascade(b'image') == 'selected text'


def test_default_install_uses_sidecar_without_picker(monkeypatch, tmp_path):
    monkeypatch.setattr(readmd, 'APP_DIR', str(tmp_path))
    monkeypatch.setattr(readmd, 'DATA_DIR', str(tmp_path / 'data'))
    archive = tmp_path / 'ReadMD-Desktop-Pet.zip'
    archive.write_bytes(b'test')
    api = readmd.Api()
    monkeypatch.setattr(api._pet_launcher, 'status', lambda: {'available': False})
    monkeypatch.setattr(api, 'choose_pet_plugin', lambda: pytest.fail('no picker'))
    calls = []
    monkeypatch.setattr(api, 'install_pet_plugin', lambda path, confirm: calls.append((path, confirm)) or {'ok': True})
    assert api.install_default_pet_plugin()['ok']
    assert calls == [(str(archive), True)]
    assert api._pet_installer.target == tmp_path / 'data' / 'plugins' / 'pet' / 'hermes-adapter'


def test_companion_install_preserves_selected_live2d(monkeypatch, tmp_path):
    monkeypatch.setattr(readmd, 'DATA_DIR', str(tmp_path))
    monkeypatch.setattr(readmd, 'SETTINGS_FILE', str(tmp_path / 'settings.json'))
    readmd.save_json(readmd.SETTINGS_FILE, {'pet_renderer': 'live2d'})
    api = readmd.Api()
    monkeypatch.setattr(api, 'install_default_pet_plugin', lambda: {'ok': True})
    configured = []
    monkeypatch.setattr(api, 'configure_pet', lambda settings: configured.append(settings) or {'ok': True})
    assert api.install_companion_pet()['ok']
    assert configured == [{'enabled': True, 'in_app': False, 'renderer': 'live2d'}]


def test_model_readiness_uses_installed_extension(monkeypatch, tmp_path):
    monkeypatch.setattr(readmd, 'DATA_DIR', str(tmp_path))
    api = readmd.Api()
    model = api._pet_installer.target / 'app' / 'models' / 'arch-chan'
    model.mkdir(parents=True)
    monkeypatch.setattr(readmd, 'verify_model_bundle', lambda path: {'ready': path == model})
    assert api._pet_model_status()['ready']


def test_import_failure_is_not_install_success(monkeypatch):
    def broken_import(name):
        raise ImportError('missing native dependency')
    monkeypatch.setattr(pm, 'mount_sandbox', lambda: None)
    monkeypatch.setattr(pm.importlib, 'import_module', broken_import)
    monkeypatch.setattr(pm, '_install_tasks', {})
    assert not pm._verify_plugin_import('rapidocr', pm.PLUGIN_SPECS['rapidocr'])
    assert pm._install_tasks['rapidocr']['status'] == 'error'


def test_unconnected_plugins_are_not_reported_enabled(plugin_state):
    assert not pm.load_manifest()['pygments']['enabled']
    assert not pm.load_manifest()['pygments']['runtime_connected']
