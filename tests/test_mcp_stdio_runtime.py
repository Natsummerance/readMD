# -*- coding: utf-8 -*-
"""ReadMD MCP Server stdio 运行时测试：真实子进程协议回路、流式进度、通知取消与动态 Skill 注册。"""

import json
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time
import types
import unittest
from unittest.mock import patch
from contextlib import contextmanager

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MCP_DIR = os.path.join(ROOT, 'packages', 'mcp-server')
SERVER_PATH = os.path.join(MCP_DIR, 'readmd_mcp_server.py')
if MCP_DIR not in sys.path:
    sys.path.insert(0, MCP_DIR)

import readmd_mcp_server

EXPECTED_TOOL_NAMES = [
    "readmd_fix_markdown", "readmd_convert_to_markdown", "readmd_web_to_markdown",
    "readmd_ocr_to_markdown", "readmd_export_document", "readmd_latex_to_md",
    "readmd_md_to_latex", "readmd_parse_bibtex", "readmd_latex_to_omml",
    "readmd_ai_assistant", "readmd_ai_providers", "readmd_ai_chat",
    "readmd_process_imports", "readmd_generate_toc", "readmd_export_presentation",
    "readmd_export_epub", "readmd_run_code_chunk",
]


def _wait_for(predicate, timeout=15.0, message="condition not met in time"):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if predicate():
            return
        time.sleep(0.01)
    raise AssertionError(message)


class _FakeStdin:
    """Blocks readline() on a queue so run_stdio_server behaves like a real stdio loop."""

    def __init__(self):
        self._lines = queue.Queue()

    def push(self, payload):
        self._lines.put(json.dumps(payload, ensure_ascii=False) + "\n")

    def push_raw(self, line):
        self._lines.put(line if line.endswith("\n") else line + "\n")

    def readline(self):
        item = self._lines.get(timeout=60)
        return item

    def close(self):
        self._lines.put("")


class _FakeStdout:
    def __init__(self):
        self._lock = threading.Lock()
        self._buffer = ""
        self.messages = []

    def write(self, text):
        with self._lock:
            self._buffer += text
            while "\n" in self._buffer:
                line, self._buffer = self._buffer.split("\n", 1)
                if line.strip():
                    self.messages.append(json.loads(line))
        return len(text)

    def flush(self):
        return None

    def of_id(self, req_id):
        with self._lock:
            return [m for m in self.messages if m.get("id") == req_id]


class _FakeAI:
    """Minimal stand-in for src.readmd_modules.ai consumed by readmd_ai_chat."""

    def __init__(self, gen_factory):
        self.gen_factory = gen_factory
        self.calls = []

    def find_provider(self, provider):
        return {"id": provider, "name": provider}

    @staticmethod
    def _is_local_provider(_provider):
        return False

    def chat(self, payload):
        self.calls.append(payload)
        return self.gen_factory(payload)


@contextmanager
def _fake_runtime(gen_factory):
    """Run run_stdio_server in a thread with fake stdio and a fake AI module."""
    fake_stdin, fake_stdout = _FakeStdin(), _FakeStdout()
    fake_ai = _FakeAI(gen_factory)
    fake_module = types.SimpleNamespace(get=lambda name: fake_ai if name == "ai" else None)
    saved = (sys.stdin, sys.stdout, readmd_mcp_server.RM)
    sys.stdin, sys.stdout, readmd_mcp_server.RM = fake_stdin, fake_stdout, fake_module
    worker = threading.Thread(target=readmd_mcp_server.run_stdio_server, daemon=True,
                              name="mcp-stdio-test")
    worker.start()
    try:
        yield fake_stdin, fake_stdout, fake_ai
    finally:
        fake_stdin.close()
        worker.join(timeout=15)
        sys.stdin, sys.stdout, readmd_mcp_server.RM = saved


class TestStdioSubprocessProtocol(unittest.TestCase):
    """Spawn the real server as a subprocess and speak newline-delimited JSON-RPC."""

    def test_full_protocol_roundtrip(self):
        env = dict(os.environ, PYTHONIOENCODING="utf-8")
        proc = subprocess.Popen(
            [sys.executable, SERVER_PATH], cwd=ROOT,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            env=env)
        lines_queue = queue.Queue()
        stderr_lines = []

        def _pump(stream, sink):
            for raw in iter(stream.readline, b""):
                sink.put(raw.decode("utf-8", "replace"))
            sink.put(None)

        threading.Thread(target=_pump, args=(proc.stdout, lines_queue), daemon=True).start()
        threading.Thread(target=lambda: [stderr_lines.append(l) for l in
                                         iter(proc.stderr.readline, b"")], daemon=True).start()

        def rpc(payload, timeout=120):
            proc.stdin.write((json.dumps(payload) + "\n").encode("utf-8"))
            proc.stdin.flush()
            deadline = time.time() + timeout
            while time.time() < deadline:
                try:
                    raw = lines_queue.get(timeout=0.25)
                except queue.Empty:
                    if proc.poll() is not None:
                        break
                    continue
                if raw is None:
                    break
                msg = json.loads(raw)
                if msg.get("id") == payload.get("id"):
                    return msg
            raise AssertionError("no response for id=%r" % payload.get("id"))

        try:
            init = rpc({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
            self.assertEqual(init["result"]["protocolVersion"], "2024-11-05")
            self.assertEqual(init["result"]["serverInfo"]["name"], "readmd-mcp-server")
            for cap in ("tools", "resources", "prompts"):
                self.assertIn(cap, init["result"]["capabilities"])

            tools = rpc({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
            names = [t["name"] for t in tools["result"]["tools"]]
            self.assertEqual(len(names), 17)
            for expected in EXPECTED_TOOL_NAMES:
                self.assertIn(expected, names)

            call = rpc({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
                "name": "readmd_fix_markdown",
                "arguments": {"content": "测试公式 $E=mc^2$ 与表格"}}})
            payload = json.loads(call["result"]["content"][0]["text"])
            self.assertTrue(payload["ok"])
            self.assertIn("repaired_content", payload)

            resources = rpc({"jsonrpc": "2.0", "id": 4, "method": "resources/list"})
            uris = {r["uri"] for r in resources["result"]["resources"]}
            self.assertIn("readmd://skills/readmd-summary", uris)
            self.assertIn("readmd://sessions", uris)
            self.assertIn("readmd://providers", uris)

            prompts = rpc({"jsonrpc": "2.0", "id": 5, "method": "prompts/list"})
            prompt_names = [p["name"] for p in prompts["result"]["prompts"]]
            self.assertIn("readmd-summary", prompt_names)

            missing = rpc({"jsonrpc": "2.0", "id": 6, "method": "no/such/method"})
            self.assertEqual(missing["error"]["code"], -32601)

            # A notification (no id) is never answered; the next request still works.
            proc.stdin.write((json.dumps({
                "jsonrpc": "2.0", "method": "readmd/test-notify"}) + "\n").encode("utf-8"))
            proc.stdin.flush()
            after = rpc({"jsonrpc": "2.0", "id": 7, "method": "tools/list"})
            self.assertEqual(len(after["result"]["tools"]), 17)

            deadline = time.time() + 2.0
            echoed = False
            while time.time() < deadline:
                try:
                    raw = lines_queue.get(timeout=0.1)
                except queue.Empty:
                    continue
                if raw is None:
                    break
                if "readmd/test-notify" in raw:
                    echoed = True
            self.assertFalse(echoed, "notification must not be answered")
        finally:
            try:
                proc.stdin.close()
            except Exception:
                pass
            try:
                proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                proc.kill()
                raise
        self.assertEqual(proc.returncode, 0)


class TestStdioLoopStreaming(unittest.TestCase):
    """In-process stdio loop: progress notifications flow while tools/call streams."""

    def test_progress_notifications_and_final_result(self):
        chunks = ["第一段。", "第二段。", "第三段。"]

        def gen_factory(_payload):
            yield from chunks
            yield {"usage": {"total_tokens": 9}}

        with _fake_runtime(gen_factory) as (stdin, stdout, fake_ai):
            stdin.push({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
            _wait_for(lambda: stdout.of_id(1), message="initialize not answered")
            stdin.push({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
                "name": "readmd_ai_chat",
                "arguments": {"provider": "custom:test", "credential_id": "cred:abc12345",
                              "model": "mock", "skill_id": "readmd-summary",
                              "markdown_content": "# 文档", "stream": True},
                "_meta": {"progressToken": 7}}})
            _wait_for(lambda: stdout.of_id(2), message="tools/call not answered")
            response = stdout.of_id(2)[0]
            payload = json.loads(response["result"]["content"][0]["text"])
            self.assertTrue(payload["ok"])
            self.assertEqual(payload["content"], "第一段。第二段。第三段。")
            self.assertEqual(payload["usage"], {"total_tokens": 9})
            self.assertEqual(payload["skill_id"], "readmd-summary")

            progress = [m for m in stdout.messages
                        if m.get("method") == "notifications/progress"]
            self.assertEqual([p["params"]["progressToken"] for p in progress], [7, 7, 7])
            self.assertEqual([p["params"]["progress"] for p in progress], [1, 2, 3])
            self.assertEqual([p["params"]["message"] for p in progress], chunks)
            self.assertEqual(fake_ai.calls[0]["stream"], True)
            self.assertNotIn("api_key", fake_ai.calls[0])

    def test_cancel_notification_stops_stream_early(self):
        full_text = "".join("chunk-%03d" % i for i in range(200))

        def gen_factory(_payload):
            for i in range(200):
                time.sleep(0.005)
                yield "chunk-%03d" % i

        with _fake_runtime(gen_factory) as (stdin, stdout, _fake_ai):
            stdin.push({"jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": {
                "name": "readmd_ai_chat",
                "arguments": {"provider": "custom:test", "credential_id": "cred:abc12345",
                              "model": "mock", "skill_id": "readmd-summary",
                              "markdown_content": "# 文档"},
                "_meta": {"progressToken": 3}}})
            _wait_for(lambda: len([m for m in stdout.messages
                                   if m.get("method") == "notifications/progress"]) >= 3,
                      message="no progress notifications observed")
            stdin.push({"jsonrpc": "2.0", "method": "notifications/cancelled",
                        "params": {"requestId": 11}})
            _wait_for(lambda: stdout.of_id(11), message="cancelled call never answered")
            response = stdout.of_id(11)[0]
            payload = json.loads(response["result"]["content"][0]["text"])
            self.assertFalse(payload["ok"])
            self.assertEqual(payload["error_code"], "ai_cancelled")
            partial = payload["content"]
            self.assertTrue(partial)
            self.assertTrue(full_text.startswith(partial),
                            "partial content must be a prefix of the full stream")
            emitted = len([m for m in stdout.messages
                           if m.get("method") == "notifications/progress"])
            self.assertLess(emitted, 200, "stream must stop early after cancellation")
            _wait_for(lambda: 11 not in readmd_mcp_server._CANCEL_EVENTS,
                      message="cancel event entry was not cleaned up")

    def test_worker_thread_keeps_main_loop_responsive(self):
        def gen_factory(_payload):
            for i in range(40):
                time.sleep(0.01)
                yield "t%d" % i

        with _fake_runtime(gen_factory) as (stdin, stdout, _fake_ai):
            stdin.push({"jsonrpc": "2.0", "id": 21, "method": "tools/call", "params": {
                "name": "readmd_ai_chat",
                "arguments": {"provider": "custom:test", "credential_id": "cred:abc12345",
                              "model": "mock", "skill_id": "readmd-summary",
                              "markdown_content": "# 文档"},
                "_meta": {"progressToken": 5}}})
            # While the worker is still streaming, the main loop must answer tools/list.
            stdin.push({"jsonrpc": "2.0", "id": 22, "method": "tools/list"})
            _wait_for(lambda: stdout.of_id(22), message="main loop blocked by tools/call")
            self.assertEqual(len(stdout.of_id(22)[0]["result"]["tools"]), 17)
            _wait_for(lambda: stdout.of_id(21), message="streaming call never completed")
            payload = json.loads(stdout.of_id(21)[0]["result"]["content"][0]["text"])
            self.assertTrue(payload["ok"])
            self.assertEqual(payload["content"], "".join("t%d" % i for i in range(40)))


class TestAICancelUnit(unittest.TestCase):
    """handle_tool_call honors a preset cancel_event without consuming chunks."""

    def test_preset_cancel_returns_ai_cancelled_without_consuming(self):
        consumed = []

        def gen_factory(_payload):
            for i in range(5):
                consumed.append(i)
                yield "chunk-%d" % i

        fake_ai = _FakeAI(gen_factory)
        fake_module = types.SimpleNamespace(get=lambda name: fake_ai if name == "ai" else None)
        cancel_event = threading.Event()
        cancel_event.set()
        with patch.object(readmd_mcp_server, "RM", fake_module):
            res = readmd_mcp_server.handle_tool_call("readmd_ai_chat", {
                "provider": "custom:test", "credential_id": "cred:abc12345",
                "model": "mock", "skill_id": "readmd-summary",
                "markdown_content": "# 文档",
            }, cancel_event=cancel_event)
        self.assertFalse(res.get("isError", False))
        payload = json.loads(res["content"][0]["text"])
        self.assertFalse(payload["ok"])
        self.assertEqual(payload["error_code"], "ai_cancelled")
        self.assertEqual(payload["content"], "")
        self.assertEqual(consumed, [], "cancelled request must not consume any chunk")


class TestDynamicSkillRegistration(unittest.TestCase):
    """Project Skills dropped into <project>/.readmd/skills appear over MCP."""

    def test_project_skill_visible_in_resources_prompts_and_assistant(self):
        with tempfile.TemporaryDirectory() as project_dir:
            skill_dir = os.path.join(project_dir, ".readmd", "skills", "my-draft-skill")
            os.makedirs(skill_dir)
            with open(os.path.join(skill_dir, "SKILL.md"), "w", encoding="utf-8") as handle:
                handle.write("---\n"
                             "name: my-draft-skill\n"
                             "description: 项目草稿 Skill 用于运行时注册验证\n"
                             "---\n"
                             "请总结以下文档：{{document}}\n")
            from src.readmd_core.service import ReadMDCoreService
            saved = readmd_mcp_server._CORE_SERVICE
            readmd_mcp_server._CORE_SERVICE = ReadMDCoreService(project_dir=project_dir)
            try:
                uris = {item["uri"] for item in readmd_mcp_server._all_resources()}
                self.assertIn("readmd://skills/my-draft-skill", uris)
                prompt_names = {p["name"] for p in readmd_mcp_server._prompt_descriptors()}
                self.assertIn("my-draft-skill", prompt_names)
                direct = readmd_mcp_server._read_skill_resource("readmd://skills/my-draft-skill")
                self.assertIn("请总结以下文档", direct["contents"][0]["text"])
                res = readmd_mcp_server.handle_tool_call("readmd_ai_assistant", {
                    "workflow_id": "my-draft-skill",
                    "markdown_content": "# 项目文档\n正文。",
                })
                self.assertFalse(res.get("isError", False))
                payload = json.loads(res["content"][0]["text"])
                self.assertEqual(payload["workflow_id"], "my-draft-skill")
                self.assertIn("请总结以下文档", payload.get("system_prompt", ""))
            finally:
                readmd_mcp_server._CORE_SERVICE = saved


if __name__ == "__main__":
    unittest.main()
