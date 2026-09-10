import sys
import pytest
from src.readmd_modules import code_chunk_runner as runner

@pytest.mark.skipif(sys.platform == 'win32', reason='POSIX selector')
def test_reproduce_bug_003(tmp_path):
    code = ('import os,time; os.close(1); os.close(2); '
            'time.sleep(0.2); open("done", "w").write("ok")')
    result = runner._run_process(
        [sys.executable, '-c', code], cwd=str(tmp_path), timeout=2)
    assert result['ok']
    assert (tmp_path / 'done').read_text() == 'ok'
