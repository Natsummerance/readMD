import builtins
import pytest
from src.readmd_modules import code_chunk_runner as runner

def test_reproduce_bug_007(tmp_path, monkeypatch):
    original = builtins.open
    def guarded(path, *a, **kw):
        if str(path).endswith('query.sql'):
            raise OSError('disk full')
        return original(path, *a, **kw)
    monkeypatch.setattr(builtins, 'open', guarded)
    with pytest.raises(OSError):
        runner._execute_sql_chunk('SELECT 1', cwd=str(tmp_path))
    assert list(tmp_path.iterdir()) == []
