# -*- coding: utf-8 -*-
"""Verify that Vega and Vega-Lite render correctly in the ReadMD webview client."""

import json
import pytest
from src.readmd_modules.diagrams import render_vega_svg

def test_vega_lite_server_svg():
    spec = {
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "data": {"values": [{"a": "A", "b": 28}, {"a": "B", "b": 55}]},
        "mark": "bar",
        "encoding": {
            "x": {"field": "a", "type": "nominal"},
            "y": {"field": "b", "type": "quantitative"}
        }
    }
    svg = render_vega_svg(json.dumps(spec), "vega-lite")
    assert svg.startswith("<svg")
    assert "</svg>" in svg
    assert "A" in svg
    assert "B" in svg

def test_vega_server_svg():
    spec = {
        "$schema": "https://vega.github.io/schema/vega/v5.json",
        "width": 400,
        "height": 200,
        "padding": 5,
        "data": [
            {
                "name": "table",
                "values": [
                    {"category": "A", "amount": 28},
                    {"category": "B", "amount": 55}
                ]
            }
        ],
        "signals": [],
        "scales": [
            {
                "name": "xscale",
                "type": "band",
                "domain": {"data": "table", "field": "category"},
                "range": "width",
                "padding": 0.05
            },
            {
                "name": "yscale",
                "domain": {"data": "table", "field": "amount"},
                "nice": True,
                "range": "height"
            }
        ],
        "axes": [
            {"orient": "bottom", "scale": "xscale"},
            {"orient": "left", "scale": "yscale"}
        ],
        "marks": [
            {
                "type": "rect",
                "from": {"data": "table"},
                "encode": {
                    "enter": {
                        "x": {"scale": "xscale", "field": "category"},
                        "width": {"scale": "xscale", "band": 1},
                        "y": {"scale": "yscale", "field": "amount"},
                        "y2": {"scale": "yscale", "value": 0},
                        "fill": {"value": "steelblue"}
                    }
                }
            }
        ]
    }
    svg = render_vega_svg(json.dumps(spec), "vega")
    assert svg.startswith("<svg")
    assert "</svg>" in svg
