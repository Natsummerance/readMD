from src.readmd_modules.import_processor import ImportProcessor

import pytest

@pytest.mark.parametrize("value", ["abc", "-1", "1.5"])
def test_reproduce_bug_003(tmp_path, value):
    (tmp_path / "a.py").write_text("print(1)", encoding="utf-8")
    output = ImportProcessor(str(tmp_path)).process(
        f'@import "a.py" {{line_begin={value}}}')
    assert "invalid_line_range" in output
