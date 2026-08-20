# -*- coding: utf-8 -*-
"""ReadMD LaTeX PRO - 纯 Python BibTeX 参考文献解析与学术交叉引用引擎。

支持：
1. 自动扫描 Markdown 同目录或上级目录下的 .bib 文件；
2. 解析 @article, @book, @inproceedings, @techreport, @phdthesis, @misc 等条目；
3. 提取 Title, Author, Year/Date, Journal/Booktitle, Volume, Pages, DOI, URL；
4. 格式化学术引用短名（如 "Vaswani et al., 2017"）与完整标准参考文献条目；
5. 零第三方依赖，纯标准库实现。
"""

import os
import re
import logging


def parse_bibtex_file(file_path):
    """解析单个 .bib 文件并返回字典 {cite_key: entry_dict}。"""
    if not file_path or not os.path.isfile(file_path):
        return {}

    entries = {}
    try:
        with open(file_path, 'r', encoding='utf-8', errors='replace') as f:
            content = f.read()
    except Exception as e:
        logging.warning('Read bib file failed: %s (%s)', file_path, e)
        return {}

    # 正则匹配 @type{key, fields...}
    # 匹配 @type{...}，处理嵌套括号
    pattern = re.compile(r'@([a-zA-Z]+)\s*\{\s*([^,\s]+)\s*,', re.IGNORECASE)
    
    pos = 0
    while True:
        m = pattern.search(content, pos)
        if not m:
            break
        entry_type = m.group(1).lower()
        cite_key = m.group(2).strip()
        
        # 寻找匹配的闭合大括号
        start_idx = m.end()
        brace_count = 1
        end_idx = start_idx
        for i in range(start_idx, len(content)):
            char = content[i]
            if char == '{':
                brace_count += 1
            elif char == '}':
                brace_count -= 1
                if brace_count == 0:
                    end_idx = i
                    break
        
        body = content[start_idx:end_idx]
        pos = end_idx + 1

        if entry_type == 'comment':
            continue

        # 解析字段 key = {value} 或 key = "value" 或 key = 123
        fields = parse_bib_fields(body)
        fields['entry_type'] = entry_type
        fields['cite_key'] = cite_key
        
        # 生成短引用文本（如 "Vaswani et al., 2017"）
        fields['short_cite'] = format_short_cite(fields)
        fields['full_reference'] = format_full_reference(fields)
        
        entries[cite_key] = fields

    return entries


def parse_bib_fields(body):
    """解析 BibTeX 条目内部的 key = value 字段。"""
    fields = {}
    field_pattern = re.compile(r'([a-zA-Z0-9_\-]+)\s*=\s*', re.IGNORECASE)
    
    pos = 0
    while pos < len(body):
        m = field_pattern.search(body, pos)
        if not m:
            break
        key = m.group(1).lower().strip()
        val_start = m.end()
        
        # 判断值是用 { }, " " 还是无括号
        val = ''
        next_pos = val_start
        if val_start < len(body):
            first_char = body[val_start]
            if first_char == '{':
                b_count = 1
                i = val_start + 1
                while i < len(body):
                    if body[i] == '{':
                        b_count += 1
                    elif body[i] == '}':
                        b_count -= 1
                        if b_count == 0:
                            val = body[val_start + 1:i]
                            next_pos = i + 1
                            break
                    i += 1
            elif first_char == '"':
                i = val_start + 1
                while i < len(body):
                    if body[i] == '"' and body[i - 1] != '\\':
                        val = body[val_start + 1:i]
                        next_pos = i + 1
                        break
                    i += 1
            else:
                # 数字或无引号标识符 直到逗号或换行
                end_m = re.search(r'[,}\n\r]', body[val_start:])
                if end_m:
                    val = body[val_start:val_start + end_m.start()].strip()
                    next_pos = val_start + end_m.end()
                else:
                    val = body[val_start:].strip()
                    next_pos = len(body)
        
        # 清理多余大括号与换行缩进
        clean_val = re.sub(r'[\r\n\t]+', ' ', val).strip()
        clean_val = re.sub(r'\{|\}', '', clean_val)
        fields[key] = clean_val
        pos = next_pos

    return fields


def format_short_cite(fields):
    """生成短学术引用标签，例如 [Vaswani et al., 2017]。"""
    author = fields.get('author', '').strip()
    year = fields.get('year', '') or fields.get('date', '')
    if not year and fields.get('year'):
        year = str(fields.get('year'))

    if not author:
        return f"[{fields.get('cite_key')}]"

    # 分割多个作者 (BibTeX 中以 and 分隔)
    authors = [a.strip() for a in author.split(' and ') if a.strip()]
    if not authors:
        return f"[{fields.get('cite_key')}]"

    first_author = authors[0]
    # 处理 "Lastname, Firstname" 格式
    if ',' in first_author:
        first_last = first_author.split(',')[0].strip()
    else:
        parts = first_author.split()
        first_last = parts[-1] if parts else first_author

    if len(authors) == 1:
        cite_str = f"{first_last}, {year}" if year else first_last
    elif len(authors) == 2:
        second_author = authors[1]
        second_last = second_author.split(',')[0].strip() if ',' in second_author else second_author.split()[-1]
        cite_str = f"{first_last} & {second_last}, {year}" if year else f"{first_last} & {second_last}"
    else:
        cite_str = f"{first_last} et al., {year}" if year else f"{first_last} et al."

    return f"[{cite_str}]"


def format_full_reference(fields):
    """格式化标准参考文献条目文本（APA/IEEE 风格）。"""
    author = fields.get('author', '').replace(' and ', ', ')
    year = fields.get('year', '')
    title = fields.get('title', '')
    journal = fields.get('journal', '') or fields.get('booktitle', '') or fields.get('publisher', '')
    volume = fields.get('volume', '')
    pages = fields.get('pages', '')
    doi = fields.get('doi', '')

    parts = []
    if author:
        parts.append(f"{author}.")
    if year:
        parts.append(f"({year}).")
    if title:
        parts.append(f"{title}.")
    if journal:
        j_part = f"_{journal}_"
        if volume:
            j_part += f", {volume}"
        if pages:
            j_part += f", {pages}"
        parts.append(j_part + ".")
    if doi:
        parts.append(f"https://doi.org/{doi}")

    return " ".join(parts)


def find_and_load_bib_for_file(markdown_file_path):
    """自动查找并载入 Markdown 文件同级或上级目录的 .bib 文件。"""
    if not markdown_file_path:
        return {}

    base_dir = os.path.dirname(os.path.abspath(markdown_file_path)) if os.path.isfile(markdown_file_path) else os.path.abspath(markdown_file_path)
    if not os.path.isdir(base_dir):
        return {}

    bib_files = []
    # 扫描当前目录下的 .bib
    for f in os.listdir(base_dir):
        if f.lower().endswith('.bib'):
            bib_files.append(os.path.join(base_dir, f))

    # 如果当前目录没有，扫描上一级目录
    if not bib_files:
        parent_dir = os.path.dirname(base_dir)
        if os.path.isdir(parent_dir) and parent_dir != base_dir:
            for f in os.listdir(parent_dir):
                if f.lower().endswith('.bib'):
                    bib_files.append(os.path.join(parent_dir, f))

    all_citations = {}
    for bf in bib_files:
        parsed = parse_bibtex_file(bf)
        all_citations.update(parsed)

    return all_citations
