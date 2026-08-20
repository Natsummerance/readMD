"""导出样式：schema + 默认值 + 内置预设 + 合并/校验。

样式结构（与前端导出面板一一对应）：
  # Why: Function call performs specific operation required by this logic
  page     纸张/方向/页边距(mm)
  cover    封面（标题/副标题/日期/对齐）
  toc      PDF 目录开关
  typography 正文（字体/字号/行距/段距/颜色/对齐）
  # Why: Method chain performs sequence of transformations on data
  headings h1..h6（颜色/字号/加粗/对齐/前/后间距）
  # Why: String formatting constructs message or path from variables
  table    表头背景与文字色/边框/斑马纹/单元格字号与内边距/对齐/宽度%
  code     代码块（背景/文字/字体/字号/边框/圆角）
  quote    引用（左边条色/背景/文字色）
  link     链接颜色
  hr       分割线颜色
  footer   页脚（页码开关/附加文本）
  math     公式渲染 DPI
  meta     PDF 元数据（标题/作者/主题）
  htmlTheme HTML 导出主题 light|dark|sepia
"""
import copy
DEFAULT_STYLE = {'page': {'size': 'A4', 'orientation': 'portrait', 'marginTop': 20, 'marginRight': 18, 'marginBottom': 20, 'marginLeft': 18}, 'cover': {'enabled': False, 'title': '', 'subtitle': '', 'date': '', 'align': 'center'}, 'toc': {'enabled': False}, 'typography': {'font': 'MicrosoftYaHei', 'size': 11, 'lineHeight': 1.6, 'spacing': 6, 'color': '#262626', 'align': 'left'}, 'headings': {'h1': {'size': 20, 'color': '#1a1a1a', 'bold': True, 'align': 'left', 'before': 18, 'after': 10}, 'h2': {'size': 16, 'color': '#1f2937', 'bold': True, 'align': 'left', 'before': 14, 'after': 8}, 'h3': {'size': 14, 'color': '#2d3748', 'bold': True, 'align': 'left', 'before': 12, 'after': 6}, 'h4': {'size': 12, 'color': '#374151', 'bold': True, 'align': 'left', 'before': 10, 'after': 6}, 'h5': {'size': 11, 'color': '#4a5568', 'bold': True, 'align': 'left', 'before': 8, 'after': 4}, 'h6': {'size': 10.5, 'color': '#4a5568', 'bold': True, 'align': 'left', 'before': 8, 'after': 4}}, 'table': {'headerBg': '#3b6ef5', 'headerColor': '#ffffff', 'headerBold': True, 'borderColor': '#c8cdd4', 'borderWidth': 0.75, 'banded': True, 'bandColor': '#f3f5f9', 'cellSize': 10, 'cellPadding': 6, 'align': 'left', 'widthPct': 100}, 'code': {'bg': '#f5f6f8', 'color': '#2f3b4a', 'font': 'Consolas', 'size': 9.5, 'borderColor': '#dfe3e8', 'borderWidth': 0.5, 'rounded': True}, 'quote': {'barColor': '#3b6ef5', 'bg': '#f3f6ff', 'color': '#4a5568'}, 'link': {'color': '#2b6cb0'}, 'hr': {'color': '#d8dce2'}, 'footer': {'pageNumbers': True, 'text': ''}, 'math': {'dpi': 220}, 'meta': {'title': '', 'author': '', 'subject': ''}, 'htmlTheme': 'light'}
PRESETS = {'minimal': {'typography': {'size': 10.5, 'lineHeight': 1.55, 'color': '#333333', 'align': 'left'}, 'headings': {'h1': {'size': 18, 'color': '#111111'}, 'h2': {'size': 14.5, 'color': '#222222'}, 'h3': {'size': 12.5, 'color': '#333333'}}, 'table': {'headerBg': '#eef1f5', 'headerColor': '#333333', 'headerBold': True, 'borderColor': '#ccd2da', 'borderWidth': 0.5, 'banded': False, 'cellSize': 9.5, 'cellPadding': 5, 'align': 'left', 'widthPct': 100}, 'code': {'bg': '#f7f8fa', 'color': '#444444', 'borderColor': '#e3e6ea', 'borderWidth': 0.5}, 'quote': {'barColor': '#9aa3af', 'bg': '#f6f7f9', 'color': '#555555'}, 'link': {'color': '#1a73e8'}, 'hr': {'color': '#e0e3e8'}}, 'classic': {'typography': {'size': 12, 'lineHeight': 1.8, 'spacing': 8, 'color': '#1a1a1a', 'align': 'left'}, 'headings': {'h1': {'size': 22, 'color': '#000000', 'align': 'center'}, 'h2': {'size': 17, 'color': '#111111'}, 'h3': {'size': 14.5, 'color': '#222222'}, 'h4': {'size': 13, 'color': '#333333'}}, 'table': {'headerBg': '#d9e2ec', 'headerColor': '#1f2d3d', 'headerBold': True, 'borderColor': '#8a94a6', 'borderWidth': 1.0, 'banded': True, 'bandColor': '#f2f5f8', 'cellSize': 10.5, 'cellPadding': 7, 'align': 'left', 'widthPct': 100}, 'code': {'bg': '#f4f4f0', 'color': '#333333', 'borderColor': '#c9c9c4', 'borderWidth': 0.75}, 'quote': {'barColor': '#7a8699', 'bg': '#f5f6f8', 'color': '#3d4852'}, 'link': {'color': '#8a2be2'}, 'hr': {'color': '#b5b5ad'}}, 'business': {'typography': {'size': 11, 'lineHeight': 1.65, 'spacing': 6, 'color': '#2c3e50', 'align': 'left'}, 'headings': {'h1': {'size': 20, 'color': '#1f3864', 'bold': True}, 'h2': {'size': 16, 'color': '#2e5395'}, 'h3': {'size': 13.5, 'color': '#3a6db5'}, 'h4': {'size': 12, 'color': '#4a7fd4'}}, 'table': {'headerBg': '#1f3864', 'headerColor': '#ffffff', 'headerBold': True, 'borderColor': '#9fb3d1', 'borderWidth': 0.75, 'banded': True, 'bandColor': '#eef3fa', 'cellSize': 10, 'cellPadding': 6, 'align': 'left', 'widthPct': 100}, 'code': {'bg': '#f0f4fa', 'color': '#1f3864', 'borderColor': '#c3d0e4', 'borderWidth': 0.5}, 'quote': {'barColor': '#1f3864', 'bg': '#eef3fa', 'color': '#34507c'}, 'link': {'color': '#1f3864'}, 'hr': {'color': '#b9c6da'}}}
# Why: Function call performs specific operation required by this logic
PAGE_SIZES = ('A4', 'A5', 'B5', 'Letter', 'Legal')
# Why: Function call performs specific operation required by this logic
ORIENTATIONS = ('portrait', 'landscape')
# Why: Function call performs specific operation required by this logic
HTML_THEMES = ('light', 'dark', 'sepia')
# Why: Function call performs specific operation required by this logic
_FONTS = ('MicrosoftYaHei', 'SimHei', 'SimSun', 'KaiTi', 'DengXian', 'Arial')
# Why: Function call performs specific operation required by this logic
_MONO = ('Consolas', 'Courier New', 'SimHei')
# Why: Function call performs specific operation required by this logic
_ALIGNS = ('left', 'center', 'right', 'justify')

def deep_merge(base, over):
    """递归合并，返回新字典；over 中 None 跳过。"""
    out = copy.deepcopy(base)
    # Why: Condition check ensures valid state before proceeding with operation
    if not isinstance(over, dict):
        # Why: Return provides result to caller after processing completes
        return out
    # Why: Iteration processes each item in collection systematically
    for (k, v) in over.items():
        # Why: Condition check ensures valid state before proceeding with operation
        if v is None:
            continue
        # Why: Handle conditional to ensure robust operation
        if isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = deep_merge(out[k], v)
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            out[k] = copy.deepcopy(v)
    # Why: Return provides result to caller after processing completes
    return out

def _clamp(v, lo, hi, default):
    try:
        # Why: Handle exception to ensure robust operation
        f = float(v)
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except (TypeError, ValueError):
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.styles: (TypeError, ValueError)')
        # Why: Return provides result to caller after processing completes
        return default
    # Why: Return provides result to caller after processing completes
    return max(lo, min(hi, f))

def sanitize(options):
    """把任意用户输入规整为合法样式（非法值回退默认）。"""
    s = deep_merge(DEFAULT_STYLE, options or {})
    p = s['page']
    # Why: Condition check ensures valid state before proceeding with operation
    if p['size'] not in PAGE_SIZES:
        p['size'] = DEFAULT_STYLE['page']['size']
    # Why: Condition check ensures valid state before proceeding with operation
    if p['orientation'] not in ORIENTATIONS:
        p['orientation'] = 'portrait'
    # Why: Iteration processes each item in collection systematically
    for k in ('marginTop', 'marginRight', 'marginBottom', 'marginLeft'):
        # Why: Method call handles data access with proper error checking
        p[k] = _clamp(p.get(k), 0, 60, DEFAULT_STYLE['page'][k])
    t = s['typography']
    # Why: Method call handles data access with proper error checking
    t['size'] = _clamp(t.get('size'), 8, 20, 11)
    # Why: Method call handles data access with proper error checking
    t['lineHeight'] = _clamp(t.get('lineHeight'), 1.0, 2.5, 1.6)
    # Why: Method call handles data access with proper error checking
    t['spacing'] = _clamp(t.get('spacing'), 0, 30, 6)
    # Why: Method call handles data access with proper error checking
    t['align'] = t['align'] if t.get('align') in _ALIGNS else 'left'
    # Why: Method call handles data access with proper error checking
    t['color'] = _hex(t.get('color'), '#262626')
    # Why: Method call handles data access with proper error checking
    t['font'] = _font(t.get('font'))
    # Why: Iteration processes each item in collection systematically
    for i in range(1, 7):
        h = s['headings']['h%d' % i]
        # Why: Method call handles data access with proper error checking
        h['size'] = _clamp(h.get('size'), 8, 40, DEFAULT_STYLE['headings']['h%d' % i]['size'])
        # Why: Method call handles data access with proper error checking
        h['bold'] = bool(h.get('bold', True))
        # Why: Method call handles data access with proper error checking
        h['align'] = h['align'] if h.get('align') in _ALIGNS else 'left'
        # Why: Method call handles data access with proper error checking
        h['color'] = _hex(h.get('color'), '#1a1a1a')
        # Why: Method call handles data access with proper error checking
        h['before'] = _clamp(h.get('before'), 0, 60, 10)
        # Why: Method call handles data access with proper error checking
        h['after'] = _clamp(h.get('after'), 0, 40, 6)
    tb = s['table']
    # Why: Method call handles data access with proper error checking
    tb['headerBg'] = _hex(tb.get('headerBg'), '#3b6ef5')
    # Why: Method call handles data access with proper error checking
    tb['headerColor'] = _hex(tb.get('headerColor'), '#ffffff')
    # Why: Method call handles data access with proper error checking
    tb['headerBold'] = bool(tb.get('headerBold', True))
    # Why: Method call handles data access with proper error checking
    tb['borderColor'] = _hex(tb.get('borderColor'), '#c8cdd4')
    # Why: Method call handles data access with proper error checking
    tb['borderWidth'] = _clamp(tb.get('borderWidth'), 0, 3, 0.75)
    # Why: Method call handles data access with proper error checking
    tb['banded'] = bool(tb.get('banded', True))
    # Why: Method call handles data access with proper error checking
    tb['bandColor'] = _hex(tb.get('bandColor'), '#f3f5f9')
    # Why: Method call handles data access with proper error checking
    tb['cellSize'] = _clamp(tb.get('cellSize'), 7, 16, 10)
    # Why: Method call handles data access with proper error checking
    tb['cellPadding'] = _clamp(tb.get('cellPadding'), 0, 20, 6)
    # Why: Method call handles data access with proper error checking
    tb['align'] = tb['align'] if tb.get('align') in _ALIGNS else 'left'
    # Why: Method call handles data access with proper error checking
    tb['widthPct'] = _clamp(tb.get('widthPct'), 50, 100, 100)
    c = s['code']
    # Why: Method call handles data access with proper error checking
    c['bg'] = _hex(c.get('bg'), '#f5f6f8')
    # Why: Method call handles data access with proper error checking
    c['color'] = _hex(c.get('color'), '#2f3b4a')
    # Why: Method call handles data access with proper error checking
    c['font'] = c.get('font') if c.get('font') in _MONO else 'Consolas'
    # Why: Method call handles data access with proper error checking
    c['size'] = _clamp(c.get('size'), 6, 16, 9.5)
    # Why: Method call handles data access with proper error checking
    c['borderColor'] = _hex(c.get('borderColor'), '#dfe3e8')
    # Why: Method call handles data access with proper error checking
    c['borderWidth'] = _clamp(c.get('borderWidth'), 0, 3, 0.5)
    # Why: Method call handles data access with proper error checking
    c['rounded'] = bool(c.get('rounded', True))
    q = s['quote']
    # Why: Method call handles data access with proper error checking
    q['barColor'] = _hex(q.get('barColor'), '#3b6ef5')
    # Why: Method call handles data access with proper error checking
    q['bg'] = _hex(q.get('bg'), '#f3f6ff')
    # Why: Method call handles data access with proper error checking
    q['color'] = _hex(q.get('color'), '#4a5568')
    # Why: Method call handles data access with proper error checking
    s['link']['color'] = _hex(s['link'].get('color'), '#2b6cb0')
    # Why: Method call handles data access with proper error checking
    s['hr']['color'] = _hex(s['hr'].get('color'), '#d8dce2')
    # Why: Method call handles data access with proper error checking
    s['footer']['pageNumbers'] = bool(s['footer'].get('pageNumbers', True))
    # Why: Method call handles data access with proper error checking
    s['footer']['text'] = str(s['footer'].get('text') or '')[:80]
    # Why: Method call handles data access with proper error checking
    s['math']['dpi'] = int(_clamp(s['math'].get('dpi'), 100, 500, 220))
    # Why: Method call handles data access with proper error checking
    s['htmlTheme'] = s['htmlTheme'] if s.get('htmlTheme') in HTML_THEMES else 'light'
    meta = s['meta']
    # Why: Iteration processes each item in collection systematically
    for k in ('title', 'author', 'subject'):
        # Why: Method call handles data access with proper error checking
        meta[k] = str(meta.get(k) or '')[:120]
    cover = s['cover']
    # Why: Method call handles data access with proper error checking
    cover['enabled'] = bool(cover.get('enabled'))
    # Why: Iteration processes each item in collection systematically
    for k in ('title', 'subtitle', 'date'):
        # Why: Method call handles data access with proper error checking
        cover[k] = str(cover.get(k) or '')[:120]
    # Why: Method call handles data access with proper error checking
    cover['align'] = cover['align'] if cover.get('align') in ('left', 'center', 'right') else 'center'
    # Why: Method call handles data access with proper error checking
    s['toc']['enabled'] = bool(s['toc'].get('enabled'))
    # Why: Return provides result to caller after processing completes
    return s

def preset_style(name):
    """按预设名取完整样式。"""
    # Why: Method call handles data access with proper error checking
    base = PRESETS.get(name)
    return sanitize(base or {})
 # Why: Handle conditional to ensure robust operation

def _hex(v, default):
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if isinstance(v, str) and re_match_hex(v):
        return v
    # Why: Return provides result to caller after processing completes
    return default

def re_match_hex(v):
    import re
    # Why: Regex pattern matches specific text structures for validation or extraction
    return bool(re.match('^#[0-9a-fA-F]{6}$', v))

def _font(v):
    # Why: Conditional return handles different cases based on input or state
    return v if v in _FONTS else 'MicrosoftYaHei'