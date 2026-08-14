# -*- coding: utf-8 -*-
"""导出样式：schema + 默认值 + 内置预设 + 合并/校验。

样式结构（与前端导出面板一一对应）：
  page     纸张/方向/页边距(mm)
  cover    封面（标题/副标题/日期/对齐）
  toc      PDF 目录开关
  typography 正文（字体/字号/行距/段距/颜色/对齐）
  headings h1..h6（颜色/字号/加粗/对齐/前/后间距）
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

DEFAULT_STYLE = {
    'page': {'size': 'A4', 'orientation': 'portrait',
             'marginTop': 20, 'marginRight': 18, 'marginBottom': 20, 'marginLeft': 18},
    'cover': {'enabled': False, 'title': '', 'subtitle': '', 'date': '', 'align': 'center'},
    'toc': {'enabled': False},
    'typography': {'font': 'MicrosoftYaHei', 'size': 11, 'lineHeight': 1.6,
                   'spacing': 6, 'color': '#262626', 'align': 'left'},
    'headings': {
        'h1': {'size': 20, 'color': '#1a1a1a', 'bold': True, 'align': 'left', 'before': 18, 'after': 10},
        'h2': {'size': 16, 'color': '#1f2937', 'bold': True, 'align': 'left', 'before': 14, 'after': 8},
        'h3': {'size': 14, 'color': '#2d3748', 'bold': True, 'align': 'left', 'before': 12, 'after': 6},
        'h4': {'size': 12, 'color': '#374151', 'bold': True, 'align': 'left', 'before': 10, 'after': 6},
        'h5': {'size': 11, 'color': '#4a5568', 'bold': True, 'align': 'left', 'before': 8, 'after': 4},
        'h6': {'size': 10.5, 'color': '#4a5568', 'bold': True, 'align': 'left', 'before': 8, 'after': 4},
    },
    'table': {'headerBg': '#3b6ef5', 'headerColor': '#ffffff', 'headerBold': True,
              'borderColor': '#c8cdd4', 'borderWidth': 0.75, 'banded': True,
              'bandColor': '#f3f5f9', 'cellSize': 10, 'cellPadding': 6,
              'align': 'left', 'widthPct': 100},
    'code': {'bg': '#f5f6f8', 'color': '#2f3b4a', 'font': 'Consolas', 'size': 9.5,
             'borderColor': '#dfe3e8', 'borderWidth': 0.5, 'rounded': True},
    'quote': {'barColor': '#3b6ef5', 'bg': '#f3f6ff', 'color': '#4a5568'},
    'link': {'color': '#2b6cb0'},
    'hr': {'color': '#d8dce2'},
    'footer': {'pageNumbers': True, 'text': ''},
    'math': {'dpi': 220},
    'meta': {'title': '', 'author': '', 'subject': ''},
    'htmlTheme': 'light',
}

# 内置预设（在默认值之上的增量覆盖）
PRESETS = {
    'minimal': {
        'typography': {'size': 10.5, 'lineHeight': 1.55, 'color': '#333333', 'align': 'left'},
        'headings': {
            'h1': {'size': 18, 'color': '#111111'},
            'h2': {'size': 14.5, 'color': '#222222'},
            'h3': {'size': 12.5, 'color': '#333333'},
        },
        'table': {'headerBg': '#eef1f5', 'headerColor': '#333333', 'headerBold': True,
                  'borderColor': '#ccd2da', 'borderWidth': 0.5, 'banded': False,
                  'cellSize': 9.5, 'cellPadding': 5, 'align': 'left', 'widthPct': 100},
        'code': {'bg': '#f7f8fa', 'color': '#444444', 'borderColor': '#e3e6ea', 'borderWidth': 0.5},
        'quote': {'barColor': '#9aa3af', 'bg': '#f6f7f9', 'color': '#555555'},
        'link': {'color': '#1a73e8'},
        'hr': {'color': '#e0e3e8'},
    },
    'classic': {
        'typography': {'size': 12, 'lineHeight': 1.8, 'spacing': 8, 'color': '#1a1a1a', 'align': 'left'},
        'headings': {
            'h1': {'size': 22, 'color': '#000000', 'align': 'center'},
            'h2': {'size': 17, 'color': '#111111'},
            'h3': {'size': 14.5, 'color': '#222222'},
            'h4': {'size': 13, 'color': '#333333'},
        },
        'table': {'headerBg': '#d9e2ec', 'headerColor': '#1f2d3d', 'headerBold': True,
                  'borderColor': '#8a94a6', 'borderWidth': 1.0, 'banded': True,
                  'bandColor': '#f2f5f8', 'cellSize': 10.5, 'cellPadding': 7,
                  'align': 'left', 'widthPct': 100},
        'code': {'bg': '#f4f4f0', 'color': '#333333', 'borderColor': '#c9c9c4', 'borderWidth': 0.75},
        'quote': {'barColor': '#7a8699', 'bg': '#f5f6f8', 'color': '#3d4852'},
        'link': {'color': '#8a2be2'},
        'hr': {'color': '#b5b5ad'},
    },
    'business': {
        'typography': {'size': 11, 'lineHeight': 1.65, 'spacing': 6, 'color': '#2c3e50', 'align': 'left'},
        'headings': {
            'h1': {'size': 20, 'color': '#1f3864', 'bold': True},
            'h2': {'size': 16, 'color': '#2e5395'},
            'h3': {'size': 13.5, 'color': '#3a6db5'},
            'h4': {'size': 12, 'color': '#4a7fd4'},
        },
        'table': {'headerBg': '#1f3864', 'headerColor': '#ffffff', 'headerBold': True,
                  'borderColor': '#9fb3d1', 'borderWidth': 0.75, 'banded': True,
                  'bandColor': '#eef3fa', 'cellSize': 10, 'cellPadding': 6,
                  'align': 'left', 'widthPct': 100},
        'code': {'bg': '#f0f4fa', 'color': '#1f3864', 'borderColor': '#c3d0e4', 'borderWidth': 0.5},
        'quote': {'barColor': '#1f3864', 'bg': '#eef3fa', 'color': '#34507c'},
        'link': {'color': '#1f3864'},
        'hr': {'color': '#b9c6da'},
    },
}

PAGE_SIZES = ('A4', 'A5', 'B5', 'Letter', 'Legal')
ORIENTATIONS = ('portrait', 'landscape')
HTML_THEMES = ('light', 'dark', 'sepia')
_FONTS = ('MicrosoftYaHei', 'SimHei', 'SimSun', 'KaiTi', 'DengXian', 'Arial')
_MONO = ('Consolas', 'Courier New', 'SimHei')
_ALIGNS = ('left', 'center', 'right', 'justify')


def deep_merge(base, over):
    """递归合并，返回新字典；over 中 None 跳过。"""
    out = copy.deepcopy(base)
    if not isinstance(over, dict):
        return out
    for k, v in over.items():
        if v is None:
            continue
        if isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = deep_merge(out[k], v)
        else:
            out[k] = copy.deepcopy(v)
    return out


def _clamp(v, lo, hi, default):
    try:
        f = float(v)
    except (TypeError, ValueError):
        return default
    return max(lo, min(hi, f))


def sanitize(options):
    """把任意用户输入规整为合法样式（非法值回退默认）。"""
    s = deep_merge(DEFAULT_STYLE, options or {})
    p = s['page']
    if p['size'] not in PAGE_SIZES:
        p['size'] = DEFAULT_STYLE['page']['size']
    if p['orientation'] not in ORIENTATIONS:
        p['orientation'] = 'portrait'
    for k in ('marginTop', 'marginRight', 'marginBottom', 'marginLeft'):
        p[k] = _clamp(p.get(k), 0, 60, DEFAULT_STYLE['page'][k])

    t = s['typography']
    t['size'] = _clamp(t.get('size'), 8, 20, 11)
    t['lineHeight'] = _clamp(t.get('lineHeight'), 1.0, 2.5, 1.6)
    t['spacing'] = _clamp(t.get('spacing'), 0, 30, 6)
    t['align'] = t['align'] if t.get('align') in _ALIGNS else 'left'
    t['color'] = _hex(t.get('color'), '#262626')
    t['font'] = _font(t.get('font'))

    for i in range(1, 7):
        h = s['headings']['h%d' % i]
        h['size'] = _clamp(h.get('size'), 8, 40, DEFAULT_STYLE['headings']['h%d' % i]['size'])
        h['bold'] = bool(h.get('bold', True))
        h['align'] = h['align'] if h.get('align') in _ALIGNS else 'left'
        h['color'] = _hex(h.get('color'), '#1a1a1a')
        h['before'] = _clamp(h.get('before'), 0, 60, 10)
        h['after'] = _clamp(h.get('after'), 0, 40, 6)

    tb = s['table']
    tb['headerBg'] = _hex(tb.get('headerBg'), '#3b6ef5')
    tb['headerColor'] = _hex(tb.get('headerColor'), '#ffffff')
    tb['headerBold'] = bool(tb.get('headerBold', True))
    tb['borderColor'] = _hex(tb.get('borderColor'), '#c8cdd4')
    tb['borderWidth'] = _clamp(tb.get('borderWidth'), 0, 3, 0.75)
    tb['banded'] = bool(tb.get('banded', True))
    tb['bandColor'] = _hex(tb.get('bandColor'), '#f3f5f9')
    tb['cellSize'] = _clamp(tb.get('cellSize'), 7, 16, 10)
    tb['cellPadding'] = _clamp(tb.get('cellPadding'), 0, 20, 6)
    tb['align'] = tb['align'] if tb.get('align') in _ALIGNS else 'left'
    tb['widthPct'] = _clamp(tb.get('widthPct'), 50, 100, 100)

    c = s['code']
    c['bg'] = _hex(c.get('bg'), '#f5f6f8')
    c['color'] = _hex(c.get('color'), '#2f3b4a')
    c['font'] = c.get('font') if c.get('font') in _MONO else 'Consolas'
    c['size'] = _clamp(c.get('size'), 6, 16, 9.5)
    c['borderColor'] = _hex(c.get('borderColor'), '#dfe3e8')
    c['borderWidth'] = _clamp(c.get('borderWidth'), 0, 3, 0.5)
    c['rounded'] = bool(c.get('rounded', True))

    q = s['quote']
    q['barColor'] = _hex(q.get('barColor'), '#3b6ef5')
    q['bg'] = _hex(q.get('bg'), '#f3f6ff')
    q['color'] = _hex(q.get('color'), '#4a5568')

    s['link']['color'] = _hex(s['link'].get('color'), '#2b6cb0')
    s['hr']['color'] = _hex(s['hr'].get('color'), '#d8dce2')
    s['footer']['pageNumbers'] = bool(s['footer'].get('pageNumbers', True))
    s['footer']['text'] = str(s['footer'].get('text') or '')[:80]
    s['math']['dpi'] = int(_clamp(s['math'].get('dpi'), 100, 500, 220))
    s['htmlTheme'] = s['htmlTheme'] if s.get('htmlTheme') in HTML_THEMES else 'light'

    meta = s['meta']
    for k in ('title', 'author', 'subject'):
        meta[k] = str(meta.get(k) or '')[:120]

    cover = s['cover']
    cover['enabled'] = bool(cover.get('enabled'))
    for k in ('title', 'subtitle', 'date'):
        cover[k] = str(cover.get(k) or '')[:120]
    cover['align'] = cover['align'] if cover.get('align') in ('left', 'center', 'right') else 'center'
    s['toc']['enabled'] = bool(s['toc'].get('enabled'))
    return s


def preset_style(name):
    """按预设名取完整样式。"""
    base = PRESETS.get(name)
    return sanitize(base or {})


def _hex(v, default):
    if isinstance(v, str) and re_match_hex(v):
        return v
    return default


def re_match_hex(v):
    import re
    return bool(re.match(r'^#[0-9a-fA-F]{6}$', v))


def _font(v):
    return v if v in _FONTS else 'MicrosoftYaHei'
