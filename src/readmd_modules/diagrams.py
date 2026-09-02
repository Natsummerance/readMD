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
    return f"{server_url.rstrip('/')}/svg/~1{encoded}"


def has_local_plantuml() -> bool:
    """探测系统本地是否具备 Java 及 PlantUML 环境。"""
    return bool(shutil.which('plantuml') or (shutil.which('java') and os.environ.get('PLANTUML_JAR')))


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

    node = shutil.which("node")
    root = Path(__file__).resolve().parents[2]
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
    const view = new vega.View(vega.parse(compiled), { renderer: 'none' });
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
        r'```(mermaid|puml|plantuml|wavedrom|bitfield|viz|dot|vega|vega-lite|d2|tikz)\b[^\n]*\n([\s\S]*?)```',
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
