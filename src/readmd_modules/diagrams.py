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
import os
import re
import shutil
import subprocess
import zlib
from typing import Dict, List, Optional, Tuple

PLANTUML_CHARS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_"


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


def format_tikz_html(tikz_code: str) -> str:
    """将 TikZ 代码片段包装为 TikZjax 标准 HTML 节点。"""
    code = tikz_code.strip()
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
