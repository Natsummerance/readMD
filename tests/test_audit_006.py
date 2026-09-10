import builtins
import pytest
from src.readmd_modules import code_chunk_runner as runner

def test_reproduce_bug_006(tmp_path, monkeypatch):
    def fail(*a, **kw):
        raise OSError('disk full')
    monkeypatch.setattr(builtins, 'open', fail)
    with pytest.raises(OSError):
        runner._write_temp_script('.py', 'print(1)', str(tmp_path))
    assert list(tmp_path.iterdir()) == []
