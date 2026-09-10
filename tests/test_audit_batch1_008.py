from src.readmd_modules import code_chunk_runner as runner

def test_reproduce_bug_008(monkeypatch):
    expected = {"ok": True, "stdout": "https://example.com"}
    monkeypatch.setattr(runner, "_execute_code_chunk", lambda *a, **kw: expected)
    result = runner.execute_code_chunk('print("https://example.com")', capture_plot=False)
    assert result["ok"] is True
