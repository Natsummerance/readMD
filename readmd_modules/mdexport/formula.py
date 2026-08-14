# -*- coding: utf-8 -*-
"""LaTeX 公式 -> PNG（matplotlib mathtext，无外部 LaTeX 依赖）。

mathtext 支持常见子集（分数/根号/希腊字母/上下标/矩阵等）；
不支持的语法抛错时返回 None，由调用方回退为源码文本。
"""

import io
import logging
import re
import warnings

_mp = None
_cjk_setup = False
_CJK_RE = re.compile(r'[\u4e00-\u9fff\u3000-\u303f\uff00-\uffef]')


def _matplotlib():
    global _mp
    if _mp is None:
        import matplotlib
        matplotlib.use('Agg')
        import matplotlib.pyplot as plt
        _mp = plt
    return _mp


def _setup_cjk():
    global _cjk_setup
    if _cjk_setup:
        return
    try:
        from matplotlib import font_manager, rcParams
        if not font_manager.findfont('Microsoft YaHei', fallback_to_default=False):
            font_manager.fontManager.addfont(r'C:\Windows\Fonts\msyh.ttc')
        rcParams.update({'mathtext.fontset': 'custom',
                         'mathtext.rm': 'Microsoft YaHei',
                         'mathtext.it': 'Microsoft YaHei',
                         'mathtext.bf': 'Microsoft YaHei',
                         'mathtext.tt': 'Microsoft YaHei'})
    except Exception:
        pass
    _cjk_setup = True


def render_latex(latex, dpi=220):
    """渲染公式为透明 PNG bytes；失败返回 None。"""
    try:
        plt = _matplotlib()
        text = latex.strip()
        if not text:
            return None
        if _CJK_RE.search(text):
            _setup_cjk()
        expr = '$' + text + '$'
        fig = plt.figure(figsize=(0.1, 0.1))
        try:
            with warnings.catch_warnings():
                warnings.simplefilter('ignore')
                t = fig.text(0, 0, expr, fontsize=12)
                fig.canvas.draw()
                buf = io.BytesIO()
                fig.savefig(buf, format='png', transparent=True,
                            bbox_inches='tight', pad_inches=0.03, dpi=dpi)
            data = buf.getvalue()
            return data if data[:4] == b'\x89PNG' else None
        finally:
            plt.close(fig)
    except Exception:
        logging.exception('formula render failed: %s', latex)
        return None


def png_size(data):
    """从 PNG bytes 读取 (px_w, px_h)。"""
    try:
        import struct
        if data[:8] != b'\x89PNG\r\n\x1a\n':
            return None
        w, h = struct.unpack('>II', data[16:24])
        return w, h
    except Exception:
        return None


def prepare(blocks, style, warns):
    """遍历块 AST，把 math 节点预渲染并标注 png/w/h(pt)/fallback。

    display 公式渲染为独立图；行内公式渲染为小图（按行高缩放）。
    """
    dpi = int(style['math']['dpi'])
    base_pt = float(style['typography']['size'])
    rendered = {}

    def _do(nodes, display_hint=False):
        for nd in nodes:
            if nd.get('t') == 'math':
                latex = nd.get('latex', '')
                key = (latex, nd.get('display'))
                info = rendered.get(key)
                if info is None:
                    data = render_latex(latex, dpi)
                    if data:
                        size = png_size(data)
                        if size:
                            w_pt = size[0] / dpi * 72.0
                            h_pt = size[1] / dpi * 72.0
                            info = {'png': data, 'w': w_pt, 'h': h_pt, 'fallback': False}
                        else:
                            info = {'fallback': True}
                    else:
                        info = {'fallback': True}
                    rendered[key] = info
                if info.get('fallback'):
                    warns.append('公式无法渲染，已按文本保留：%s' % latex[:60])
                    nd['fallback'] = True
                else:
                    nd['png'] = info['png']
                    nd['w'] = info['w']
                    nd['h'] = info['h']
                    nd['fallback'] = False
                # 行内公式按正文行高约束高度
                if not nd.get('display') and nd.get('w'):
                    scale = min(1.0, (base_pt * 1.4) / nd['h'])
                    nd['w'] *= scale
                    nd['h'] *= scale
            elif nd.get('t') == 'link' and isinstance(nd.get('text'), list):
                _do(nd['text'])
            elif nd.get('t') in ('bold', 'italic', 'strike') and isinstance(nd.get('v'), list):
                _do(nd['v'])

    def _walk(blks):
        for b in blks:
            if b['type'] in ('paragraph', 'heading'):
                _do(b.get('text', []))
            elif b['type'] == 'table':
                for cell in b.get('header', []):
                    _do(cell)
                for row in b.get('rows', []):
                    for cell in row:
                        _do(cell)
            elif b['type'] == 'list':
                for it in b.get('items', []):
                    _do(it.get('text', []))
            elif b['type'] == 'quote':
                _walk(b.get('blocks', []))
            elif b['type'] == 'math' and b.get('display'):
                latex = b.get('latex', '')
                info = rendered.get((latex, True))
                if info is None:
                    data = render_latex(latex, dpi)
                    if data:
                        size = png_size(data)
                        if size:
                            info = {'png': data, 'w': size[0] / dpi * 72.0,
                                    'h': size[1] / dpi * 72.0, 'fallback': False}
                        else:
                            info = {'fallback': True}
                    else:
                        info = {'fallback': True}
                    rendered[(latex, True)] = info
                if info.get('fallback'):
                    warns.append('公式无法渲染，已按文本保留：%s' % latex[:60])
                    b['fallback'] = True
                else:
                    b['png'] = info['png']
                    b['w'] = info['w']
                    b['h'] = info['h']
                    b['fallback'] = False

    _walk(blocks)
    return rendered
