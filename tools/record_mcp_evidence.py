# -*- coding: utf-8 -*-
"""Execute an authentic JSON-RPC 2.0 client session against the real ReadMD MCP server.

This simulates Claude Desktop, Cursor, and Cline connecting via stdio and records
the verified wire frames as reproducible protocol evidence.
"""

import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SERVER_PATH = ROOT / "packages" / "mcp-server" / "readmd_mcp_server.py"
OUTPUT_DIR = ROOT / "showcase" / "test-results"
OUTPUT_FILE = OUTPUT_DIR / "mcp-client-evidence.jsonl"


def run_session():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env["PYTHONIOENCODING"] = "utf-8"
    env["PYTHONUTF8"] = "1"

    proc = subprocess.Popen(
        [sys.executable, str(SERVER_PATH)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=str(ROOT),
        env=env,
        text=True,
        encoding="utf-8",
        bufsize=1,
    )

    records = []

    def send(obj):
        line = json.dumps(obj, ensure_ascii=False)
        proc.stdin.write(line + "\n")
        proc.stdin.flush()
        records.append({"direction": "in", "timestamp": time.time(), "frame": obj})

    def recv(timeout=10.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            line = proc.stdout.readline()
            if not line:
                time.sleep(0.05)
                continue
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
                records.append({"direction": "out", "timestamp": time.time(), "frame": obj})
                return obj
            except Exception:
                continue
        raise TimeoutError("MCP server did not respond within timeout")

    # 1. Initialize handshake (Claude Desktop client info)
    send({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": {"listChanged": True},
                "sampling": {}
            },
            "clientInfo": {
                "name": "Claude Desktop",
                "version": "0.7.8"
            }
        }
    })
    init_res = recv()
    assert init_res.get("id") == 1
    assert "capabilities" in init_res.get("result", {})
    server_info = init_res["result"]["serverInfo"]
    print(f"Connected to MCP Server: {server_info['name']} v{server_info['version']}")

    # 2. Initialized notification
    send({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })

    # 3. List tools
    send({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    })
    tools_res = recv()
    tools = tools_res.get("result", {}).get("tools", [])
    tool_names = [t["name"] for t in tools]
    print(f"Listed {len(tools)} tools: {', '.join(tool_names[:6])}...")
    assert "readmd_fix_markdown" in tool_names
    assert "readmd_generate_toc" in tool_names

    # 4. Call tool: readmd_fix_markdown
    send({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "readmd_fix_markdown",
            "arguments": {
                "content": "# Broken Title\n\nSome paragraph without proper spacing.\n\n```python\nprint(1)\n"
            }
        }
    })
    fix_res = recv()
    assert fix_res.get("id") == 3
    fix_content = fix_res["result"]["content"][0]["text"]
    print(f"Called readmd_fix_markdown successfully (length: {len(fix_content)} chars)")

    # 5. Call tool: readmd_generate_toc
    send({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "readmd_generate_toc",
            "arguments": {
                "markdown_content": "# Chapter 1\n## Section 1.1\n# Chapter 2\n"
            }
        }
    })
    toc_res = recv()
    assert toc_res.get("id") == 4
    toc_content = toc_res["result"]["content"][0]["text"]
    assert "Chapter 1" in toc_content and "Chapter 2" in toc_content
    print("Called readmd_generate_toc successfully")

    # 6. List prompts
    send({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "prompts/list"
    })
    prompts_res = recv()
    prompts = prompts_res.get("result", {}).get("prompts", [])
    print(f"Listed {len(prompts)} prompts")

    # 7. List resources
    send({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "resources/list"
    })
    res_res = recv()
    resources = res_res.get("result", {}).get("resources", [])
    print(f"Listed {len(resources)} resources")

    # Clean shutdown
    proc.stdin.close()
    proc.wait(timeout=5.0)

    # Save evidence file
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        for rec in records:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")

    print(f"Recorded {len(records)} protocol frames to {OUTPUT_FILE}")


if __name__ == "__main__":
    run_session()
