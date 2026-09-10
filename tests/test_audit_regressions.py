import io
import json
import os
from pathlib import Path
import shutil
import sys
import time
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'packages' / 'mcp-server'))
from src.readmd_modules import code_chunk_runner as runner
from src.readmd_modules.mdexport import parser
from src.readmd_modules.pet.hermes_adapter import HermesPetBridge
import readmd_mcp_server as mcp


@pytest.mark.skipif(sys.platform != 'linux', reason='POSIX process sessions')
def test_reproduce_bug_001():
    # Finite-lived descendant: even the buggy version self-cleans after 4 s.
    code = (
        "import sys\nfrom subprocess import Popen\n"
        "Popen([sys.executable, '-c', 'import time; time.sleep(4)'], "
        "start_new_session=True)\n"
    )
    start = time.monotonic()
    runner.execute_code_chunk(code, 'python', capture_plot=False, timeout=1)
    elapsed = time.monotonic() - start
    assert elapsed < 2.5, f'1-second budget blocked for {elapsed:.2f}s'


@pytest.mark.skipif(sys.platform != 'linux' or not shutil.which('node'), reason='Linux with Node.js')
def test_reproduce_bug_002():
    result = runner.execute_code_chunk('console.log(42)', 'js', capture_plot=False)
    assert result['ok'], result['stderr']
    assert result['stdout'] == '42'


def test_reproduce_bug_003():
    sql = "SELECT 'abc\\'; SELECT 2;"
    assert len(runner._split_sql_statements(sql)) == 2
    result = runner.execute_code_chunk(sql, 'sql')
    assert result['ok'], result
    assert 'abc\\' in result['stdout'] and '2' in result['stdout']


@pytest.mark.parametrize('race', ['regular', 'symlink'])
def test_reproduce_bug_004(tmp_path, monkeypatch, race):
    target = tmp_path / 'out.tex'
    victim = tmp_path / 'original.tex'
    victim.write_text('DO NOT CHANGE', encoding='utf-8')
    if race == 'symlink':
        probe = tmp_path / 'probe'
        try:
            probe.symlink_to(victim)
            probe.unlink()
        except OSError:
            pytest.skip('Symlink creation unavailable')

    def render(_content, **_kwargs):
        # A different client creates the destination after validation.
        if race == 'symlink':
            target.symlink_to(victim)
        else:
            target.write_text('CONCURRENT DOCUMENT', encoding='utf-8')
        return 'NEW EXPORT'

    module = SimpleNamespace(markdown_to_latex=render)
    monkeypatch.setattr(mcp, 'texmd', module)
    monkeypatch.setitem(mcp.OPTIONAL_MODULES, 'texmd', module)
    result = mcp.handle_tool_call('readmd_export_document', {
        'markdown_content': '# Test', 'output_path': str(target),
        'output_format': 'tex', 'confirm': True, 'overwrite': False,
    })
    assert victim.read_text(encoding='utf-8') == 'DO NOT CHANGE'
    if race == 'regular':
        assert target.read_text(encoding='utf-8') == 'CONCURRENT DOCUMENT'
    assert result.get('isError') is True


def test_reproduce_bug_005(monkeypatch):
    request = {'jsonrpc': '2.0', 'id': 37, 'method': 'tools/call', 'params': None}
    output = io.StringIO()
    monkeypatch.setattr(mcp.sys, 'stdin', io.StringIO(json.dumps(request) + '\n'))
    monkeypatch.setattr(mcp.sys, 'stdout', output)
    mcp.run_stdio_server()
    response = json.loads(output.getvalue())
    assert response['id'] == 37
    assert response['error']['code'] == -32602


def test_reproduce_bug_007(tmp_path, monkeypatch):
    bridge = HermesPetBridge(str(tmp_path))
    bridge.command_path.parent.mkdir(parents=True)
    first = {'command': {'type': 'open-menu'}}
    second = {'command': {'type': 'drop', 'paths': ['C:\\my notes\\报告.md']}}
    bridge.command_path.write_text(json.dumps(first), encoding='utf-8')
    original_loads = json.loads
    injected = False

    def receive_then_new_command(raw, *args, **kwargs):
        nonlocal injected
        value = original_loads(raw, *args, **kwargs)
        if not injected:
            injected = True
            bridge.command_path.write_text(json.dumps(second), encoding='utf-8')
        return value

    monkeypatch.setattr(json, 'loads', receive_then_new_command)
    assert bridge.take_command() == first['command']
    assert bridge.take_command() == second['command']


def test_reproduce_bug_008():
    blocks = parser.parse('| A | B |\n| :--- | ---: |\n| x\\|y | z |')
    table = blocks[0]
    assert len(table['rows'][0]) == 2
    assert [parser.inline_text(c) for c in table['rows'][0]] == ['x|y', 'z']
    assert table['aligns'] == ['left', 'right']


def test_reproduce_bug_009():
    blocks = parser.parse('```text\na\n```not-a-close\nb\n```')
    assert blocks == [{'type': 'code', 'lang': 'text',
                       'content': 'a\n```not-a-close\nb'}]


@pytest.mark.parametrize('newline', ['\n', '\r\n'])
def test_reproduce_bug_010(newline):
    blocks = parser.parse('$$x +' + newline + 'y$$')
    assert blocks == [{'type': 'math', 'display': True, 'latex': 'x +\ny'}]


def test_reproduce_bug_011():
    # A 1.2 KB document must not crash the export parser.
    blocks = parser.parse('>' * 1200 + ' KEEP_ME')
    # A bounded, literal fallback is acceptable; losing content is not.
    pending = list(blocks)
    found = False
    while pending:
        block = pending.pop()
        pending.extend(block.get('blocks', []))
        if block.get('type') == 'paragraph':
            found |= 'KEEP_ME' in parser.inline_text(block['text'])
    assert found
