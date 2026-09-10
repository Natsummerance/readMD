"""Copy to the pinned ReadMD checkout: tests/test_audit_regressions.py."""
import io
import json
import os
from pathlib import Path
import sys
import time
from unittest.mock import patch

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'packages' / 'mcp-server'))

from src.readmd_modules import code_chunk_runner as runner
from src.readmd_modules.import_processor import ImportProcessor
from src.readmd_modules.mdexport.parser import parse, inline_text
import readmd_mcp_server as mcp


@pytest.mark.skipif(os.name == 'nt', reason='POSIX session escape; run Windows job tests separately')
def test_reproduce_bug_001():
    # The descendant exits itself: no permanent orphan is created by this test.
    child = 'import time; time.sleep(2.5)'
    code = ('import subprocess,sys; '
            f'subprocess.Popen([sys.executable,"-c",{child!r}],start_new_session=True); '
            'print("done",flush=True)')
    started = time.monotonic()
    try:
        runner._run_process([sys.executable, '-c', code], timeout=1)
        elapsed = time.monotonic() - started
    finally:
        time.sleep(max(0, 2.8 - (time.monotonic() - started)))
    assert elapsed < 1.8, f'pipe cleanup exceeded wall-clock budget: {elapsed:.3f}s'


@pytest.mark.parametrize('kind', ['regular', 'symlink'])
def test_reproduce_bug_002(tmp_path, monkeypatch, kind):
    target = tmp_path / 'report.tex'
    victim = tmp_path / 'unrelated.txt'
    victim.write_text('KEEP', encoding='utf-8')
    if kind == 'symlink':
        probe = tmp_path / 'probe'
        try:
            probe.symlink_to(victim)
        except OSError:
            pytest.skip('symlink privilege unavailable')
        probe.unlink()

    def render(*args, **kwargs):
        # Deterministic interleaving: file appears after validation, before open.
        if kind == 'symlink':
            target.symlink_to(victim)
        else:
            target.write_text('KEEP', encoding='utf-8')
        return 'REPLACED'

    monkeypatch.setattr(mcp.texmd, 'markdown_to_latex', render)
    result = mcp.handle_tool_call('readmd_export_document', {
        'confirm': True, 'overwrite': False, 'output_format': 'tex',
        'output_path': str(target), 'markdown_content': '# test',
    })
    protected = victim if kind == 'symlink' else target
    assert protected.read_text(encoding='utf-8') == 'KEEP'
    assert result.get('isError') is True


def test_reproduce_bug_003(monkeypatch):
    # Acceptance policy: at most eight active workers, no unbounded queue.
    # Fake workers remain pending without creating OS threads.
    created = []
    class PendingThread:
        def __init__(self, *args, **kwargs):
            created.append(self)
        def start(self):
            pass
    messages = [
        {'jsonrpc': '2.0', 'id': i, 'method': 'tools/call',
         'params': {'name': 'readmd_fix_markdown', 'arguments': {'content': 'x'}}}
        for i in range(20)
    ]
    source = io.StringIO(''.join(json.dumps(m) + '\n' for m in messages))
    sink = io.StringIO()
    monkeypatch.setattr(mcp.threading, 'Thread', PendingThread)
    monkeypatch.setattr(mcp.sys, 'stdin', source)
    monkeypatch.setattr(mcp.sys, 'stdout', sink)
    mcp._CANCEL_EVENTS.clear()
    try:
        mcp.run_stdio_server()
        assert len(created) <= 8, f'{len(created)} pending workers were admitted'
        replies = [json.loads(line) for line in sink.getvalue().splitlines()]
        rejected = [r for r in replies if 'error' in r]
        assert len(rejected) == 12
        assert all(r['error']['code'] == -32000 for r in rejected)
        assert len({r['id'] for r in rejected}) == 12
    finally:
        mcp._CANCEL_EVENTS.clear()


def test_reproduce_bug_005():
    # In SQLite, a backslash does not escape the closing single quote.
    result = runner.execute_code_chunk("SELECT 'tail\\' AS value; SELECT 42 AS sentinel;", lang='sql')
    assert result['ok'], result
    assert 'tail\\' in result['stdout']
    assert '42' in result['stdout']


def test_reproduce_bug_006():
    table = parse('| key | value |\n| --- | --- |\n| a\\|b | KEEP |')[0]
    cells = [inline_text(cell) for cell in table['rows'][0]]
    assert cells == ['a|b', 'KEEP']
    assert len(table['rows'][0]) == len(table['header'])


def test_reproduce_bug_007():
    blocks = parse('```text\nhello\n```not-a-closer\nKEEP\n```')
    assert blocks == [{'type': 'code', 'lang': 'text',
                       'content': 'hello\n```not-a-closer\nKEEP'}]


def test_reproduce_bug_008():
    blocks = parse('>' * 1500 + ' KEEP')
    assert blocks
    # Traverse iteratively so the regression test itself has no recursion limit.
    stack = list(blocks)
    text = []
    while stack:
        block = stack.pop()
        stack.extend(block.get('blocks', []))
        if 'text' in block:
            text.append(inline_text(block['text']))
    assert 'KEEP' in ''.join(text)


def test_reproduce_bug_009(tmp_path):
    (tmp_path / 'child.md').write_text('IMPORTED', encoding='utf-8')
    processor = ImportProcessor(str(tmp_path))
    lf = processor.process('@import "child.md"\n')
    crlf = processor.process('@import "child.md"\r\n')
    assert 'IMPORTED' in lf
    assert crlf.replace('\r\n', '\n') == lf
