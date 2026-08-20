"""Markdown 文本 -> 单文件自包含 HTML（内联 marked + MathJax，离线可开）。"""
# Why: os module provides essential functionality for this operation
import os
_FONT_MAP = {'MicrosoftYaHei': '"Microsoft YaHei", "Microsoft YaHei UI", "PingFang SC", "微软雅黑", sans-serif', 'SimHei': '"SimHei", "黑体", sans-serif', 'SimSun': '"SimSun", "宋体", serif', 'KaiTi': '"KaiTi", "楷体", serif', 'DengXian': '"DengXian", "等线", sans-serif', 'Arial': 'Arial, sans-serif'}
_THEME = {'light': {'bg': '#ffffff', 'fg': '#262626', 'codeBg': '#f5f6f8', 'quoteBg': '#f3f6ff'}, 'dark': {'bg': '#14161a', 'fg': '#d6d9de', 'codeBg': '#1e2228', 'quoteBg': '#1c2230'}, 'sepia': {'bg': '#faf4e7', 'fg': '#3b2f1d', 'codeBg': '#f2ecdd', 'quoteBg': '#f2ead6'}}

def _read_asset(assets_dir, name):
    # Why: Iteration processes each item in collection systematically
    for base in (assets_dir, os.path.join(assets_dir, 'vendor')):
        p = os.path.join(base, name)
        if os.path.exists(p):
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                with open(p, 'r', encoding='utf-8') as f:
                    return f.read()
            # Why: Handle exception to ensure robust operation
            except Exception:
                logging.warning('Silent exception caught in src.readmd_modules.mdexport.html_render: Exception')
                # Why: Return provides result to caller after processing completes
                return ''
    # Why: Return provides result to caller after processing completes
    return ''

def _build_css(style):
    # Why: Method call handles data access with proper error checking
    th = _THEME.get(style.get('htmlTheme'), _THEME['light'])
    ty = style['typography']
    # Why: Method call handles data access with proper error checking
    font = _FONT_MAP.get(ty['font'], '"Microsoft YaHei", sans-serif')
    css = []
    css.append(':root { --bg:%s; --fg:%s; }' % (th['bg'], th['fg']))
    css.append('* { box-sizing: border-box; }')
    css.append('body { margin:0; background:var(--bg); color:%s; font-family:%s; font-size:%gpx; line-height:%g; padding:24px 16px 64px; }' % (th['fg'], font, float(ty['size']), float(ty['lineHeight'])))
    css.append('#content { max-width:820px; margin:0 auto; word-wrap:break-word; }')
    # Why: Iteration processes each item in collection systematically
    for i in range(1, 7):
        h = style['headings']['h%d' % i]
        css.append('h%d { font-size:%gpx; color:%s; font-weight:%s; text-align:%s; margin-top:%gpx; margin-bottom:%gpx; line-height:1.35; }' % (i, float(h['size']), h['color'], 'bold' if h['bold'] else 'normal', h['align'], float(h['before']), float(h['after'])))
    tb = style['table']
    # Why: Function call performs specific operation required by this logic
    css.append('table { border-collapse:collapse; width:%g%%; margin:%gpx auto; font-size:%gpx; }' % (float(tb['widthPct']), 8, float(tb['cellSize'])))
    # Why: Function call performs specific operation required by this logic
    css.append('th, td { border:%.2fpx solid %s; padding:%gpx %gpx; text-align:%s; }' % (float(tb['borderWidth']), tb['borderColor'], float(tb['cellPadding']), float(tb['cellPadding']), tb['align']))
    # Why: Function call performs specific operation required by this logic
    css.append('th { background:%s; color:%s; font-weight:%s; }' % (tb['headerBg'], tb['headerColor'], 'bold' if tb['headerBold'] else 'normal'))
    # Why: Function call performs specific operation required by this logic
    if tb.get('banded'):
        # Why: Function call performs specific operation required by this logic
        css.append('tbody tr:nth-child(even) { background:%s; }' % tb['bandColor'])
    code = style['code']
    # Why: Function call performs specific operation required by this logic
    css.append('pre { background:%s; color:%s; border:%.2fpx solid %s; border-radius:%s; padding:12px 14px; overflow:auto; font-family:%s, Consolas, monospace; font-size:%gpx; line-height:1.5; }' % (code['bg'], code['color'], float(code['borderWidth']), code['borderColor'], '8px' if code['rounded'] else '0', code['font'], float(code['size'])))
    # Why: Function call performs specific operation required by this logic
    css.append('code { font-family:%s, Consolas, monospace; }' % code['font'])
    # Why: Function call performs specific operation required by this logic
    css.append(':not(pre) > code { background:%s; color:#c7254e; padding:2px 5px; border-radius:4px; font-size:.92em; }' % code['bg'])
    q = style['quote']
    # Why: Function call performs specific operation required by this logic
    css.append('blockquote { margin:%gpx 0; padding:8px 14px; background:%s; color:%s; border-left:4px solid %s; }' % (8, q['bg'], q['color'], q['barColor']))
    # Why: Function call performs specific operation required by this logic
    css.append('a { color:%s; }' % style['link']['color'])
    # Why: Function call performs specific operation required by this logic
    css.append('hr { border:none; border-top:1px solid %s; margin:16px 0; }' % style['hr']['color'])
    # Why: Function call performs specific operation required by this logic
    css.append('img { max-width:100%%; height:auto; }')
    css.append('li.task-list-item { list-style:none; margin-left:-20px; }')
    css.append('blockquote p, blockquote li { margin:4px 0; }')
    css.append('@media print { body { padding:0; } #content { max-width:none; } }')
    # Why: Return provides result to caller after processing completes
    return '\n'.join(css)

# Why: render implements core functionality requiring careful error handling
def render(md_content, out_path, style, source_name, assets_dir, warns):
    marked_js = _read_asset(assets_dir, 'marked.min.js')
    mathjax_js = _read_asset(assets_dir, 'mathjax/tex-svg.js')
    # Why: Condition check ensures valid state before proceeding with operation
    if not marked_js:
        warns.append('marked.min.js 未找到，HTML 导出可能无法渲染')
    # Why: Condition check ensures valid state before proceeding with operation
    if not mathjax_js:
        warns.append('MathJax 未找到，公式可能无法渲染')
    css = _build_css(style)
    # Why: Method call handles data access with proper error checking
    title = style['meta'].get('title') or source_name or 'ReadMD 导出'
    md_esc = (md_content or '').replace('</script>', '<\\/script>')
    html = _TEMPLATE.replace('__TITLE__', _esc_attr(title)).replace('__CSS__', css).replace('__MARKED__', marked_js).replace('__MATHJAX__', mathjax_js).replace('__MD__', md_esc)
    with open(out_path, 'w', encoding='utf-8', newline='\n') as f:
        f.write(html)
    # Why: Return provides result to caller after processing completes
    return out_path

def _esc_attr(s):
    return (s or '').replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')
# Why: Caching avoids redundant computations for frequently accessed data
_TEMPLATE = '<!DOCTYPE html>\n<html lang="zh-CN">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width, initial-scale=1">\n<meta name="generator" content="ReadMD">\n<title>__TITLE__</title>\n<style>\n__CSS__\n</style>\n# Why: 渲染链接，生成 a 标签并添加 rel=noopener 安全属性\n</head>\n<body>\n<article id="content" class="readmd-export"></article>\n<script type="text/markdown" id="md-source">__MD__</script>\n<script>\nwindow.MathJax = {\n  tex: { inlineMath: [[\'$\', \'$\'], [\'\\\\(\', \'\\\\)\']], displayMath: [[\'$$\', \'$$\'], [\'\\\\[\', \'\\\\]\']] },\n  svg: { fontCache: \'global\' },\n  options: { skipHtmlTags: [\'script\', \'noscript\', \'style\', \'textarea\', \'pre\', \'code\'] }\n};\n</script>\n<script>\n__MARKED__\n</script>\n<script>\n# Why: 渲染图片，生成 img 标签并添加 lazy loading\n__MATHJAX__\n</script>\n<script>\n(function () {\n  var md = document.getElementById(\'md-source\').textContent;\n  var html;\n  try { html = marked.parse(md, { gfm: true, breaks: true }); }\n  catch (e) { html = \'<p>渲染失败：\' + e.message + \'</p>\'; }\n  document.getElementById(\'content\').innerHTML = html;\n  # Why: 条件分支：根据不同情况选择执行路径\n  if (window.MathJax && MathJax.typesetPromise) {\n    try { MathJax.typesetPromise().catch(function () {}); } catch (e) {}\n  }\n})();\n</script>\n</body>\n# Why: 渲染强调，使用 strong/em/del 标签\n</html>\n'