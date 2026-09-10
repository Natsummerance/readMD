from src.readmd_modules.import_processor import ImportProcessor

def test_reproduce_bug_001(tmp_path):
    (tmp_path / "leaf.md").write_text("x" * 4096, encoding="utf-8")
    child = "leaf.md"
    for i in range(4):
        name = f"level{i}.md"
        (tmp_path / name).write_text(
            (f'@import "{child}"\n') * 4, encoding="utf-8")
        child = name
    # 修复新增共享预算参数；当前实现不接受此参数。
    processor = ImportProcessor(str(tmp_path), max_output_bytes=65536)
    result = processor.process(f'@import "{child}"')
    assert len(result.encode("utf-8")) <= 65536
    assert "import_budget_exceeded" in result
