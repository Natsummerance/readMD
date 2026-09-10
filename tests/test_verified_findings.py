"""Run from the audited ReadMD checkout; no production files are modified."""
import io
import json
import os
from pathlib import Path
import signal
import sys

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "packages" / "mcp-server"))

from src.readmd_modules import code_chunk_runner as runner
from src.readmd_modules import mdexport
from src.readmd_modules.mdexport import html_render, parser
from src.readmd_modules.import_processor import slice_code_lines
import readmd_mcp_server as mcp


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX process-group branch")
def test_reproduce_bug_001():
    import psutil
    child_pid = None
    try:
        result = runner.execute_python_chunk(
            "import subprocess, sys\n"
            "p = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'], "
            "stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\n"
            "print(p.pid, flush=True)\n",
            capture_plot=False, timeout=2,
        )
        assert result["ok"], result
        child_pid = int(result["stdout"])
        def running():
            try:
                return psutil.Process(child_pid).status() != psutil.STATUS_ZOMBIE
            except psutil.NoSuchProcess:
                return False
        assert not running(), "Successful execution left a running descendant"
    finally:
        if child_pid:
            try:
                os.kill(child_pid, signal.SIGKILL)
            except ProcessLookupError:
                pass


@pytest.mark.skipif(sys.platform == "win32", reason="POSIX byte stream branch")
def test_reproduce_bug_002(monkeypatch):
    real_read = runner.os.read
    # Pipe reads may legally split a UTF-8 character at any byte boundary.
    monkeypatch.setattr(runner.os, "read", lambda fd, size: real_read(fd, min(size, 1)))
    result = runner.execute_python_chunk("print('中文😀')", capture_plot=False)
    assert result["ok"], result
    assert result["stdout"] == "中文😀"


def test_reproduce_bug_003(tmp_path, monkeypatch):
    target = tmp_path / "report.html"
    sentinel = "created by another request after validation"
    def racing_render(content, output, *args, **kwargs):
        Path(output).write_text("<html>new export</html>", encoding="utf-8")
        target.write_text(sentinel, encoding="utf-8")
    monkeypatch.setattr(html_render, "render", racing_render)
    result = mcp.handle_tool_call("readmd_export_document", {
        "confirm": True, "overwrite": False, "output_format": "html",
        "output_path": str(target), "markdown_content": "# test",
    })
    assert target.read_text(encoding="utf-8") == sentinel
    assert result.get("isError") is True


def test_reproduce_bug_004(tmp_path, monkeypatch):
    def denied(*args, **kwargs):
        raise PermissionError("simulated Windows file lock")
    monkeypatch.setattr(html_render, "render", denied)
    target = tmp_path / "report.html"
    result = mcp.handle_tool_call("readmd_export_document", {
        "confirm": True, "output_format": "html", "output_path": str(target),
        "markdown_content": "# test",
    })
    assert not target.exists()
    assert result.get("isError") is True, result
    payload = json.loads(result["content"][0]["text"])
    assert payload["ok"] is False
    assert payload["stage"] == "render"


def test_reproduce_bug_005(monkeypatch):
    # Hold all workers before execution. Simulates tools blocked on I/O,
    # without starting threads or exhausting the test machine.
    started = []
    replies = []
    class HeldThread:
        def __init__(self, target, **kwargs):
            self.target = target
        def start(self):
            started.append(self)
    requests = [json.dumps({"jsonrpc": "2.0", "id": i, "method": "tools/call",
                 "params": {"name": "readmd_fix_markdown", "arguments": {"content": "x"}}})
                for i in range(40)]
    monkeypatch.setattr(mcp.sys, "stdin", io.StringIO("\n".join(requests) + "\n"))
    monkeypatch.setattr(mcp.threading, "Thread", HeldThread)
    monkeypatch.setattr(mcp, "_write_message", replies.append)
    monkeypatch.setattr(mcp, "_CANCEL_EVENTS", {})
    # New policy: at most 8 concurrent tools; overflow is immediately rejected.
    monkeypatch.setattr(mcp, "MAX_CONCURRENT_TOOLS", 8, raising=False)
    mcp.run_stdio_server()
    assert len(started) <= 8, f"{len(started)} workers admitted without backpressure"
    rejected = [r for r in replies if r.get("error", {}).get("code") == -32001]
    assert len(rejected) == 32


def test_reproduce_bug_006(tmp_path):
    source = "before\n```\n# this is code, not a document heading\n```\nafter"
    wrapped = slice_code_lines(source, lang="text")
    blocks = parser.parse(wrapped)
    assert len(blocks) == 1, blocks
    assert blocks[0]["type"] == "code"
    assert blocks[0]["content"] == source


def test_reproduce_bug_007(tmp_path):
    text = "中文测试报告"
    path = tmp_path / "gbk.txt"
    path.write_bytes(text.encode("gbk"))
    result = mcp.handle_tool_call("readmd_convert_to_markdown", {"file_path": str(path)})
    assert not result.get("isError"), result
    rendered = result["content"][0]["text"]
    assert text in rendered
    assert "\ufffd" not in rendered
