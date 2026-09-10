from src.readmd_modules.import_processor import ImportProcessor

def test_reproduce_bug_002(tmp_path):
    (tmp_path / "sample.md").write_text("private-content", encoding="utf-8")
    source = '```markdown\n@import "sample.md"\n```'
    assert ImportProcessor(str(tmp_path)).process(source) == source
