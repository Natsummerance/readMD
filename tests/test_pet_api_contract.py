# -*- coding: utf-8 -*-
"""The desktop bridge must expose safe pet readiness and batch seams."""

import json

import readmd


def test_api_refuses_live2d_without_a_verified_original_model(monkeypatch):
    monkeypatch.setattr(readmd, "verify_model_bundle", lambda _path: {
        "ready": False,
        "code": "model_manifest_missing",
    })
    api = readmd.Api()

    result = api.configure_pet({"enabled": True, "renderer": "live2d"})

    assert result == {"ok": False, "code": "model_manifest_missing"}
    assert api.get_pet_runtime_status()["enabled"] is False


def test_api_can_enable_copied_hermes_sprite_without_live2d_model(monkeypatch):
    monkeypatch.setattr(readmd, "verify_model_bundle", lambda _path: {
        "ready": False,
        "code": "cubism_publication_license_pending",
    })
    api = readmd.Api()
    monkeypatch.setattr(api._pet_launcher, "start", lambda: {
        "ok": True, "runtime": {"available": True, "running": True},
    })
    monkeypatch.setattr(api._pet_bridge, "publish", lambda runtime, **_kwargs: runtime)
    monkeypatch.setattr(api, "_start_pet_command_loop", lambda: None)

    result = api.configure_pet({"enabled": True})

    assert result["ok"] is True
    assert result["renderer"] == "hermes-sprite"
    assert result["model"]["ready"] is False
    assert api.get_pet_runtime_status()["enabled"] is True


def test_hermes_sprite_uses_compact_cell_scale_when_no_user_scale_is_saved(monkeypatch, tmp_path):
    monkeypatch.setattr(readmd, "SETTINGS_FILE", str(tmp_path / "settings.json"))
    api = readmd.Api()

    assert api._pet_preferences()["info"]["scale"] == 0.33


def test_pet_plugin_file_picker_returns_only_a_user_selected_zip(monkeypatch):
    api = readmd.Api()

    class Window:
        def create_file_dialog(self, mode, file_types=()):
            assert file_types == ('ReadMD desktop pet (*.zip)',)
            return ('C:/downloads/readmd-pet.zip',)

    fake_webview = type('Webview', (), {'OPEN_DIALOG': 1})
    monkeypatch.setitem(__import__('sys').modules, 'webview', fake_webview)
    api._window = Window()

    assert api.choose_pet_plugin() == 'C:/downloads/readmd-pet.zip'


def test_api_exposes_non_destructive_pet_batch_queue(tmp_path):
    source = tmp_path / "note.md"
    source.write_text("unchanged", encoding="utf-8")
    api = readmd.Api()

    result = api.enqueue_pet_files([str(source)])

    assert result["ok"] is True
    assert result["tasks"][0]["source_path"] == str(source)
    assert source.read_text(encoding="utf-8") == "unchanged"


def test_api_queues_pet_drop_for_explicit_batch_confirmation(monkeypatch, tmp_path):
    markdown = tmp_path / "note.md"
    document = tmp_path / "paper.pdf"
    markdown.write_text("# note", encoding="utf-8")
    document.write_bytes(b"pdf")
    api = readmd.Api()
    opened, converted = [], []
    monkeypatch.setattr(readmd, "push_control", opened.append)
    monkeypatch.setattr(readmd, "_start_convert_job", lambda paths, overwrite=False: converted.append((paths, overwrite)) or "p1")
    notified = []
    monkeypatch.setattr(readmd, "push_pet_batch", lambda paths: notified.append(list(paths)) or True)
    api._pet_bridge.command_path.parent.mkdir(parents=True, exist_ok=True)
    api._pet_bridge.command_path.write_text(
        json.dumps({"command": {"type": "drop", "paths": [str(markdown), str(document)]}}), encoding="utf-8"
    )

    result = api._drain_pet_command()

    assert result == {
        "type": "drop", "accepted": 2,
        "queued": {"markdown": 1, "convert": 1},
        "requires_confirmation": True,
    }
    assert opened == []
    assert converted == []
    assert notified == [[str(markdown), str(document)]]
    grouped = api.get_pet_batch()
    assert [item["source_path"] for item in grouped["markdown"]] == [str(markdown)]
    assert [item["source_path"] for item in grouped["convert"]] == [str(document)]


def test_pet_batch_control_queue_is_separate_from_file_open_queue(monkeypatch):
    monkeypatch.setitem(readmd._CONTROL, "pet_batches", [])
    monkeypatch.setitem(readmd._CONTROL, "queue", [])
    monkeypatch.setitem(readmd._CONTROL, "window", None)
    monkeypatch.setitem(readmd._CONTROL, "ready", False)

    assert readmd.push_pet_batch(["C:/safe/a.md", "C:/safe/b.pdf"]) is True
    assert readmd.pop_control() is None
    assert readmd.pop_pet_batch() == ["C:/safe/a.md", "C:/safe/b.pdf"]
    assert readmd.pop_pet_batch() is None


def test_pet_quick_menu_queue_is_separate_from_file_open_queue(monkeypatch):
    monkeypatch.setitem(readmd._CONTROL, "pet_menus", 0)
    monkeypatch.setitem(readmd._CONTROL, "queue", [])
    monkeypatch.setitem(readmd._CONTROL, "window", None)
    monkeypatch.setitem(readmd._CONTROL, "ready", False)

    assert readmd.push_pet_menu() is True
    assert readmd.pop_control() is None
    assert readmd.pop_pet_menu() is True
    assert readmd.pop_pet_menu() is False


def test_pet_open_menu_command_uses_existing_ui_menu(monkeypatch):
    api = readmd.Api()
    notified = []
    monkeypatch.setattr(readmd, "push_pet_menu", lambda: notified.append(True) or True)
    api._pet_bridge.command_path.parent.mkdir(parents=True, exist_ok=True)
    api._pet_bridge.command_path.write_text(
        json.dumps({"command": {"type": "open-menu"}}), encoding="utf-8"
    )

    assert api._drain_pet_command() == {"type": "open-menu", "ok": True}
    assert notified == [True]


def test_pet_drag_and_scale_commands_are_saved_for_the_next_launch(monkeypatch, tmp_path):
    api = readmd.Api()
    saved = []
    monkeypatch.setattr(api, "save_settings", lambda value: saved.append(value) or True)
    api._pet_bridge.command_path.parent.mkdir(parents=True, exist_ok=True)
    api._pet_bridge.command_path.write_text(
        json.dumps({"command": {"type": "bounds", "bounds": {"x": 31, "y": 42, "width": 320, "height": 400}}}),
        encoding="utf-8",
    )
    assert api._drain_pet_command() == {"type": "bounds", "ok": True}
    api._pet_bridge.command_path.write_text(
        json.dumps({"command": {"type": "scale", "scale": 0.42}}), encoding="utf-8"
    )
    assert api._drain_pet_command() == {"type": "scale", "ok": True}
    assert saved == [
        {"pet_bounds": {"x": 31, "y": 42, "width": 320, "height": 400}},
        {"pet_scale": 0.42},
    ]


def test_api_materializes_explicit_pet_clipboard_text_under_data_dir(monkeypatch, tmp_path):
    api = readmd.Api()
    opened = []
    monkeypatch.setattr(readmd, "DATA_DIR", str(tmp_path))
    monkeypatch.setattr(readmd, "push_control", opened.append)

    result = api._open_pet_clipboard({"text": "# clipped", "image_png": "", "paths": []})

    assert result["accepted"] == 1
    assert result["path"].startswith(str(tmp_path))
    assert open(result["path"], encoding="utf-8").read() == "# clipped"
    assert opened == [result["path"]]


def test_api_rejects_invalid_pet_clipboard_image(monkeypatch, tmp_path):
    api = readmd.Api()
    monkeypatch.setattr(readmd, "DATA_DIR", str(tmp_path))

    assert api._open_pet_clipboard({"text": "", "image_png": "not-base64", "paths": []}) == {
        "type": "clipboard", "accepted": 0, "code": "clipboard_image_write_failed"
    }


def test_pet_mixed_clipboard_keeps_text_and_queues_each_file_kind(monkeypatch, tmp_path):
    markdown = tmp_path / "from-clipboard.md"
    document = tmp_path / "from-clipboard.pdf"
    markdown.write_text("# existing", encoding="utf-8")
    document.write_bytes(b"pdf")
    api = readmd.Api()
    opened, notified = [], []
    monkeypatch.setattr(readmd, "DATA_DIR", str(tmp_path / "data"))
    monkeypatch.setattr(readmd, "push_control", opened.append)
    monkeypatch.setattr(readmd, "push_pet_batch", lambda paths: notified.append(list(paths)) or True)
    monkeypatch.setattr(api, "_record_pet_event", lambda _event: None)

    result = api._open_pet_clipboard({
        "text": "# copied text", "image_png": "", "paths": [str(markdown), str(document)],
    })

    assert result["accepted"] == 3
    assert result["batch"] == {
        "type": "drop", "accepted": 2, "queued": {"markdown": 1, "convert": 1},
        "requires_confirmation": True,
    }
    assert notified == [[str(markdown), str(document)]]
    assert len(opened) == 1
    assert open(opened[0], encoding="utf-8").read() == "# copied text"
