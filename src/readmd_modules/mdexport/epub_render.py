# -*- coding: utf-8 -*-
"""ReadMD EPUB 3.0 原生电子书打包导出引擎 (纯标准库，零外部依赖)。

生成标准 IDPF EPUB 3.2 OCF ZIP 容器：
- `mimetype` (无压缩存储)
- `META-INF/container.xml`
- `OEBPS/content.opf` (元数据、资源清单与阅读脊柱)
- `OEBPS/nav.xhtml` (EPUB 3 语义导航)
- `OEBPS/toc.ncx` (EPUB 2 兼容导航)
- `OEBPS/style.css` (精美电子书排版样式)
- `OEBPS/chapter_*.xhtml` (章节内容 XHTML)
"""

import datetime
import html
import io
import os
import re
import uuid
import zipfile
from typing import Dict, List, Optional, Tuple

EPUB_CSS = """
@charset "utf-8";
body {
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", serif;
    margin: 5% 8%;
    line-height: 1.8;
    color: #1a1a1a;
}
h1, h2, h3, h4, h5, h6 {
    font-family: -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", sans-serif;
    font-weight: 600;
    line-height: 1.4;
    color: #0f172a;
    page-break-after: avoid;
}
h1 { font-size: 1.8em; margin-top: 1.5em; border-bottom: 1px solid #e2e8f0; padding-bottom: 0.3em; }
h2 { font-size: 1.4em; margin-top: 1.3em; }
p { margin: 0.8em 0; text-align: justify; }
pre, code {
    font-family: "Courier New", Courier, monospace;
    background-color: #f1f5f9;
    font-size: 0.9em;
}
pre {
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
    border: 1px solid #e2e8f0;
}
blockquote {
    margin: 1em 0;
    padding-left: 1em;
    border-left: 4px solid #3b82f6;
    color: #475569;
}
table {
    width: 100%;
    border-collapse: collapse;
    margin: 1.2em 0;
}
th, td {
    border: 1px solid #cbd5e1;
    padding: 8px 10px;
    text-align: left;
}
th { background-color: #f8fafc; }
"""


def _simple_md_to_html(md_text: str) -> str:
    """轻量级纯标准库 Markdown 转 XHTML 段落。"""
    lines = md_text.splitlines()
    html_out = []
    in_code = False
    code_lines = []
    in_list = False

    for line in lines:
        stripped = line.strip()

        # 代码块
        if stripped.startswith('```'):
            if in_code:
                in_code = False
                escaped_code = html.escape("\n".join(code_lines))
                html_out.append(f"<pre><code>{escaped_code}</code></pre>")
                code_lines = []
            else:
                in_code = True
                code_lines = []
            continue

        if in_code:
            code_lines.append(line)
            continue

        if not stripped:
            if in_list:
                html_out.append("</ul>")
                in_list = False
            continue

        # 标题
        if stripped.startswith('#'):
            if in_list:
                html_out.append("</ul>")
                in_list = False
            level = len(line) - len(line.lstrip('#'))
            title_text = html.escape(line.lstrip('#').strip())
            html_out.append(f"<h{level}>{title_text}</h{level}>")
            continue

        # 列表
        if stripped.startswith('- ') or stripped.startswith('* '):
            if not in_list:
                html_out.append("<ul>")
                in_list = True
            item_text = html.escape(stripped[2:].strip())
            html_out.append(f"<li>{item_text}</li>")
            continue

        # 引用
        if stripped.startswith('> '):
            if in_list:
                html_out.append("</ul>")
                in_list = False
            quote_text = html.escape(stripped[2:].strip())
            html_out.append(f"<blockquote><p>{quote_text}</p></blockquote>")
            continue

        # 普通段落
        if in_list:
            html_out.append("</ul>")
            in_list = False

        escaped = html.escape(line)
        # 行内粗体与行内代码简单转译
        escaped = re.sub(r'\*\*(.+?)\*\*', r'<strong>\1</strong>', escaped)
        escaped = re.sub(r'`(.+?)`', r'<code>\1</code>', escaped)
        html_out.append(f"<p>{escaped}</p>")

    if in_list:
        html_out.append("</ul>")

    return "\n".join(html_out)


def split_into_chapters(markdown_content: str) -> List[Tuple[str, str]]:
    """按一级或二级标题切分多章节。"""
    lines = markdown_content.splitlines()
    chapters: List[Tuple[str, List[str]]] = []
    curr_title = "序言"
    curr_lines = []

    for line in lines:
        if line.startswith('# ') or line.startswith('## '):
            if curr_lines:
                chapters.append((curr_title, "\n".join(curr_lines)))
                curr_lines = []
            curr_title = line.lstrip('#').strip()
            curr_lines.append(line)
        else:
            curr_lines.append(line)

    if curr_lines:
        chapters.append((curr_title, "\n".join(curr_lines)))

    return [(t, c) for t, c in chapters if c.strip()]


def export_epub(markdown_content: str, output_path: str,
                title: str = "ReadMD 电子书",
                author: str = "ReadMD Author",
                language: str = "zh-CN") -> str:
    """将 Markdown 编译打包为标准 EPUB 3 电子书文件。"""
    out_dir = os.path.dirname(os.path.abspath(output_path))
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)

    book_uuid = str(uuid.uuid4())
    mod_time = datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")

    raw_chapters = split_into_chapters(markdown_content)
    if not raw_chapters:
        raw_chapters = [(title, markdown_content)]

    # 准备各章节 XHTML
    chapter_files = []
    for idx, (chap_title, chap_md) in enumerate(raw_chapters):
        chap_id = f"chap_{idx+1}"
        chap_filename = f"chapter_{idx+1}.xhtml"
        chap_body = _simple_md_to_html(chap_md)
        chap_xhtml = f"""<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}" lang="{language}">
<head>
    <meta charset="utf-8"/>
    <title>{html.escape(chap_title)}</title>
    <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
    {chap_body}
</body>
</html>"""
        chapter_files.append((chap_id, chap_filename, chap_title, chap_xhtml))

    # 1. 构造 nav.xhtml
    nav_items = "\n".join([
        f'        <li><a href="{fn}">{html.escape(t)}</a></li>'
        for _, fn, t, _ in chapter_files
    ])
    nav_xhtml = f"""<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{language}" lang="{language}">
<head>
    <meta charset="utf-8"/>
    <title>目录</title>
    <link rel="stylesheet" type="text/css" href="style.css"/>
</head>
<body>
    <nav epub:type="toc" id="toc">
        <h1>目录</h1>
        <ol>
{nav_items}
        </ol>
    </nav>
</body>
</html>"""

    # 2. 构造 toc.ncx (EPUB 2 兼容)
    ncx_points = "\n".join([
        f"""    <navPoint id="navPoint-{i+1}" playOrder="{i+1}">
        <navLabel><text>{html.escape(t)}</text></navLabel>
        <content src="{fn}"/>
    </navPoint>"""
        for i, (_, fn, t, _) in enumerate(chapter_files)
    ])
    toc_ncx = f"""<?xml version="1.0" encoding="utf-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
    <head>
        <meta name="dtb:uid" content="urn:uuid:{book_uuid}"/>
        <meta name="dtb:depth" content="1"/>
        <meta name="dtb:totalPageCount" content="0"/>
        <meta name="dtb:maxPageNumber" content="0"/>
    </head>
    <docTitle><text>{html.escape(title)}</text></docTitle>
    <navMap>
{ncx_points}
    </navMap>
</ncx>"""

    # 3. 构造 content.opf
    manifest_items = [
        '<item id="style" href="style.css" media-type="text/css"/>',
        '<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>',
        '<item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>'
    ]
    for cid, fn, _, _ in chapter_files:
        manifest_items.append(f'<item id="{cid}" href="{fn}" media-type="application/xhtml+xml"/>')

    spine_items = ['<itemref idref="nav"/>']
    for cid, _, _, _ in chapter_files:
        spine_items.append(f'<itemref idref="{cid}"/>')

    content_opf = f"""<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="BookId" version="3.0">
    <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
        <dc:identifier id="BookId">urn:uuid:{book_uuid}</dc:identifier>
        <dc:title>{html.escape(title)}</dc:title>
        <dc:creator>{html.escape(author)}</dc:creator>
        <dc:language>{language}</dc:language>
        <meta property="dcterms:modified">{mod_time}</meta>
    </metadata>
    <manifest>
        {chr(10).join(manifest_items)}
    </manifest>
    <spine toc="ncx">
        {chr(10).join(spine_items)}
    </spine>
</package>"""

    # 4. 写入标准 OCF ZIP 容器
    with zipfile.ZipFile(output_path, 'w', compression=zipfile.ZIP_DEFLATED) as zf:
        # mimetype 必须是第一个文件且无压缩
        zf.writestr('mimetype', 'application/epub+zip', compress_type=zipfile.ZIP_STORED)

        # META-INF/container.xml
        container_xml = """<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
    <rootfiles>
        <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
    </rootfiles>
</container>"""
        zf.writestr('META-INF/container.xml', container_xml)

        # OEBPS 资源
        zf.writestr('OEBPS/content.opf', content_opf)
        zf.writestr('OEBPS/nav.xhtml', nav_xhtml)
        zf.writestr('OEBPS/toc.ncx', toc_ncx)
        zf.writestr('OEBPS/style.css', EPUB_CSS)

        for _, fn, _, xhtml_content in chapter_files:
            zf.writestr(f'OEBPS/{fn}', xhtml_content)

    return output_path
