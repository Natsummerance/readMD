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
    r'^[ \t]*@import\s+["\']([^"\']+)["\'](?:\s*\{([^}]*)\})?[ \t]*\r?$',
    re.MULTILINE
)

MAX_IMPORT_DEPTH = 8


class _ImportBudget:
    """Tracks cumulative output and read resource budgets during import processing."""

    def __init__(self, max_output_bytes: Optional[int] = None,
                 max_read_bytes: Optional[int] = None):
        self.max_output_bytes = max_output_bytes
        self.max_read_bytes = max_read_bytes
        self.total_output_bytes = 0
        self.total_read_bytes = 0
        self.exceeded = False

    def can_read(self, n_bytes: int) -> bool:
        if self.exceeded:
            return False
        if self.max_read_bytes is not None and self.total_read_bytes + n_bytes > self.max_read_bytes:
            self.exceeded = True
            return False
        return True

    def record_read(self, n_bytes: int):
        self.total_read_bytes += n_bytes

    def can_output(self, n_bytes: int) -> bool:
        if self.exceeded:
            return False
        if self.max_output_bytes is not None and self.total_output_bytes + n_bytes > self.max_output_bytes:
            self.exceeded = True
            return False
        return True

    def record_output(self, n_bytes: int):
        self.total_output_bytes += n_bytes


def _process_outside_fences(content: str, transform) -> str:
    """Apply transform only to content outside Markdown code fences."""
    lines = content.splitlines(keepends=True)
    if not lines:
        return content

    in_fence = False
    fence_char = ''
    fence_len = 0

    out = []
    normal_chunk = []

    fence_start_re = re.compile(r'^[ \t]{0,3}(`{3,}|~{3,})')

    for line in lines:
        if not in_fence:
            m = fence_start_re.match(line)
            if m:
                if normal_chunk:
                    out.append(transform(''.join(normal_chunk)))
                    normal_chunk = []
                in_fence = True
                fence_char = m.group(1)[0]
                fence_len = len(m.group(1))
                out.append(line)
            else:
                normal_chunk.append(line)
        else:
            out.append(line)
            closer_re = re.compile(rf'^[ \t]{{0,3}}\{fence_char}{{{fence_len},}}[ \t]*\r?$')
            if closer_re.match(line):
                in_fence = False
                fence_char = ''
                fence_len = 0

    if normal_chunk:
        out.append(transform(''.join(normal_chunk)))

    return ''.join(out)


def _parse_positive_int(val: any) -> Optional[int]:
    if val is None:
        return None
    if isinstance(val, bool):
        return None
    if isinstance(val, int):
        return val if val > 0 else None
    if isinstance(val, str):
        val = val.strip()
        if val.isascii() and val.isdigit():
            try:
                v = int(val)
                return v if v > 0 else None
            except ValueError:
                return None
    return None


def _format_diagram_block(code: str, lang: str) -> str:
    body = code.strip()
    longest = max((len(m.group(0)) for m in re.finditer(r'`+', body)), default=0)
    fence = '`' * max(3, longest + 1)
    return f"{fence}{lang}\n{body}\n{fence}"


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
    """解析 @import 的键值对属性，如 {line_begin=10 line_end=20 as_code=true}。"""
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
                items = []
                for x in v[1:-1].split(','):
                    s = x.strip()
                    if s.isascii() and s.isdigit():
                        try:
                            items.append(int(s))
                        except ValueError:
                            items.append(s)
                    else:
                        items.append(s)
                attrs[k] = items
            except Exception:
                attrs[k] = v
        elif (v.startswith('"') and v.endswith('"')) or (v.startswith("'") and v.endswith("'")):
            attrs[k] = v[1:-1]
        elif v.lower() == 'true':
            attrs[k] = True
        elif v.lower() == 'false':
            attrs[k] = False
        elif v.isascii() and v.isdigit():
            try:
                attrs[k] = int(v)
            except ValueError:
                attrs[k] = v
        else:
            attrs[k] = v
    return attrs


def _format_markdown_cell(raw: str) -> str:
    """Format cell value for Markdown tables, converting newlines to <br> and escaping pipes."""
    if not raw:
        return ""
    s = raw.replace("\r\n", "\n").replace("\r", "\n")
    s = s.replace("\n", "<br>")
    s = s.replace("|", "\\|")
    return s.strip()


def csv_to_markdown_table(csv_content: str, delimiter: str = ',') -> str:
    """将 CSV/TSV 文本格式化为标准 Markdown 表格。"""
    f = io.StringIO(csv_content.strip())
    reader = csv.reader(f, delimiter=delimiter)
    rows = list(reader)
    if not rows:
        return ""

    num_cols = max((len(r) for r in rows), default=0)
    if num_cols == 0:
        return ""

    headers = rows[0] + [""] * (num_cols - len(rows[0]))
    md_lines = []
    # 表头
    md_lines.append("| " + " | ".join(_format_markdown_cell(h) for h in headers) + " |")
    # 分隔线
    md_lines.append("| " + " | ".join("---" for _ in range(num_cols)) + " |")

    # 数据行
    for row in rows[1:]:
        padded_row = row + [""] * (num_cols - len(row))
        md_lines.append("| " + " | ".join(_format_markdown_cell(cell) for cell in padded_row[:num_cols]) + " |")

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

    longest = max((len(m.group(0)) for m in re.finditer(r'`+', sliced)), default=0)
    fence = '`' * max(3, longest + 1)
    return f"{fence}{lang}\n{sliced}\n{fence}"


class ImportProcessor:
    """文档模块化导入解析器。"""

    def __init__(self, base_dir: str, allow_private: bool = True,
                 max_output_bytes: Optional[int] = None,
                 max_read_bytes: Optional[int] = None):
        self.base_dir = os.path.abspath(base_dir) if base_dir else os.getcwd()
        self.allow_private = allow_private
        self.max_output_bytes = max_output_bytes
        self.max_read_bytes = max_read_bytes

    def process(self, content: str, current_file: Optional[str] = None,
                visited: Optional[Set[str]] = None, depth: int = 0,
                budget: Optional[_ImportBudget] = None) -> str:
        """递归解析并展平 Markdown 文档中的全部 @import 指令。"""
        if visited is None:
            visited = set()

        if budget is None:
            budget = _ImportBudget(self.max_output_bytes, self.max_read_bytes)
            budget.record_output(len(content.encode('utf-8')))

        if current_file:
            abs_curr = os.path.abspath(current_file)
            visited.add(abs_curr)
            curr_dir = os.path.dirname(abs_curr)
        else:
            curr_dir = self.base_dir

        def replace_import(match: re.Match) -> str:
            if depth >= MAX_IMPORT_DEPTH:
                return f"\n> **[ReadMD 警告]**: 达到最大 @import 嵌套深度限制 ({MAX_IMPORT_DEPTH} 层)，已停止继续递归。\n"

            if budget.exceeded:
                return "\n> **[ReadMD 错误]**: import_budget_exceeded\n"

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

            file_size = os.path.getsize(target_path)
            if not budget.can_read(file_size):
                return "\n> **[ReadMD 错误]**: import_budget_exceeded\n"
            budget.record_read(file_size)

            ext = os.path.splitext(target_path)[1].lower()

            # 3. 导入 PDF 页面切片
            if ext == '.pdf':
                page_no = attrs.get('page_no') or attrs.get('page') or 1
                doc = None
                try:
                    import fitz  # PyMuPDF
                    doc = fitz.open(target_path)
                    idx = max(0, min(int(page_no) - 1, len(doc) - 1))
                    page = doc[idx]
                    pix = page.get_pixmap(dpi=150)
                    img_bytes = pix.tobytes("png")
                    b64_str = base64.b64encode(img_bytes).decode('ascii')
                    result_chunk = f"![PDF Page {page_no}](data:image/png;base64,{b64_str})"
                except Exception as e:
                    result_chunk = f"\n> **[ReadMD 错误]**: 提取 PDF 页面失败 `{raw_path}` (页码 {page_no}): {str(e)}\n"
                finally:
                    if doc is not None:
                        try:
                            doc.close()
                        except Exception:
                            pass
                if not budget.can_output(len(result_chunk.encode('utf-8'))):
                    return "\n> **[ReadMD 错误]**: import_budget_exceeded\n"
                budget.record_output(len(result_chunk.encode('utf-8')))
                return result_chunk

            try:
                with open(target_path, 'rb') as f:
                    raw_bytes = f.read()
            except Exception as e:
                return f"\n> **[ReadMD 错误]**: 读取文件失败 `{raw_path}`: {str(e)}\n"

            file_text = None
            user_enc = attrs.get('encoding')
            encodings_to_try = [user_enc] if user_enc else ['utf-8-sig', 'utf-8', 'gb18030', 'big5']
            for enc in encodings_to_try:
                if not enc:
                    continue
                try:
                    file_text = raw_bytes.decode(enc)
                    break
                except (UnicodeDecodeError, LookupError):
                    continue

            if file_text is None:
                return f"\n> **[ReadMD 错误]**: unsupported_encoding `{raw_path}`\n"

            # 1. 导入子 Markdown
            if ext in ('.md', '.markdown', '.mdown'):
                sub_visited = set(visited)
                res = self.process(file_text, current_file=target_path, visited=sub_visited,
                                   depth=depth + 1, budget=budget)
                if budget.exceeded:
                    return "\n> **[ReadMD 错误]**: import_budget_exceeded\n"
                return res

            # 2. 导入 CSV / TSV 数据表
            elif ext == '.csv':
                res = csv_to_markdown_table(file_text, delimiter=',')
            elif ext == '.tsv':
                res = csv_to_markdown_table(file_text, delimiter='\t')

            # 4. 导入 LESS 样式
            elif ext == '.less':
                res = f'<style type="text/less">\n{file_text.strip()}\n</style>'

            # 5. 导入 TikZ 矢量图
            elif ext == '.tikz' or (ext == '.tex' and attrs.get('tikz')):
                from src.readmd_modules.diagrams import format_tikz_html
                res = format_tikz_html(file_text)

            # 6. 导入图表源码文件 (PUML / DOT / WaveDrom)
            elif ext in ('.puml', '.plantuml'):
                res = _format_diagram_block(file_text, "puml")
            elif ext in ('.dot', '.viz'):
                res = _format_diagram_block(file_text, "viz")
            elif ext == '.wavedrom':
                res = _format_diagram_block(file_text, "wavedrom")

            # 7. 导入源码切片或普通代码块
            else:
                lang = attrs.get('lang', ext.lstrip('.'))
                lb = None
                le = None
                if 'line_begin' in attrs:
                    lb = _parse_positive_int(attrs['line_begin'])
                    if lb is None:
                        return "\n> **[ReadMD 错误]**: invalid_line_range\n"
                if 'line_end' in attrs:
                    le = _parse_positive_int(attrs['line_end'])
                    if le is None:
                        return "\n> **[ReadMD 错误]**: invalid_line_range\n"
                if lb is not None and le is not None and le < lb:
                    return "\n> **[ReadMD 错误]**: invalid_line_range\n"
                res = slice_code_lines(file_text, line_begin=lb, line_end=le, lang=lang)

            if not budget.can_output(len(res.encode('utf-8'))):
                return "\n> **[ReadMD 错误]**: import_budget_exceeded\n"
            budget.record_output(len(res.encode('utf-8')))
            return res

        transformed = _process_outside_fences(content, lambda text: IMPORT_PATTERN.sub(replace_import, text))
        if budget.max_output_bytes is not None:
            limit = max(0, int(budget.max_output_bytes))
            enc = transformed.encode('utf-8')
            if len(enc) > limit:
                tag = b"\n> **[ReadMD \xe9\x94\x99\xe8\xaf\xaf]**: import_budget_exceeded\n"
                if limit >= len(tag):
                    cutoff = limit - len(tag)
                    transformed = enc[:cutoff].decode('utf-8', errors='ignore') + tag.decode('utf-8')
                else:
                    transformed = tag[:limit].decode('utf-8', errors='ignore')
        return transformed


def process_markdown_imports(content: str, base_dir: str = "", current_file: Optional[str] = None,
                             max_output_bytes: Optional[int] = None,
                             max_read_bytes: Optional[int] = None) -> str:
    """对外快捷调用函数。"""
    processor = ImportProcessor(base_dir=base_dir, max_output_bytes=max_output_bytes,
                                max_read_bytes=max_read_bytes)
    return processor.process(content, current_file=current_file)
