# -*- coding: utf-8 -*-
"""ReadMD 用户自定义样式 (custom.css) 与 HTML Head (head.html) 注入器。

查找优先级：
1. 当前工作区 / 文档所在目录下的 `.readmd/custom.css` 与 `.readmd/head.html`
2. 用户主目录 `~/.readmd/custom.css` 与 `~/.readmd/head.html`
3. 自定义配置指定的路径
"""

import os
from typing import Optional, Tuple


def find_custom_file(filename: str, workspace_dir: Optional[str] = None) -> Optional[str]:
    """按优先级探测自定义配置文件的物理绝对路径。"""
    # 1. 探测工作区 .readmd/
    if workspace_dir:
        ws_path = os.path.join(workspace_dir, '.readmd', filename)
        if os.path.isfile(ws_path):
            return ws_path

    # 2. 探测用户家目录 ~/.readmd/
    home_dir = os.path.expanduser('~')
    home_path = os.path.join(home_dir, '.readmd', filename)
    if os.path.isfile(home_path):
        return home_path

    return None


def get_custom_css(workspace_dir: Optional[str] = None) -> str:
    """读取用户自定义 CSS 样式内容。"""
    path = find_custom_file('custom.css', workspace_dir=workspace_dir)
    if path and os.path.isfile(path):
        try:
            with open(path, 'r', encoding='utf-8', errors='replace') as f:
                return f.read().strip()
        except Exception:
            return ""
    return ""


def get_custom_head(workspace_dir: Optional[str] = None) -> str:
    """读取用户自定义 HTML Head 片段内容。"""
    path = find_custom_file('head.html', workspace_dir=workspace_dir)
    if path and os.path.isfile(path):
        try:
            with open(path, 'r', encoding='utf-8', errors='replace') as f:
                return f.read().strip()
        except Exception:
            return ""
    return ""


def inject_custom_styles_to_html(html_content: str, workspace_dir: Optional[str] = None) -> str:
    """将 custom.css 与 head.html 注入到 HTML 字符串的 <head> 区域。"""
    css = get_custom_css(workspace_dir=workspace_dir)
    head_html = get_custom_head(workspace_dir=workspace_dir)

    injections = []
    if css:
        injections.append(f'<style id="readmd-custom-style">\n{css}\n</style>')
    if head_html:
        injections.append(f'<!-- ReadMD Custom Head -->\n{head_html}')

    if not injections:
        return html_content

    injection_str = "\n".join(injections)

    if '</head>' in html_content:
        return html_content.replace('</head>', f'{injection_str}\n</head>', 1)
    else:
        return f'{injection_str}\n{html_content}'


def get_custom_styles(workspace_dir: Optional[str] = None) -> dict:
    """获取自定义 CSS 和 Head 结构。"""
    return {
        'css': get_custom_css(workspace_dir=workspace_dir),
        'head': get_custom_head(workspace_dir=workspace_dir),
        'css_path': find_custom_file('custom.css', workspace_dir=workspace_dir) or '',
        'head_path': find_custom_file('head.html', workspace_dir=workspace_dir) or ''
    }


def save_custom_styles(css: str, head_html: str, workspace_dir: Optional[str] = None) -> bool:
    """保存自定义 CSS 和 Head。"""
    target_dir = os.path.join(workspace_dir, '.readmd') if workspace_dir else os.path.join(os.path.expanduser('~'), '.readmd')
    try:
        os.makedirs(target_dir, exist_ok=True)
        with open(os.path.join(target_dir, 'custom.css'), 'w', encoding='utf-8') as f:
            f.write(css or '')
        with open(os.path.join(target_dir, 'head.html'), 'w', encoding='utf-8') as f:
            f.write(head_html or '')
        return True
    except Exception:
        return False

