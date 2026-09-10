from src.readmd_modules.import_processor import ImportProcessor

def test_reproduce_bug_006(tmp_path):
    (tmp_path / "legacy.md").write_bytes("中文文档".encode("gbk"))
    result = ImportProcessor(str(tmp_path)).process('@import "legacy.md"')
    # 可采用严格拒绝或显式编码支持，不能静默损坏。
    assert "中文文档" in result or "unsupported_encoding" in result
    assert "\ufffd" not in result
