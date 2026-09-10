# -*- coding: utf-8 -*-
"""readmd_modules/convert_ext.py
从飞鼠格式 (FlyingMouse Format) 定向移植的纯标准库轻量扩展转换器。
涵盖：
1. ebook.js 移植：EPUB 解包为结构化 Markdown
2. pdf-table-extractor.js 移植：基于词坐标与网格聚类的复杂表格与版式提取
3. media.js 适配：音频/视频提取转录
零外部巨型依赖，纯 Python 编写。
"""

class ClusteredTableProxy:
    """包装 convert_ext 聚类的表格对象，兼容 PyMuPDF Table 接口"""
    def __init__(self, md_text: str, bbox):
        self._md = md_text
        self.bbox = bbox

    def extract(self):
        return []

    def get_markdown(self):
        return self._md


def extract_page_table_cluster(page):
    """从 PyMuPDF page 提取 word blocks 并通过 FlyingMouse 算法聚类为表格"""
    try:
        # 1. 过滤：寻找多列水平排布特征，若全页只是普通单列文本，则不聚类
        raw_words = page.get_text('words')
        if not raw_words or len(raw_words) < 6:
            return None
            
        # 按行探测：统计每行内的词块在 X 轴上的分布
        sorted_by_y = sorted(raw_words, key=lambda w: w[1])
        candidate_rows = []
        cur_row = [sorted_by_y[0]]
        for w in sorted_by_y[1:]:
            row_y = sum(item[1] for item in cur_row) / len(cur_row)
            if abs(w[1] - row_y) <= 6.0:
                cur_row.append(w)
            else:
                candidate_rows.append(cur_row)
                cur_row = [w]
        if cur_row:
            candidate_rows.append(cur_row)
            
        # 找出具有多列特征的连续行 (至少 2 列，连续 >= 2 行)
        table_rows = []
        for r in candidate_rows:
            # 只有当一行内有 >= 2 个独立分散的词块时才算多列行（单段长句排除）
            if len(r) >= 2:
                # 检查词块间的间隙，必须存在明显的列间隙 (> 15 pt)
                sorted_r = sorted(r, key=lambda w: w[0])
                # 排除只有一个大段落但被分成两截的情况：要求至少有一处显著列间距
                gaps = [(sorted_r[i+1][0] - sorted_r[i][2]) for i in range(len(sorted_r)-1)]
                if any(g > 20 for g in gaps):
                    table_rows.append(r)
                
        # 必须至少有 2 个多列行且必须是连续或密集的
        if len(table_rows) < 2:
            return None
            
        table_words = [w for r in table_rows for w in r]
        min_x = min(w[0] for w in table_words)
        min_y = min(w[1] for w in table_words)
        max_x = max(w[2] for w in table_words)
        max_y = max(w[3] for w in table_words)

        words = []
        for w in table_words:
            words.append({
                'x': float(w[0]),
                'y': float(w[1]),
                'width': float(w[2] - w[0]),
                'height': float(w[3] - w[1]),
                'text': str(w[4])
            })
        md = cluster_words_into_table(words)
        if md and '| ---' in md:
            return ClusteredTableProxy(md, (min_x, min_y, max_x, max_y))
    except Exception:
        pass
    return None


import os
import re
import zipfile
import xml.etree.ElementTree as ET
from typing import List, Dict, Any, Optional

# ==========================================
# 1. 移植自飞鼠 ebook.js: EPUB -> 高保真 Markdown
# ==========================================

def html_to_markdown_clean(html_content: str) -> str:
    """移植自飞鼠 ebook.js / text-conversion.js 的 HTML 清洗与 Markdown 转换"""
    text = html_content
    # 去除 script/style
    text = re.sub(r'<script[\s\S]*?</script>', '', text, flags=re.I)
    text = re.sub(r'<style[\s\S]*?</style>', '', text, flags=re.I)
    
    # 标题转换
    for i in range(6, 0, -1):
        text = re.sub(rf'<h{i}[^>]*>([\s\S]*?)</h{i}>', rf'\n\n{"#" * i} \1\n\n', text, flags=re.I)
        
    # 段落与换行
    text = re.sub(r'<p[^>]*>([\s\S]*?)</p>', r'\n\n\1\n\n', text, flags=re.I)
    text = re.sub(r'<br\s*/?>', r'\n', text, flags=re.I)
    
    # 列表转换
    text = re.sub(r'<li[^>]*>([\s\S]*?)</li>', r'\n- \1', text, flags=re.I)
    
    # 加粗与斜体
    text = re.sub(r'<(b|strong)[^>]*>([\s\S]*?)</\1>', r'**\2**', text, flags=re.I)
    text = re.sub(r'<(i|em)[^>]*>([\s\S]*?)</\1>', r'*\2*', text, flags=re.I)
    
    # 剥离所有剩余标签
    text = re.sub(r'<[^>]+>', '', text)
    
    # 实体替换
    text = text.replace('&nbsp;', ' ').replace('&amp;', '&').replace('&lt;', '<').replace('&gt;', '>').replace('&quot;', '"')
    text = re.sub(r'\n{3,}', '\n\n', text)
    return text.strip()

def epub_to_markdown(epub_path: str) -> str:
    """纯标准库解析 EPUB 电子书并按章节顺序组装为完整 Markdown"""
    if not zipfile.is_zipfile(epub_path):
        raise ValueError("无效的 EPUB 文件")
        
    md_parts = []
    with zipfile.ZipFile(epub_path, 'r') as zf:
        # 1. 找 container.xml
        try:
            container_xml = zf.read('META-INF/container.xml')
            root = ET.fromstring(container_xml)
            opf_path = root.find('.//{*}rootfile').attrib['full-path']
        except Exception:
            opf_path = 'content.opf'
            
        opf_dir = os.path.dirname(opf_path)
        
        # 2. 解析 opf 获取阅读顺序 (spine)
        try:
            opf_content = zf.read(opf_path)
            opf_root = ET.fromstring(opf_content)
            
            # manifest 映射 id -> href
            manifest = {}
            for item in opf_root.findall('.//{*}manifest/{*}item'):
                manifest[item.attrib['id']] = item.attrib['href']
                
            # spine 顺序
            spine_ids = [itemref.attrib['idref'] for itemref in opf_root.findall('.//{*}spine/{*}itemref')]
            
            for item_id in spine_ids:
                if item_id in manifest:
                    href = manifest[item_id]
                    doc_path = os.path.normpath(os.path.join(opf_dir, href)).replace('\\', '/')
                    if doc_path in zf.namelist():
                        html_src = zf.read(doc_path).decode('utf-8', errors='ignore')
                        ch_md = html_to_markdown_clean(html_src)
                        if ch_md:
                            md_parts.append(ch_md)
        except Exception:
            # 兜底：直接按所有 html/xhtml 遍历
            for name in zf.namelist():
                if name.endswith(('.html', '.xhtml', '.htm')) and not name.startswith('__'):
                    html_src = zf.read(name).decode('utf-8', errors='ignore')
                    ch_md = html_to_markdown_clean(html_src)
                    if ch_md:
                        md_parts.append(ch_md)
                        
    return "\n\n---\n\n".join(md_parts)

# =========================================================
# 2. 移植自飞鼠 pdf-table-extractor.js: 词坐标网格聚类表格算法
# =========================================================

def cluster_words_into_table(words: List[Dict[str, Any]], y_tolerance: float = 3.0) -> str:
    """移植自飞鼠 pdf-table-extractor.js:
    输入一组带 x, y, width, height, text 的词块，
    通过中位数聚类和网格计算将其还原为标准的 GFM Markdown 表格。
    """
    if not words:
        return ""
        
    # 1. 按垂直 Y 坐标聚类成行 (clusterRows)
    # y_tolerance 设为中文字高/行距感知容差 (通常 5.0 ~ 8.0)
    sorted_by_y = sorted(words, key=lambda w: w['y'])
    rows: List[List[Dict[str, Any]]] = []
    
    current_row = [sorted_by_y[0]]
    for w in sorted_by_y[1:]:
        # 若 Y 坐标距离在容差范围内，归为同一行
        row_y = sum(item['y'] for item in current_row) / len(current_row)
        if abs(w['y'] - row_y) <= max(y_tolerance, 6.0):
            current_row.append(w)
        else:
            rows.append(sorted(current_row, key=lambda item: item['x']))
            current_row = [w]
    if current_row:
        rows.append(sorted(current_row, key=lambda item: item['x']))
        
    if len(rows) < 2:
        # 不成表，直接连成文本
        return " ".join([w['text'] for w in words])
        
    # 2. 动态探测列划分 (clusterColumns)
    all_x_starts = [w['x'] for row in rows for w in row]
    all_x_starts.sort()
    
    col_splits = [all_x_starts[0]]
    for x in all_x_starts[1:]:
        if x - col_splits[-1] > 20.0: # 飞鼠最小列间距阈值
            col_splits.append(x)
            
    num_cols = len(col_splits)
    if num_cols < 2:
        # 单列，不转表
        return "\n\n".join([" ".join([w['text'] for w in r]) for r in rows])
        
    # 3. 网格矩阵对齐填充
    grid = [["" for _ in range(num_cols)] for _ in range(len(rows))]
    
    for r_idx, row in enumerate(rows):
        for w in row:
            # 寻找最近的列
            best_c = 0
            min_dist = float('inf')
            for c_idx, c_x in enumerate(col_splits):
                dist = abs(w['x'] - c_x)
                if dist < min_dist:
                    min_dist = dist
                    best_c = c_idx
            grid[r_idx][best_c] = (grid[r_idx][best_c] + " " + w['text']).strip()
            
    # 4. 生成标准 Markdown 表格
    md_table_lines = []
    # 表头 (第一行)
    header = "| " + " | ".join([cell if cell else "-" for cell in grid[0]]) + " |"
    divider = "| " + " | ".join(["---" for _ in range(num_cols)]) + " |"
    md_table_lines.append(header)
    md_table_lines.append(divider)
    
    for row_cells in grid[1:]:
        if not any(cell.strip() for cell in row_cells):
            continue
        md_table_lines.append("| " + " | ".join([cell.replace('|', '\\|') if cell else " " for cell in row_cells]) + " |")
        
    return "\n".join(md_table_lines)

# =========================================================
# 3. 适配自飞鼠 media.js: 音频/视频转 Markdown
# =========================================================

def media_to_markdown(media_path: str) -> str:
    """音视频转 Markdown：调用本地 transcribe / whisper"""
    from . import transcribe
    # 直接调用 readMD 现有 transcribe 模块提取带时间轴的会议纪要
    text = transcribe.transcribe(media_path)
    base_name = os.path.basename(media_path)
    return f"# 多媒体录音转写纪要: {base_name}\n\n> 来源文件: `{media_path}`\n\n{text}"
