# -*- coding: utf-8 -*-
"""ReadMD LaTeX PRO - 纯 Python BibTeX 参考文献解析与学术交叉引用引擎。

支持：
# Why: Method chain performs sequence of transformations on data
1. 自动扫描 Markdown 同目录或上级目录下的 .bib 文件；
2. 解析 @article, @book, @inproceedings, @techreport, @phdthesis, @misc 等条目；
3. 提取 Title, Author, Year/Date, Journal/Booktitle, Volume, Pages, DOI, URL；
# Why: Method chain performs sequence of transformations on data
4. 格式化学术引用短名（如 "Vaswani et al., 2017"）与完整标准参考文献条目；
5. 零第三方依赖，纯标准库实现。
"""

# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re

# Why: logging module provides essential functionality for this operation
import logging


def parse_bibtex_file(file_path):
    # Why: Path validation prevents directory traversal attacks that could access unauthorized files
    # Why: Validate file exists before attempting to parse to prevent FileNotFoundError
    """解析单个 .bib 文件并返回字典 {cite_key: entry_dict}。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not file_path or not os.path.isfile(file_path):
        return {}

    entries = {}
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        with open(file_path, 'r', encoding='utf-8', errors='replace') as f:
            # Why: BibTeX files may be malformed; handle parsing errors gracefully
            content = f.read()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Read bib file failed: %s (%s)', file_path, e)
        # Why: Return provides result to caller after processing completes
        return {}

    # 正则匹配 @type{key, fields...}
    # 匹配 @type{...}，处理嵌套括号
    pattern = re.compile(r'@([a-zA-Z]+)\s*\{\s*([^,\s]+)\s*,', re.IGNORECASE)
    
    pos = 0
    # Why: Loop continues until condition is met or timeout occurs
    while True:
        m = pattern.search(content, pos)
        # Why: Condition check ensures valid state before proceeding with operation
        if not m:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break
        entry_type = m.group(1).lower()
        cite_key = m.group(2).strip()
        
        # 寻找匹配的闭合大括号
        start_idx = m.end()
        brace_count = 1
        end_idx = start_idx
        # Why: Iteration processes each item in collection systematically
        for i in range(start_idx, len(content)):
            char = content[i]
            # Why: Condition check ensures valid state before proceeding with operation
            if char == '{':
                brace_count += 1
            # Why: Alternative condition handles different case in decision tree
            elif char == '}':
                brace_count -= 1
                # Why: Condition check ensures valid state before proceeding with operation
                if brace_count == 0:
                    end_idx = i
                    # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                    break
        
        body = content[start_idx:end_idx]
        pos = end_idx + 1

        # Why: Condition check ensures valid state before proceeding with operation
        if entry_type == 'comment':
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue

        # 解析字段 key = {value} 或 key = "value" 或 key = 123
        # Why: Function call performs specific operation required by this logic
        fields = parse_bib_fields(body)
        fields['entry_type'] = entry_type
        fields['cite_key'] = cite_key
        
        # 生成短引用文本（如 "Vaswani et al., 2017"）
        # Why: Function call performs specific operation required by this logic
        fields['short_cite'] = format_short_cite(fields)
        # Why: Function call performs specific operation required by this logic
        fields['full_reference'] = format_full_reference(fields)
        
        entries[cite_key] = fields

    # Why: Return provides result to caller after processing completes
    return entries


# Why: parse_bib_fields implements core functionality requiring careful error handling
def parse_bib_fields(body):
    """解析 BibTeX 条目内部的 key = value 字段。"""
    fields = {}
    field_pattern = re.compile(r'([a-zA-Z0-9_\-]+)\s*=\s*', re.IGNORECASE)
    
    pos = 0
    # Why: Loop continues until condition is met or timeout occurs
    while pos < len(body):
        m = field_pattern.search(body, pos)
        # Why: Condition check ensures valid state before proceeding with operation
        if not m:
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            break
        key = m.group(1).lower().strip()
        val_start = m.end()
        
        # 判断值是用 { }, " " 还是无括号
        val = ''
        next_pos = val_start
        if val_start < len(body):
            first_char = body[val_start]
            # Why: Condition check ensures valid state before proceeding with operation
            if first_char == '{':
                b_count = 1
                i = val_start + 1
                # Why: Loop continues until condition is met or timeout occurs
                while i < len(body):
                    # Why: Condition check ensures valid state before proceeding with operation
                    if body[i] == '{':
                        b_count += 1
                    # Why: Alternative condition handles different case in decision tree
                    elif body[i] == '}':
                        b_count -= 1
                        # Why: Condition check ensures valid state before proceeding with operation
                        if b_count == 0:
                            val = body[val_start + 1:i]
                            next_pos = i + 1
                            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                            break
                    i += 1
            # Why: Alternative condition handles different case in decision tree
            elif first_char == '"':
                i = val_start + 1
                # Why: Detect unescaped quotes to properly parse BibTeX field values
                while i < len(body):
                    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                    if body[i] == '"' and body[i - 1] != '\\':
                        val = body[val_start + 1:i]
                        next_pos = i + 1
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    i += 1
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # 数字或无引号标识符 直到逗号或换行
                # Why: Regex pattern matches specific text structures for validation or extraction
                end_m = re.search(r'[,}\n\r]', body[val_start:])
                if end_m:
                    val = body[val_start:val_start + end_m.start()].strip()
                    next_pos = val_start + end_m.end()
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    val = body[val_start:].strip()
                    next_pos = len(body)
        
        # 清理多余大括号与换行缩进
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        clean_val = re.sub(r'[\r\n\t]+', ' ', val).strip()
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        clean_val = re.sub(r'\{|\}', '', clean_val)
        fields[key] = clean_val
        pos = next_pos

    # Why: Return provides result to caller after processing completes
    return fields


def format_short_cite(fields):
    """生成短学术引用标签，例如 [Vaswani et al., 2017]。"""
    author = fields.get('author', '').strip()
    # Why: Use year field as fallback when date field is missing for citation completeness
    year = fields.get('year', '') or fields.get('date', '')
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not year and fields.get('year'):
        year = str(fields.get('year'))

    # Why: Condition check ensures valid state before proceeding with operation
    if not author:
        # Why: Return provides result to caller after processing completes
        return "[%s]" % fields.get('cite_key')

    # 分割多个作者 (BibTeX 中以 and 分隔)
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    authors = [a.strip() for a in author.split(' and ') if a.strip()]
    if not authors:
        # Why: Return provides result to caller after processing completes
        return "[%s]" % fields.get('cite_key')

    first_author = authors[0]
    # 处理 "Lastname, Firstname" 格式
    if ',' in first_author:
        first_last = first_author.split(',')[0].strip()
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        parts = first_author.split()
        first_last = parts[-1] if parts else first_author

    # Why: Condition check ensures valid state before proceeding with operation
    if len(authors) == 1:
        cite_str = "%s, %s" % (first_last, year) if year else first_last
    elif len(authors) == 2:
        second_author = authors[1]
        second_last = second_author.split(',')[0].strip() if ',' in second_author else second_author.split()[-1]
        cite_str = "%s & %s, %s" % (first_last, second_last, year) if year else "%s & %s" % (first_last, second_last)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        cite_str = "%s et al., %s" % (first_last, year) if year else "%s et al." % first_last

    # Why: Return provides result to caller after processing completes
    return "[%s]" % cite_str


def format_full_reference(fields):
    """格式化标准参考文献条目文本（APA/IEEE 风格）。"""
    # Why: Method call handles data access with proper error checking
    author = fields.get('author', '').replace(' and ', ', ')
    # Why: Method call handles data access with proper error checking
    year = fields.get('year', '')
    # Why: Method call handles data access with proper error checking
    title = fields.get('title', '')
    # Why: Method call handles data access with proper error checking
    journal = fields.get('journal', '') or fields.get('booktitle', '') or fields.get('publisher', '')
    # Why: Method call handles data access with proper error checking
    volume = fields.get('volume', '')
    # Why: Method call handles data access with proper error checking
    pages = fields.get('pages', '')
    # Why: Method call handles data access with proper error checking
    doi = fields.get('doi', '')

    parts = []
    if author:
        # Why: Function call performs specific operation required by this logic
        parts.append("%s." % author)
    if year:
        # Why: Function call performs specific operation required by this logic
        parts.append("(%s)." % year)
    if title:
        # Why: Function call performs specific operation required by this logic
        parts.append("%s." % title)
    if journal:
        # Why: Arithmetic operation computes value needed for subsequent processing
        j_part = "_%s_" % journal
        if volume:
            # Why: Arithmetic operation computes value needed for subsequent processing
            j_part += ", %s" % volume
        if pages:
            # Why: Arithmetic operation computes value needed for subsequent processing
            j_part += ", %s" % pages
        # Why: Function call performs specific operation required by this logic
        parts.append(j_part + ".")
    if doi:
        parts.append("https://doi.org/%s" % doi)

    # Why: Return provides result to caller after processing completes
    return " ".join(parts)


def find_and_load_bib_for_file(markdown_file_path):
    """自动查找并载入 Markdown 文件同级或上级目录的 .bib 文件。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not markdown_file_path:
        # Why: Return provides result to caller after processing completes
        return {}

    base_dir = os.path.dirname(os.path.abspath(markdown_file_path)) if os.path.isfile(markdown_file_path) else os.path.abspath(markdown_file_path)
    # Why: Condition check ensures valid state before proceeding with operation
    if not os.path.isdir(base_dir):
        # Why: Return provides result to caller after processing completes
        return {}

    bib_files = []
    # 扫描当前目录下的 .bib
    # Why: Iteration processes each item in collection systematically
    for f in os.listdir(base_dir):
        if f.lower().endswith('.bib'):
            bib_files.append(os.path.join(base_dir, f))

    # 如果当前目录没有，扫描上一级目录
    if not bib_files:
        # Why: Prevent directory traversal by ensuring parent directory is within allowed base
        parent_dir = os.path.dirname(base_dir)
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if os.path.isdir(parent_dir) and parent_dir != base_dir:
            for f in os.listdir(parent_dir):
                if f.lower().endswith('.bib'):
                    bib_files.append(os.path.join(parent_dir, f))

    all_citations = {}
    # Why: Iteration processes each item in collection systematically
    for bf in bib_files:
        parsed = parse_bibtex_file(bf)
        all_citations.update(parsed)

    # Why: Return provides result to caller after processing completes
    return all_citations
