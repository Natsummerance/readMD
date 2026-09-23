#!/usr/bin/env python
"""Runtime half of the endpoint parity gate.

The static gate (endpoint_parity_gate.py) proves a Rust route exists for every
endpoint the client calls. That is still a proxy: a registered route can answer
404 or "not implemented" at runtime. This probe starts the real kernel against a
throwaway workspace and asks it, over HTTP, for every consumer endpoint.

Verdicts per endpoint:
  served  any reply that is neither 404 nor an unimplemented answer
  GAP     404  (route missing from dispatch)
  STUB    501, or a body naming not_implemented / unimplemented
  TRANSPORT connection failed or timed out

Requests are GET-only. A POST-only handler answers 405 or a validation error,
never 404, so the non-404 assertion holds without mutating anything.

Exit 0 only when GAP == 0 and STUB == 0.
Run: python rust/tools/endpoint_live_probe.py
"""

import os
import re
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
KERNEL = os.path.join(REPO, "rust", "target", "debug", "readmd.exe")
ASSETS = os.path.join(REPO, "assets")
OUT = os.path.join(REPO, "scratch", "rust_parity", "endpoint_gate", "live.txt")

LITERAL = re.compile(r"""['"`](/api/[A-Za-z0-9_\-/.]*)['"`]""")
UNIMPLEMENTED = ("not_implemented", "unimplemented", "not yet implemented")


def scan(path, found):
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            text = fh.read()
    except OSError:
        return
    for m in LITERAL.finditer(text):
        lit = m.group(1)
        if "{" in lit or lit.count("/") < 2:
            continue
        found.add(lit.rstrip("/") or "/api")


def consumer_endpoints():
    """Every /api/... literal the client can actually request."""
    found = set()
    for dirpath, _dirs, names in os.walk(os.path.join(ASSETS, "js")):
        for n in names:
            if n.endswith(".js"):
                scan(os.path.join(dirpath, n), found)
    for n in sorted(os.listdir(ASSETS)):
        if n.endswith(".html"):
            scan(os.path.join(ASSETS, n), found)
    scan(os.path.join(ASSETS, "readmd.boot.js"), found)
    return found


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def http(port, path, timeout=10):
    url = "http://127.0.0.1:%d%s" % (port, path)
    try:
        with urllib.request.urlopen(urllib.request.Request(url), timeout=timeout) as r:
            return r.status, r.read(4096).decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        try:
            body = e.read(4096).decode("utf-8", "replace")
        except Exception:
            body = ""
        return e.code, body
    except Exception as e:
        return None, "%s: %s" % (type(e).__name__, e)


def classify(status, body):
    if status is None:
        return "TRANSPORT", body[:120]
    if status == 404:
        return "GAP", "404 %s" % body.strip()[:120]
    if status == 501 or any(k in body.lower() for k in UNIMPLEMENTED):
        return "STUB", "%s %s" % (status, body.strip()[:120])
    return "served", "%s %s" % (status, body.strip()[:60])


def main():
    if not os.path.exists(KERNEL):
        print("kernel binary missing: %s\nbuild it first: cargo build --offline -p readmd-kernel" % KERNEL)
        return 2
    endpoints = sorted(consumer_endpoints())
    ws = tempfile.mkdtemp(prefix="readmd_probe_")
    with open(os.path.join(ws, "sample.md"), "w", encoding="utf-8") as fh:
        fh.write("# Probe\n\n- [link](./a.md)\n")
    port = free_port()
    proc = subprocess.Popen(
        [KERNEL, "--browser", "--host", "127.0.0.1", "--port", str(port),
         "--workspace", ws, "--assets", ASSETS, "--no-window"],
        cwd=REPO, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    lines = ["kernel=%s port=%d endpoints=%d" % (os.path.basename(KERNEL), port, len(endpoints))]
    buckets = {"served": [], "GAP": [], "STUB": [], "TRANSPORT": []}
    rc = 1
    try:
        ready = None
        for _ in range(60):
            ready, _b = http(port, "/api/ping", timeout=2)
            if ready == 200:
                break
            time.sleep(0.5)
        if ready != 200:
            lines.append("FATAL /api/ping never answered 200 (last=%s)" % ready)
            return 2

        for ep in endpoints:
            probe = ep + "/" if ep in ("/api/recent", "/api/upstream-sources") else ep
            status, body = http(port, probe)
            kind, detail = classify(status, body)
            buckets[kind].append((ep, detail))

        ctl_status, ctl_body = http(port, "/api/no/such/endpoint-at-all")
        lines.append("control unknown-route reply: %s %s" % (ctl_status, ctl_body.strip()[:80]))
        lines.append("served=%d GAP=%d STUB=%d TRANSPORT=%d"
                     % (len(buckets["served"]), len(buckets["GAP"]),
                        len(buckets["STUB"]), len(buckets["TRANSPORT"])))
        for kind in ("GAP", "STUB", "TRANSPORT"):
            for ep, detail in buckets[kind]:
                lines.append("  %s %-34s %s" % (kind, ep, detail))
        rc = 1 if buckets["GAP"] or buckets["STUB"] else 0
        return rc
    finally:
        lines.append("PROBE_RC=%d" % rc)
        os.makedirs(os.path.dirname(OUT), exist_ok=True)
        with open(OUT, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
        print("\n".join(lines))
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except Exception:
            proc.kill()


if __name__ == "__main__":
    sys.exit(main())
