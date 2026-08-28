# -*- coding: utf-8 -*-
import json
from pathlib import Path


def test_upstream_manifest_is_allowlisted_and_offline():
    from src.readmd_core import upstream

    sources = upstream.list_sources()
    assert sources
    assert all(item["manifest"] == "assets/upstream/manifest.json" for item in sources)
    source = sources[0]
    detail = upstream.get_source(source["id"])
    assert detail["source_files"]
    entry = detail["source_files"][0]
    content = upstream.get_file(source["id"], entry["id"])
    assert content["sha256"] == entry["sha256"]
    assert isinstance(content["content"], str)


def test_upstream_file_ids_reject_paths_and_unknown_sources():
    from src.readmd_core.upstream import UpstreamSourceError, get_file, get_source

    try:
        get_source("../../etc")
    except UpstreamSourceError:
        pass
    else:
        raise AssertionError("path-like source id was accepted")
    try:
        get_file("missing/source", "../../secret")
    except UpstreamSourceError:
        pass
    else:
        raise AssertionError("arbitrary upstream path was accepted")


def test_provider_catalog_contains_generated_source_entries():
    path = Path(__file__).parents[1] / "assets" / "providers" / "provider-catalog.json"
    catalog = json.loads(path.read_text(encoding="utf-8"))
    assert catalog["schema_version"] == 2
    assert catalog["snapshot_manifest"] == "assets/upstream/manifest.json"
    assert len(catalog["upstream_entries"]) >= 500
    assert all(item.get("source_only") is True for item in catalog["upstream_entries"])
