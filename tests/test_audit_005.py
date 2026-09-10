from src.readmd_modules.import_processor import process_markdown_imports

def test_reproduce_bug_005(tmp_path):
    (tmp_path / 'app.py').write_text('print(1)', encoding='utf-8')
    out = process_markdown_imports(
        '@import "app.py" {line_begin=²}', base_dir=str(tmp_path))
    assert 'invalid_line_range' in out
