# -*- coding: utf-8 -*-
"""ReadMD 专业图表辅助模块 (Diagrams Helper)。

支持图表体系：
1. Mermaid: 流程图、时序图、甘特图、状态图；
2. WaveDrom & Bitfield: 数字时序逻辑波形图、寄存器位域图；
3. Graphviz (Viz.js / dot): 状态机、网络拓扑有向图；
4. PlantUML: UML 类图、架构图（双通道：Web SVG 代理编码与本地 Java 探测）；
5. Vega & Vega-Lite: 交互式数据驱动可视化图表；
6. TikZ & LaTeX PGFPlots: 科学矢量几何图、费曼图、电路拓扑图 (TikZjax 纯前端 WebAssembly 渲染)。
"""

import base64
import json
import os
import re
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
import zlib
from pathlib import Path
from typing import Dict, List, Optional, Tuple

PLANTUML_CHARS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_"


class DiagramRenderError(RuntimeError):
    """A stable, user-safe diagram rendering failure."""

    def __init__(self, code: str):
        self.code = str(code or "diagram_render_failed")
        super().__init__(self.code)


def plantuml_encode_6bit(b: int) -> str:
    """PlantUML 6-bit 自定义 Base64 编码字符映射。"""
    return PLANTUML_CHARS[b & 0x3F]


def plantuml_encode(text: str) -> str:
    """将 PlantUML 纯文本压缩并转换为官方标准 Web URL 编码。"""
    zlib_data = zlib.compress(text.encode('utf-8'))[2:-4]
    encoded = []
    i = 0
    while i < len(zlib_data):
        b1 = zlib_data[i]
        b2 = zlib_data[i + 1] if i + 1 < len(zlib_data) else 0
        b3 = zlib_data[i + 2] if i + 2 < len(zlib_data) else 0

        c1 = b1 >> 2
        c2 = ((b1 & 0x3) << 4) | (b2 >> 4)
        c3 = ((b2 & 0xF) << 2) | (b3 >> 6)
        c4 = b3 & 0x3F

        encoded.append(plantuml_encode_6bit(c1))
        encoded.append(plantuml_encode_6bit(c2))
        if i + 1 < len(zlib_data):
            encoded.append(plantuml_encode_6bit(c3))
        if i + 2 < len(zlib_data):
            encoded.append(plantuml_encode_6bit(c4))
        i += 3

    return "".join(encoded)


def get_plantuml_svg_url(plantuml_code: str, server_url: str = "https://www.plantuml.com/plantuml") -> str:
    """生成 PlantUML 在线 SVG 渲染 URL。"""
    code = plantuml_code.strip()
    if not code.startswith("@start"):
        code = f"@startuml\n{code}\n@enduml"
    encoded = plantuml_encode(code)
    # Default server format (no prefix) is raw deflate + the PlantUML 6-bit
    # alphabet, which is exactly what plantuml_encode produces.  A "~1" prefix
    # makes plantuml.com answer with its "generated a bad URL" error image.
    return f"{server_url.rstrip('/')}/svg/{encoded}"


def fetch_plantuml_svg(plantuml_code: str, timeout: float = 15.0) -> str:
    """Fetch rendered SVG from the public PlantUML server.

    The WebView CSP forbids loading remote images, so the backend performs the
    online request (honoring system proxies) and returns SVG markup.  Failures
    stay honest: no network means a network error code, not a fake success.
    """
    url = get_plantuml_svg_url(plantuml_code)
    try:
        request = urllib.request.Request(url, headers={'User-Agent': 'ReadMD'})
        with urllib.request.urlopen(request, timeout=max(5.0, min(float(timeout), 30.0))) as response:
            data = response.read(8 * 1024 * 1024 + 1)
    except urllib.error.HTTPError:
        raise DiagramRenderError("diagram_render_failed")
    except (OSError, ValueError):
        raise DiagramRenderError("diagram_network_unavailable")
    if len(data) > 8 * 1024 * 1024:
        raise DiagramRenderError("diagram_render_failed")
    svg = data.decode('utf-8', errors='replace').strip()
    if '<svg' in svg:
        svg = svg[svg.find('<svg'):]
    if not svg.startswith('<svg'):
        raise DiagramRenderError("diagram_network_unavailable")
    # plantuml.com answers 200 with an error image when it cannot decode the
    # encoded source; never present that image as a rendered diagram.
    if 'generated a bad URL' in svg:
        raise DiagramRenderError("diagram_render_failed")
    return svg


def has_local_plantuml() -> bool:
    """探测系统本地是否具备 Java 及 PlantUML 环境。"""
    jar = os.environ.get('PLANTUML_JAR')
    if shutil.which('plantuml'):
        return True
    if not (shutil.which('java') and jar):
        return False
    try:
        return Path(jar).is_file()
    except (OSError, ValueError, TypeError):
        return False


def _plantuml_command() -> Optional[List[str]]:
    """Return a shell-free local PlantUML command, if one is configured."""
    executable = shutil.which('plantuml')
    if executable:
        return [executable, '-tsvg', '-pipe']
    java = shutil.which('java')
    jar = os.environ.get('PLANTUML_JAR')
    if java and jar:
        try:
            path = Path(jar)
            if path.is_file():
                return [java, '-jar', str(path.resolve()), '-tsvg', '-pipe']
        except (OSError, ValueError, TypeError):
            pass
    return None


def render_plantuml_svg(plantuml_code: str, timeout: float = 15.0) -> str:
    """Render PlantUML through an explicitly installed local runtime.

    No shell, network or temporary source file is used.  If Java/PlantUML is
    not installed the caller can choose the documented online URL fallback;
    this function fails closed so an unavailable local runtime is never
    mistaken for an offline success.
    """
    command = _plantuml_command()
    if not command:
        raise DiagramRenderError("diagram_dependency_missing")
    code = str(plantuml_code or '').strip()
    if not code or len(code.encode('utf-8')) > 2 * 1024 * 1024:
        raise DiagramRenderError("diagram_input_too_large")
    if not code.startswith('@start'):
        code = f'@startuml\n{code}\n@enduml'
    try:
        result = subprocess.run(
            command,
            input=code.encode('utf-8'),
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=max(1.0, min(float(timeout), 30.0)),
            cwd=str(Path(__file__).resolve().parents[2]),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        raise DiagramRenderError("diagram_engine_timeout")
    if result.returncode != 0:
        raise DiagramRenderError("diagram_render_failed")
    svg = result.stdout.decode('utf-8', errors='strict').strip()
    if '<svg' in svg:
        svg = svg[svg.find('<svg'):]
    if not svg.startswith('<svg') or len(svg.encode('utf-8')) > 8 * 1024 * 1024:
        raise DiagramRenderError("diagram_render_failed")
    return svg


def _node_runtime(root: Path) -> Optional[str]:
    """Resolve the Node runtime allowed for server-side diagram rendering.

    Frozen builds must be self-contained: a developer's PATH must not turn
    Vega support green on a machine where the installed app cannot run it.
    Development checkouts may use a local Node binary for tests and bundling.
    """
    if getattr(sys, "frozen", False):
        executable = "node.exe" if os.name == "nt" else "node"
        candidates = (
            root / "assets" / "vendor" / "node" / executable,
            root / executable,
        )
        return next((str(path) for path in candidates if path.is_file()), None)
    return shutil.which("node")


def get_diagram_capabilities() -> Dict[str, object]:
    """Return the renderers that are actually available in this installation.

    The reader keeps browser renderers lazy, so this function only checks the
    immutable files that are shipped with the app and the optional local
    processes required by server-side renderers.  It deliberately does not
    perform a network request or execute user-provided diagram source.
    """
    root = Path(__file__).resolve().parents[2]
    vendor = root / "assets" / "vendor" / "diagrams"
    browser_assets = {
        "mermaid": vendor / "mermaid" / "mermaid.min.js",
        "wavedrom": vendor / "wavedrom" / "wavedrom.min.js",
        "bitfield": vendor / "bitfield" / "bitfield.min.js",
        "viz": vendor / "viz" / "viz-standalone.js",
        "tikz": vendor / "tikzjax" / "tikzjax.js",
        "chart": vendor / "chart" / "chart.umd.js",
    }
    capabilities: Dict[str, Dict[str, object]] = {}
    for engine, asset in browser_assets.items():
        capabilities[engine] = {
            "available": asset.is_file(),
            "offline": asset.is_file(),
            "renderer": "browser",
            "requires_network": False,
        }
    # ``chartjs`` and ``chart.js`` are aliases accepted by the Markdown
    # dispatcher.  Keep one canonical capability entry so clients can
    # present a stable status without duplicating asset probes.
    capabilities["chartjs"] = dict(capabilities["chart"])
    capabilities["chart.js"] = dict(capabilities["chart"])

    node = _node_runtime(root)
    vega_assets = (
        vendor / "vega" / "vega.min.js",
        vendor / "vega-lite" / "vega-lite.min.js",
    )
    vega_ready = bool(node and all(path.is_file() for path in vega_assets))
    for engine in ("vega", "vega-lite"):
        capabilities[engine] = {
            "available": vega_ready,
            "offline": vega_ready,
            "renderer": "node",
            "requires_network": False,
            "reason": "" if vega_ready else "diagram_dependency_missing",
        }

    local_plantuml = has_local_plantuml()
    capabilities["plantuml"] = {
        "available": bool(local_plantuml),
        "offline": bool(local_plantuml),
        "renderer": "java" if local_plantuml else "remote",
        "requires_network": not local_plantuml,
        "remote_available": True,
        "reason": "" if local_plantuml else "diagram_dependency_missing",
    }
    capabilities["puml"] = dict(capabilities["plantuml"])
    # WebSequenceDiagrams is intentionally not mapped to PlantUML: the two
    # languages are not interchangeable.  Keep an explicit unavailable entry
    # so a fence gets a safe fallback instead of a misleading online success.
    capabilities["wsd"] = {
        "available": False,
        "offline": False,
        "renderer": "none",
        "requires_network": False,
        "reason": "diagram_engine_unavailable",
    }
    # D2 has no pinned runtime in this release.  Keep the entry so clients can
    # disable it explicitly instead of falling through to an online renderer.
    capabilities["d2"] = {
        "available": False,
        "offline": False,
        "renderer": "none",
        "requires_network": False,
        "reason": "diagram_engine_unavailable",
    }
    all_offline = all(bool(e.get("offline")) for e in capabilities.values() if e.get("available"))
    return {"schema_version": 1, "offline": all_offline, "engines": capabilities}


def render_vega_svg(spec_text: str, language: str = "vega-lite", timeout: float = 12.0) -> str:
    """Render a Vega/Vega-Lite specification through the bundled Node runtime.

    Vega generates expression functions at runtime.  The desktop WebView CSP
    intentionally disallows ``unsafe-eval``, so loading the browser bundle in
    the page cannot be a reliable offline renderer.  The vendored, pinned
    bundles are CommonJS-compatible and can render to SVG in a short-lived
    Node child process instead.  The spec is sent over stdin (never embedded
    in a command line), and only the resulting SVG crosses the HTTP boundary.
    """
    normalized = str(language or "vega-lite").strip().lower()
    if normalized not in {"vega", "vega-lite"}:
        raise DiagramRenderError("diagram_engine_invalid")
    raw = str(spec_text or "").strip()
    if not raw or len(raw.encode("utf-8")) > 2 * 1024 * 1024:
        raise DiagramRenderError("diagram_input_too_large")
    try:
        spec = json.loads(raw)
    except (TypeError, ValueError):
        # Keep the offline renderer deterministic and dependency-free.  YAML
        # can still be converted to JSON by the editor before insertion.
        raise DiagramRenderError("diagram_invalid_input")
    if not isinstance(spec, dict):
        raise DiagramRenderError("diagram_invalid_input")

    root = Path(__file__).resolve().parents[2]
    node = _node_runtime(root)
    vega_path = root / "assets" / "vendor" / "diagrams" / "vega" / "vega.min.js"
    vega_lite_path = root / "assets" / "vendor" / "diagrams" / "vega-lite" / "vega-lite.min.js"
    if not node or not vega_path.is_file() or not vega_lite_path.is_file():
        raise DiagramRenderError("diagram_dependency_missing")

    # This script is constant; user input is JSON on stdin only.  ``renderer:
    # none`` avoids a native canvas dependency while ``toSVG`` remains fully
    # offline and preserves Vega's own scenegraph semantics.
    script = r"""
const fs = require('fs');
const vega = require(process.argv[1]);
const vegaLite = require(process.argv[2]);
const language = process.argv[3];
let raw = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => raw += chunk);
process.stdin.on('end', async () => {
  try {
    const source = JSON.parse(raw);
    const compiled = language === 'vega-lite' ? vegaLite.compile(source).spec : source;
    const loader = (typeof vega.loader === 'function')
      ? vega.loader({ load: () => Promise.reject(new Error('external data loading blocked in offline mode')) })
      : undefined;
    const view = new vega.View(vega.parse(compiled), { renderer: 'none', loader });
    await view.runAsync();
    const svg = await view.toSVG();
    process.stdout.write(String(svg || ''));
  } catch (_) {
    process.exitCode = 2;
  }
});
"""
    try:
        result = subprocess.run(
            [node, "-e", script, str(vega_path), str(vega_lite_path), normalized],
            input=json.dumps(spec, ensure_ascii=False).encode("utf-8"),
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=max(1.0, min(float(timeout), 30.0)),
            cwd=str(root),
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        raise DiagramRenderError("diagram_engine_timeout")
    if result.returncode != 0:
        raise DiagramRenderError("diagram_render_failed")
    svg = result.stdout.decode("utf-8", errors="strict").strip()
    if not svg.startswith("<svg") or len(svg.encode("utf-8")) > 8 * 1024 * 1024:
        raise DiagramRenderError("diagram_render_failed")
    return svg


def format_tikz_html(tikz_code: str) -> str:
    """将 TikZ 代码片段包装为 TikZjax 标准 HTML 节点。"""
    # TikZ is inserted into a script element by the browser renderer.  A
    # literal closing tag in user content must not be allowed to terminate the
    # element and inject arbitrary markup.  Escaping only the slash keeps the
    # TeX source semantically identical to TikZjax while making the HTML
    # boundary unambiguous.
    code = str(tikz_code or '').strip().replace('</script', '<\\/script')
    if not code.startswith("\\begin{tikzpicture}"):
        code = f"\\begin{{tikzpicture}}\n{code}\n\\end{{tikzpicture}}"
    return f'<script type="text/tikz">\n{code}\n</script>'


def identify_diagram_blocks(markdown: str) -> List[Dict[str, any]]:
    """扫描 Markdown 中的所有专业图表代码块。"""
    pattern = re.compile(
        r'```(mermaid|plantuml|puml|wsd|wavedrom|bitfield|viz|dot|vega-lite|vega|chart\.js|chartjs|chart|d2|tikz)\b[^\n]*\n([\s\S]*?)```',
        re.IGNORECASE
    )
    diagrams = []
    for match in pattern.finditer(markdown):
        diag_type = match.group(1).lower()
        content = match.group(2).strip()
        diagrams.append({
            "type": diag_type,
            "content": content,
            "span": match.span()
        })

    # 另外识别 ```latex {tikz=true}
    latex_tikz_pattern = re.compile(
        r'```latex\b[^\n]*\{[^}]*tikz\s*=\s*true[^}]*\}[^\n]*\n([\s\S]*?)```',
        re.IGNORECASE
    )
    for match in latex_tikz_pattern.finditer(markdown):
        diagrams.append({
            "type": "tikz",
            "content": match.group(1).strip(),
            "span": match.span()
        })

    return diagrams
