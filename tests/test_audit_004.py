from src.readmd_modules.import_processor import ImportProcessor

import sys
from types import SimpleNamespace
from unittest.mock import MagicMock

def test_reproduce_bug_004(tmp_path, monkeypatch):
    (tmp_path / "a.pdf").write_bytes(b"%PDF-1.4")
    doc = MagicMock()
    doc.__len__.return_value = 1
    doc.__enter__.return_value = doc
    doc.__exit__.side_effect = lambda *args: doc.close()
    doc.__getitem__.return_value.get_pixmap.side_effect = RuntimeError("render failed")
    monkeypatch.setitem(sys.modules, "fitz", SimpleNamespace(open=lambda _: doc))
    ImportProcessor(str(tmp_path)).process('@import "a.pdf"')
    doc.close.assert_called_once()
