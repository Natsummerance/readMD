#!/usr/bin/env python
"""Endpoint parity gate: every HTTP endpoint the client calls must be served by
the Rust kernel with the same route shape the Python authority serves.

Run from anywhere:  python rust/tools/endpoint_parity_gate.py [--json out.json] [--verbose]

Exit codes: 0 = no gaps, 1 = gaps found, 2 = parser could not read a source set.

Design rules that keep this gate honest
---------------------------------------
* Only *strong* evidence counts as serving a route. For Python that is a real
  dispatch branch (`path == '/api/save':` / `path.startswith('/api/pet/')`). For
  Rust that is a ROUTES table entry, a `match` arm, or a `.starts_with(...)`
  prefix test. Bare path literals anywhere else (doc comments, tests, client
  probe URLs such as Ollama's `/api/tags`) are recorded as *weak* evidence and
  never close a gap: a name appearing in a comment is not an implementation.
* Why the parsers accept several idioms: readmd.py dispatches with single-quoted
  literals while the kernel uses a static tuple table whose handlers are often
  path-qualified (`pet::h_pet_list`), and the client uses template strings. A
  parser keyed to one quoting style or one handler shape reports phantom gaps.
* A route that exists but answers "not implemented" is a gap, so each table
  entry's handler body is scanned for the unimplemented markers.
"""

import json
import os
import re
import sys

# Endpoints that are kernel plumbing rather than replicated Python behaviour.
KERNEL_BRIDGES = {"/api/ping", "/api/kernel/status"}

UNIMPLEMENTED_MARKERS = (
    "not_implemented",
    "unimplemented",
    "NotImplemented",
    "NOT_IMPLEMENTED",
    "not yet implemented",
    "501,",
)


def repo_root():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.dirname(os.path.dirname(here))


def read(path):
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        return f.read()


def py_files(root):
    out = [os.path.join(root, "readmd.py")]
    mods = os.path.join(root, "src", "readmd_modules")
    if os.path.isdir(mods):
        for name in sorted(os.listdir(mods)):
            if name.endswith(".py"):
                out.append(os.path.join(mods, name))
    return [p for p in out if os.path.isfile(p)]


def js_files(root):
    out = []
    jsdir = os.path.join(root, "assets", "js")
    for dirpath, _dirnames, filenames in os.walk(jsdir):
        for name in sorted(filenames):
            if name.endswith(".js"):
                out.append(os.path.join(dirpath, name))
    for name in ("index.html", "readmd.boot.js"):
        p = os.path.join(root, "assets", name)
        if os.path.isfile(p):
            out.append(p)
    return out


LITERAL = re.compile(r"""['"`](/[A-Za-z0-9][A-Za-z0-9_\-/.]*)""")
PY_EXACT = re.compile(r"""path\s*(?:==|!=|\.rstrip\(\s*['"]/['"]\s*\)\s*==)\s*['"](/api/[A-Za-z0-9_\-/]*)['"]""")
PY_PREFIX = re.compile(r"""path[A-Za-z_.]*\s*\.startswith\(\s*['"](/api/[A-Za-z0-9_\-/.]*)['"]""")
RUST_TABLE = re.compile(r"""^\s*\(\s*"(/[A-Za-z0-9_\-/]*)"\s*,\s*([\w:]*\bh_\w+)""", re.M)
RUST_ARM = re.compile(r"""^\s*"(/[A-Za-z0-9_\-/.]*)"\s*=>""", re.M)
RUST_STARTS = re.compile(r"""\.starts_with\(\s*"(/[A-Za-z0-9_\-/.]*)"\s*\)""")


def norm(path):
    """Strip query strings, template holes and trailing separators."""
    path = path.split("?")[0].split("#")[0].split("$")[0]
    return path.rstrip("/") or "/"


def candidates(path):
    """Full path plus shorter prefixes: a trailing segment is usually an id or
    slug, so `/api/pet/mochi` must match a `path.startswith('/api/pet/')`
    authority just as it matches the kernel's prefix dispatch."""
    path = norm(path)
    out = [path]
    parts = [s for s in path.split("/") if s]
    while len(parts) > 1:
        parts.pop()
        out.append("/" + "/".join(parts))
    return out


def matches_prefix(path, prefixes):
    for p in prefixes:
        if p.endswith("/") and (norm(path) + "/").startswith(p):
            return p
    return None


def parse_python(root):
    exact, prefix, weak = set(), set(), set()
    for p in py_files(root):
        text = read(p)
        exact.update(PY_EXACT.findall(text))
        prefix.update(PY_PREFIX.findall(text))
        weak.update(LITERAL.findall(text))
    return {"exact": exact, "prefix": prefix, "weak": weak}


def parse_rust(root):
    server = os.path.join(root, "rust", "readmd-kernel", "src", "server.rs")
    if not os.path.isfile(server):
        return None
    text = read(server)
    table = dict(RUST_TABLE.findall(text))
    arms = set(RUST_ARM.findall(text))
    starts = set(RUST_STARTS.findall(text))
    weak = set(LITERAL.findall(text))
    return {"table": table, "arms": arms, "prefix": starts, "weak": weak, "text": text}


def parse_js(root):
    called = {}
    for p in js_files(root):
        for m in LITERAL.findall(read(p)):
            if not m.startswith("/api/"):
                continue
            n = norm(m)
            if len([s for s in n.split("/") if s]) < 2:
                continue
            called.setdefault(n, set()).add(os.path.relpath(p, root).replace("\\", "/"))
    return {k: sorted(v) for k, v in called.items()}


def python_authority(path, py):
    for c in candidates(path):
        if c in py["exact"]:
            return "exact"
    hit = matches_prefix(path, py["prefix"])
    if hit:
        return "prefix:" + hit
    for c in candidates(path):
        if c in py["exact"]:
            return "exact"
    return None


def rust_served(path, rs):
    for c in candidates(path):
        if c in rs["table"]:
            return "table:" + rs["table"][c]
        if c in rs["arms"]:
            return "match-arm"
    hit = matches_prefix(path, rs["prefix"])
    if hit:
        return "starts_with:" + hit
    for c in candidates(path):
        if c in rs["table"]:
            return "table:" + rs["table"][c]
    return None


def handler_body(text, handler):
    name = handler.split("::")[-1]
    m = re.search(r"\bfn\s+%s\s*\(" % re.escape(name), text)
    if not m:
        return ""
    nxt = re.search(r"\n(?:pub\s+)?(?:async\s+)?fn\s+\w", text[m.end():])
    return text[m.start(): m.end() + (nxt.start() if nxt else 4000)]


def implemented(path, rs):
    """False when the strong evidence is a table entry whose handler admits it
    does nothing yet."""
    served = rust_served(path, rs)
    if not served or not served.startswith("table:"):
        return True, served
    body = handler_body(rs["text"], served.split(":", 1)[1])
    for marker in UNIMPLEMENTED_MARKERS:
        if marker in body:
            return False, served + " (marker %r)" % marker
    return True, served


def main(argv):
    as_json = argv[argv.index("--json") + 1] if "--json" in argv else None
    verbose = "--verbose" in argv
    root = repo_root()

    py = parse_python(root)
    rs = parse_rust(root)
    if rs is None:
        sys.stderr.write("FATAL: rust/readmd-kernel/src/server.rs missing\n")
        return 2
    js = parse_js(root)

    gaps, stubs, name_only, orphans = [], [], [], []
    for path in sorted(js):
        auth = python_authority(path, py)
        ok, served = implemented(path, rs)
        if auth and not served:
            gaps.append({"endpoint": path, "python": auth, "callers": js[path]})
        elif auth and served and not ok:
            stubs.append({"endpoint": path, "python": auth, "rust": served, "callers": js[path]})
        elif not auth and served:
            name_only.append({"endpoint": path, "rust": served})
        elif not auth and not served:
            orphans.append({"endpoint": path, "callers": js[path]})

    python_routes = set(py["exact"]) | set(py["prefix"])
    unwired_python = sorted(
        r for r in python_routes
        if r.startswith("/api/") and not rust_served(r.rstrip("/") or "/", rs)
        and r not in KERNEL_BRIDGES
    )
    kernel_extra = sorted(
        r for r in rs["table"]
        if r.startswith("/api/") and r not in py["weak"] and r not in KERNEL_BRIDGES
    )

    print("python strong: exact=%d prefix=%d | rust strong: table=%d arms=%d prefix=%d | client endpoints=%d"
          % (len(py["exact"]), len(py["prefix"]), len(rs["table"]), len(rs["arms"]), len(rs["prefix"]), len(js)))
    sections = [
        ("consumer endpoints served by Python but NOT by Rust (must be 0)", gaps, "GAP"),
        ("consumer endpoints whose Rust handler is an explicit stub (must be 0)", stubs, "STUB"),
        ("Python routes with no Rust route at all", unwired_python, "PY-ONLY"),
        ("Rust routes that no Python source mentions", kernel_extra, "KERNEL-ONLY"),
        ("client endpoints with neither authority (informational)", orphans, "ORPHAN"),
    ]
    for title, items, tag in sections:
        print()
        print("== %s ==" % title)
        if not items:
            print("  none")
        for it in items:
            if isinstance(it, str):
                print("  %-11s %s" % (tag, it))
            else:
                extra = it.get("rust") or it.get("python") or ""
                callers = ",".join(it.get("callers", [])) if verbose else ""
                print("  %-11s %-34s %-28s %s" % (tag, it["endpoint"], extra, callers))

    if as_json:
        with open(as_json, "w", encoding="utf-8") as f:
            json.dump({"gaps": gaps, "stubs": stubs, "python_routes_unwired": unwired_python,
                       "kernel_only": kernel_extra, "orphans": orphans}, f, ensure_ascii=False, indent=2)
    return 1 if (gaps or stubs) else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
