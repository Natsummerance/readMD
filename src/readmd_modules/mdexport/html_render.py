# -*- coding: utf-8 -*-
"""Markdown 文本 -> 单文件自包含 HTML（内联 marked + MathJax，离线可开）。"""

import os
import json

_FONT_MAP = {
    'MicrosoftYaHei': '"Microsoft YaHei", "Microsoft YaHei UI", "PingFang SC", "微软雅黑", sans-serif',
    'SimHei': '"SimHei", "黑体", sans-serif',
    'SimSun': '"SimSun", "宋体", serif',
    'KaiTi': '"KaiTi", "楷体", serif',
    'DengXian': '"DengXian", "等线", sans-serif',
    'Arial': 'Arial, sans-serif',
}
_THEME = {
    'light': {'bg': '#ffffff', 'fg': '#262626', 'codeBg': '#f5f6f8', 'quoteBg': '#f3f6ff'},
    'dark': {'bg': '#14161a', 'fg': '#d6d9de', 'codeBg': '#1e2228', 'quoteBg': '#1c2230'},
    'sepia': {'bg': '#faf4e7', 'fg': '#3b2f1d', 'codeBg': '#f2ecdd', 'quoteBg': '#f2ead6'},
}


def _read_asset(assets_dir, name):
    for base in (assets_dir, os.path.join(assets_dir, 'vendor')):
        p = os.path.join(base, name)
        if os.path.exists(p):
            try:
                with open(p, 'r', encoding='utf-8') as f:
                    return f.read()
            except Exception:
                return ''
    return ''


def _build_css(style):
    th = _THEME.get(style.get('htmlTheme'), _THEME['light'])
    ty = style['typography']
    font = _FONT_MAP.get(ty['font'], '"Microsoft YaHei", sans-serif')
    css = []
    css.append(':root { --bg:%s; --fg:%s; }' % (th['bg'], th['fg']))
    css.append('* { box-sizing: border-box; }')
    css.append('body { margin:0; background:var(--bg); color:%s; font-family:%s; '
               'font-size:%gpx; line-height:%g; padding:24px 16px 64px; }' % (
                   th['fg'], font, float(ty['size']), float(ty['lineHeight'])))
    css.append('#content { max-width:820px; margin:0 auto; word-wrap:break-word; }')
    for i in range(1, 7):
        h = style['headings']['h%d' % i]
        css.append('h%d { font-size:%gpt; color:%s; font-weight:%s; text-align:%s; '
                   'margin-top:%gpt; margin-bottom:%gpt; line-height:1.35; }' % (
                       i, float(h['size']), h['color'], 'bold' if h['bold'] else 'normal',
                       h['align'], float(h['before']), float(h['after'])))
    tb = style['table']
    css.append('table { border-collapse:collapse; width:%g%%; margin:%gpx auto; font-size:%gpt; }' % (
        float(tb['widthPct']), 8, float(tb['cellSize'])))
    css.append('th, td { border:%.2fpx solid %s; padding:%gpx %gpx; text-align:%s; }' % (
        float(tb['borderWidth']), tb['borderColor'], float(tb['cellPadding']), float(tb['cellPadding']),
        tb['align']))
    css.append('th { background:%s; color:%s; font-weight:%s; }' % (
        tb['headerBg'], tb['headerColor'], 'bold' if tb['headerBold'] else 'normal'))
    if tb.get('banded'):
        css.append('tbody tr:nth-child(even) { background:%s; }' % tb['bandColor'])
    code = style['code']
    css.append('pre { background:%s; color:%s; border:%.2fpx solid %s; border-radius:%s; '
               'padding:12px 14px; overflow:auto; font-family:%s, Consolas, monospace; '
               'font-size:%gpt; line-height:1.5; }' % (
                   code['bg'], code['color'], float(code['borderWidth']), code['borderColor'],
                   '8px' if code['rounded'] else '0', code['font'], float(code['size'])))
    css.append('code { font-family:%s, Consolas, monospace; }' % code['font'])
    css.append(':not(pre) > code { background:%s; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }' % code['bg'])
    q = style['quote']
    css.append('blockquote { margin:%gpx 0; padding:8px 14px; background:%s; color:%s; '
               'border-left:4px solid %s; }' % (8, q['bg'], q['color'], q['barColor']))
    css.append('a { color:%s; }' % style['link']['color'])
    css.append('hr { border:none; border-top:1px solid %s; margin:16px 0; }' % style['hr']['color'])
    css.append('img { max-width:100%; height:auto; }')
    css.append('p > img:only-child { display:block; max-width:%g%%; margin:0 auto; object-fit:contain; }' % style['images']['widthPct'])
    css.append('li.task-list-item { list-style:none; margin-left:-20px; }')
    css.append('blockquote p, blockquote li { margin:4px 0; }')
    from .styles import page_dimensions
    width, height = page_dimensions(style)
    page = style['page']
    css.append('@page { size:%gmm %gmm; margin:%gmm %gmm %gmm %gmm; }' % (width, height, page['marginTop'], page['marginRight'], page['marginBottom'], page['marginLeft']))
    css.append('body { font-size:%gpt; } p { text-align:%s; text-indent:%gmm; margin-bottom:%gpt; }' % (ty['size'], ty['align'], ty.get('firstLineIndent', 0), ty['spacing']))
    css.append('h1,h2,h3,h4,h5,h6 { break-after:avoid; text-indent:0; } thead { display:table-header-group; } pre { white-space:pre-wrap; overflow-wrap:anywhere; }')
    for i in range(1, 7):
        if style['headings']['h%d' % i].get('pageBreakBefore'): css.append('h%d { break-before:page; }' % i)
    css.append('@media print { body { padding:0; } #content { max-width:none; } p { orphans:3; widows:3; } }')
    css.append('.readmd-pagebreak { break-after:page; page-break-after:always; }')
    return '\n'.join(css)


def render(md_content, out_path, style, source_name, assets_dir, warns):
    marked_js = _read_asset(assets_dir, 'marked.min.js')
    mathjax_js = _read_asset(assets_dir, 'mathjax/tex-svg.js')
    if not marked_js:
        warns.append('marked.min.js 未找到，HTML 导出可能无法渲染')
    if not mathjax_js:
        warns.append('MathJax 未找到，公式可能无法渲染')
    css = _build_css(style)
    title = (style['meta'].get('title') or source_name or 'ReadMD 导出')
    md_esc = json.dumps(md_content or '', ensure_ascii=False).replace('<', '\\u003c')
    html = _TEMPLATE.replace('__TITLE__', _esc_attr(title)) \
                   .replace('__CSS__', css) \
                   .replace('__MARKED__', marked_js) \
                   .replace('__MATHJAX__', mathjax_js) \
                   .replace('__MD__', md_esc)
    with open(out_path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(html)
    return out_path


def _esc_attr(s):
    return (s or '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')


_TEMPLATE = """<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="ReadMD">
<title>__TITLE__</title>
<style>
__CSS__
</style>
</head>
<body>
<article id="content" class="readmd-export"></article>
<script type="application/json" id="md-source">__MD__</script>
<script>
window.MathJax = {
  tex: { inlineMath: [['$', '$'], ['\\\\(', '\\\\)']], displayMath: [['$$', '$$'], ['\\\\[', '\\\\]']] },
  svg: { fontCache: 'global' },
  options: { skipHtmlTags: ['script', 'noscript', 'style', 'textarea', 'pre', 'code'] }
};
</script>
<script>
__MARKED__
</script>
<script>
__MATHJAX__
</script>
<script>
(function () {
  var md = JSON.parse(document.getElementById('md-source').textContent);
  var html;
  try { html = marked.parse(md, { gfm: true, breaks: true }); }
  catch (e) { html = '<p>渲染失败：' + e.message + '</p>'; }
  document.getElementById('content').innerHTML = html;
  var root = document.getElementById('content');
  var walker = document.createTreeWalker(root, NodeFilter.SHOW_COMMENT);
  var breaks = [], node;
  while ((node = walker.nextNode())) {
    if (/^page-?break$/.test(node.textContent.trim())) breaks.push(node);
  }
  root.querySelectorAll('p').forEach(function (p) {
    if (p.textContent.trim() === '\\\\newpage') breaks.push(p);
  });
  breaks.forEach(function (node) {
    var div = document.createElement('div');
    div.className = 'readmd-pagebreak'; node.replaceWith(div);
  });
  if (window.MathJax && MathJax.typesetPromise) {
    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}
  }
})();
</script>
</body>
</html>
"""
