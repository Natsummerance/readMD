# -*- coding: utf-8 -*-
"""Offline diagram renderer contracts."""

import json
import shutil

import pytest

from src.readmd_modules.diagrams import (
    DiagramRenderError,
    get_diagram_capabilities,
    identify_diagram_blocks,
    render_plantuml_svg,
    render_vega_svg,
)


def test_frozen_capability_does_not_use_developer_path(monkeypatch):
    import src.readmd_modules.diagrams as diagrams

    monkeypatch.setattr(diagrams.sys, "frozen", True, raising=False)
    assert diagrams._node_runtime(diagrams.Path.cwd()) is None


def test_capabilities_are_local_and_expose_explicit_fallbacks():
    capabilities = get_diagram_capabilities()
    assert capabilities["schema_version"] == 1
    assert capabilities["offline"] is True
    engines = capabilities["engines"]
    # Every engine has an explicit state; no client needs to infer support
    # from a missing key or attempt an implicit online fallback.
    for engine in ("mermaid", "wavedrom", "bitfield", "viz", "tikz",
                   "chart", "chartjs", "chart.js", "vega", "vega-lite",
                   "plantuml", "puml", "wsd", "d2"):
        assert engine in engines
        assert isinstance(engines[engine]["available"], bool)
        assert isinstance(engines[engine]["offline"], bool)
        assert isinstance(engines[engine]["requires_network"], bool)
    assert engines["d2"]["available"] is False
    assert engines["d2"]["offline"] is False
    assert engines["wsd"]["available"] is False


def test_chartjs_fence_is_discoverable_and_uses_canonical_capability():
    blocks = identify_diagram_blocks('```chartjs\n{"type":"bar"}\n```')
    assert blocks and blocks[0]["type"] == "chartjs"
    capabilities = get_diagram_capabilities()["engines"]
    assert capabilities["chartjs"] == capabilities["chart"]
    assert capabilities["chart.js"] == capabilities["chart"]


def test_wsd_fence_is_discoverable_but_fails_closed_without_a_renderer():
    blocks = identify_diagram_blocks('```wsd\nAlice->Bob: ping\n```')
    assert blocks and blocks[0]["type"] == "wsd"


def test_local_plantuml_uses_shell_free_pipe(monkeypatch):
    import src.readmd_modules.diagrams as diagrams

    class Result:
        returncode = 0
        stdout = b'<svg xmlns="http://www.w3.org/2000/svg"></svg>'

    monkeypatch.setattr(diagrams, "_plantuml_command", lambda: ["plantuml", "-tsvg", "-pipe"])
    calls = []

    def fake_run(command, **kwargs):
        calls.append((command, kwargs))
        return Result()

    monkeypatch.setattr(diagrams.subprocess, "run", fake_run)
    svg = render_plantuml_svg("Alice -> Bob: ping")
    assert svg.startswith("<svg")
    assert calls and calls[0][0][0] == "plantuml"
    assert calls[0][1]["input"].startswith(b"@startuml")


def test_pywebview_capabilities_bridge_matches_module_snapshot():
    import readmd

    result = readmd.Api().get_diagram_capabilities()
    assert result["ok"] is True
    assert result["engines"] == get_diagram_capabilities()["engines"]


def test_wsd_api_does_not_mislabel_plantuml_as_a_success():
    import readmd

    result = readmd.Api().render_diagram("wsd", "Alice->Bob: ping")
    assert result == {"ok": False, "error_code": "diagram_engine_unavailable", "engine": "wsd"}


@pytest.mark.skipif(shutil.which("node") is None, reason="Node runtime is not installed")
def test_vega_lite_renders_to_svg_without_network():
    spec = {
        "data": {"values": [{"label": "A", "value": 28}, {"label": "B", "value": 55}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "label", "type": "nominal"},
            "y": {"field": "value", "type": "quantitative"},
        },
    }
    svg = render_vega_svg(json.dumps(spec), "vega-lite")
    assert svg.startswith("<svg")
    assert "A" in svg and "B" in svg


@pytest.mark.skipif(shutil.which("node") is None, reason="Node runtime is not installed")
def test_vega_invalid_spec_returns_stable_error_code():
    with pytest.raises(DiagramRenderError) as error:
        render_vega_svg("{\"mark\":\"not-a-real-mark\"}", "vega-lite")
    assert error.value.code == "diagram_render_failed"


def test_browser_only_diagram_is_never_reported_as_server_success():
    with pytest.raises(DiagramRenderError) as error:
        render_vega_svg("[]", "unknown")
    assert error.value.code == "diagram_engine_invalid"


@pytest.mark.skipif(shutil.which("node") is None, reason="Node runtime is not installed")
def test_pywebview_bridge_returns_vega_svg_or_stable_failure():
    import readmd

    spec = {"data": {"values": [{"x": "A", "y": 1}]}, "mark": "bar",
            "encoding": {"x": {"field": "x", "type": "nominal"},
                          "y": {"field": "y", "type": "quantitative"}}}
    result = readmd.Api().render_diagram("vega-lite", json.dumps(spec))
    assert result["ok"] is True
    assert result["type"] == "svg"
    assert result["svg"].startswith("<svg")
