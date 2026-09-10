import pytest
from src.readmd_modules.import_processor import process_markdown_imports

@pytest.mark.parametrize('limit', [0, 1, 8, 16])
def test_reproduce_bug_004(tmp_path, limit):
    out = process_markdown_imports(
        'x' * 100, base_dir=str(tmp_path), max_output_bytes=limit)
    assert len(out.encode('utf-8')) <= limit
