# -*- coding: utf-8 -*-
"""ReadMD 文档工程化与模块化导入处理器 (@import Processor)。

支持语法：
1. 子 Markdown 章节递归嵌入：
   @import "sub_chapter.md"
2. CSV / TSV 数据表格动态解析与转换：
   @import "dataset.csv"
3. 真实源码文件局部行号范围切片：
   @import "app.py" {line_begin=10 line_end=30 highlight=[15, 18] as_code=true}
4. 图表与矢量文件嵌入：
   @import "diagram.puml"
   @import "graph.dot"

安全防线：
- 路径遍历防御 (越权路径检查)
- 循环依赖引用检测 (Circular Import Detection)
- 递归嵌套深度上限 (默认最大 8 层)
"""

import base64
import csv
import io
import os
import re
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

IMPORT_PATTERN = re.compile(
    r'^[ \t]*@import\s+["\']([^"\']+)["\'](?:\s*\{([^}]*)\})?[ \t]*$',
    re.MULTILINE
)

MAX_IMPORT_DEPTH = 8


def _is_inside_root(root: str, target: str) -> bool:
    """Return True only when target resolves inside root, including symlink targets."""
    root_path = Path(root).resolve(strict=False)
    target_path = Path(target).resolve(strict=False)
    try:
        target_path.relative_to(root_path)
        return True
    except ValueError:
        return False


def parse_attributes(attr_str: Optional[str]) -> Dict[str, any]:
    """解析 @import 后的键值属性，如 {line_begin=10 line_end=20 as_code=true}。"""
    attrs: Dict[str, any] = {}
    if not attr_str:
        return attrs

    attr_str = attr_str.strip()
    # 匹配 key=val 或 key=[...] 或 key="val"
    token_pattern = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(\[[^\]]*\]|"[^"]*"|\'[^\']*\'|[^\s,]+)')
    for match in token_pattern.finditer(attr_str):
        k = match.group(1).lower()
        v = match.group(2).strip()
        if v.startswith('[') and v.endswith(']'):
            try:
                # 解析数组，如 [15, 18]
                items = [int(x.strip()) for x in v[1:-1].split(',') if x.strip().isdigit()]
                attrs[k] = items
            except Exception:
                attrs[k] = v
        elif (v.startswith('"') and v.endswith('"')) or (v.startswith("'") and v.endswith("'")):
            attrs[k] = v[1:-1]
        elif v.lower() == 'true':
            attrs[k] = True
        elif v.lower() == 'false':
            attrs[k] = False
        elif v.isdigit():
            attrs[k] = int(v)
        else:
            attrs[k] = v
    return attrs


def csv_to_markdown_table(csv_content: str, delimiter: str = ',') -> str:
    """将 CSV/TSV 文本格式化为标准 Markdown 表格。"""
    f = io.StringIO(csv_content.strip())
    reader = csv.reader(f, delimiter=delimiter)
    rows = list(reader)
    if not rows:
        return ""

    headers = rows[0]
    num_cols = len(headers)
    if num_cols == 0:
        return ""

    md_lines = []
    # 表头
    md_lines.append("| " + " | ".join(h.strip().replace("|", "\\|") for h in headers) + " |")
    # 分隔线
    md_lines.append("| " + " | ".join("---" for _ in range(num_cols)) + " |")

    # 数据行
    for row in rows[1:]:
        padded_row = row + [""] * (num_cols - len(row))
        md_lines.append("| " + " | ".join(cell.strip().replace("|", "\\|") for cell in padded_row[:num_cols]) + " |")

    return "\n".join(md_lines)


def slice_code_lines(code_content: str, line_begin: Optional[int] = None,
                     line_end: Optional[int] = None, lang: str = "") -> str:
    """按行号范围对源码切片并包装为 Markdown 代码块。"""
    lines = code_content.splitlines()
    total_lines = len(lines)

    start = max(1, line_begin) if line_begin is not None else 1
    end = min(total_lines, line_end) if line_end is not None else total_lines

    if start > total_lines:
        sliced = ""
    else:
        sliced_lines = lines[start - 1:end]
        sliced = "\n".join(sliced_lines)

    fence = "```"
    return f"{fence}{lang}\n{sliced}\n{fence}"


class ImportProcessor:
    """文档模块化导入解析器。"""

    def __init__(self, base_dir: str, allow_private: bool = True):
        self.base_dir = os.path.abspath(base_dir) if base_dir else os.getcwd()
        self.allow_private = allow_private

    def process(self, content: str, current_file: Optional[str] = None,
                visited: Optional[Set[str]] = None, depth: int = 0) -> str:
        """递归解析并展平 Markdown 文档中的全部 @import 指令。"""
        if visited is None:
            visited = set()

        if current_file:
            abs_curr = os.path.abspath(current_file)
            visited.add(abs_curr)
            curr_dir = os.path.dirname(abs_curr)
        else:
            curr_dir = self.base_dir

        if depth >= MAX_IMPORT_DEPTH:
            return f"\n> **[ReadMD 警告]**: 达到最大 @import 嵌套深度限制 ({MAX_IMPORT_DEPTH} 层)，已停止继续递归。\n"

        def replace_import(match: re.Match) -> str:
            raw_path = match.group(1).strip()
            raw_attrs = match.group(2)
            attrs = parse_attributes(raw_attrs)

            # 忽略 [TOC] 伪导入（留给 TOC 引擎处理）
            if raw_path.upper() == '[TOC]':
                return match.group(0)

            # 解析目标路径
            if os.path.isabs(raw_path):
                target_path = os.path.abspath(raw_path)
            else:
                target_path = os.path.abspath(os.path.join(curr_dir, raw_path))

            # Enforce the documented document-root boundary after resolving links.
            if not _is_inside_root(self.base_dir, target_path):
                return f"\n> **[ReadMD 错误]**: 导入文件越权路径，已拒绝 `@import \"{raw_path}\"`\n"

            # 循环引用防御
            if target_path in visited:
                return f"\n> **[ReadMD 警告]**: 检测到循环引用 `@import \"{raw_path}\"`，已自动忽略。\n"

            # 文件存在性检查
            if not os.path.isfile(target_path):
                return f"\n> **[ReadMD 错误]**: 导入文件不存在 `@import \"{raw_path}\"` ({target_path})\n"

            ext = os.path.splitext(target_path)[1].lower()

            try:
                with open(target_path, 'r', encoding='utf-8', errors='replace') as f:
                    file_text = f.read()
            except Exception as e:
                return f"\n> **[ReadMD 错误]**: 读取文件失败 `{raw_path}`: {str(e)}\n"

            # 1. 导入子 Markdown
            if ext in ('.md', '.markdown', '.mdown'):
                sub_visited = set(visited)
                return self.process(file_text, current_file=target_path, visited=sub_visited, depth=depth + 1)

            # 2. 导入 CSV / TSV 数据表
            elif ext == '.csv':
                return csv_to_markdown_table(file_text, delimiter=',')
            elif ext == '.tsv':
                return csv_to_markdown_table(file_text, delimiter='\t')

            # 3. 导入 PDF 页面切片
            elif ext == '.pdf':
                page_no = attrs.get('page_no') or attrs.get('page') or 1
                try:
                    import fitz  # PyMuPDF
                    doc = fitz.open(target_path)
                    idx = max(0, min(int(page_no) - 1, len(doc) - 1))
                    page = doc[idx]
                    pix = page.get_pixmap(dpi=150)
                    img_bytes = pix.tobytes("png")
                    b64_str = base64.b64encode(img_bytes).decode('ascii')
                    doc.close()
                    return f"![PDF Page {page_no}](data:image/png;base64,{b64_str})"
                except Exception as e:
                    return f"\n> **[ReadMD 错误]**: 提取 PDF 页面失败 `{raw_path}` (页码 {page_no}): {str(e)}\n"

            # 4. 导入 LESS 样式
            elif ext == '.less':
                return f'<style type="text/less">\n{file_text.strip()}\n</style>'

            # 5. 导入 TikZ 矢量图
            elif ext == '.tikz' or (ext == '.tex' and attrs.get('tikz')):
                from src.readmd_modules.diagrams import format_tikz_html
                return format_tikz_html(file_text)

            # 6. 导入图表源码文件 (PUML / DOT / WaveDrom)
            elif ext in ('.puml', '.plantuml'):
                return f"```puml\n{file_text.strip()}\n```"
            elif ext in ('.dot', '.viz'):
                return f"```viz\n{file_text.strip()}\n```"
            elif ext == '.wavedrom':
                return f"```wavedrom\n{file_text.strip()}\n```"

            # 7. 导入源码切片或普通代码块
            else:
                lang = attrs.get('lang', ext.lstrip('.'))
                line_begin = attrs.get('line_begin')
                line_end = attrs.get('line_end')
                return slice_code_lines(file_text, line_begin=line_begin, line_end=line_end, lang=lang)

        return IMPORT_PATTERN.sub(replace_import, content)


def process_markdown_imports(content: str, base_dir: str = "", current_file: Optional[str] = None) -> str:
    """对外快捷调用函数。"""
    processor = ImportProcessor(base_dir=base_dir)
    return processor.process(content, current_file=current_file)
