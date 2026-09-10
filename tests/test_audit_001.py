from src.readmd_modules import code_chunk_runner as runner

def test_reproduce_bug_001(monkeypatch):
    dispatched = []
    monkeypatch.setattr(runner, '_execute_code_chunk',
        lambda *a, **kw: dispatched.append(a) or {'ok': True})
    result = runner.execute_code_chunk(
        'm = __import__("soc" + "ket"); m.socket()',
        capture_plot=False)
    assert not dispatched
    assert result.get('error') == 'network_not_allowed'
