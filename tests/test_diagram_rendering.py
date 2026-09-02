# -*- coding: utf-8 -*-
"""Offline diagram renderer contracts."""

import json
import shutil

import pytest

from src.readmd_modules.diagrams import DiagramRenderError, render_vega_svg


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
