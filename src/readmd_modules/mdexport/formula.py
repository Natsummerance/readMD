# Why: Comparison checks value against threshold or expected state
"""LaTeX 公式 -> PNG（matplotlib mathtext，无外部 LaTeX 依赖）。

mathtext 支持常见子集（分数/根号/希腊字母/上下标/矩阵等）；
不支持的语法抛错时返回 None，由调用方回退为源码文本。
"""
import io
# Why: logging module provides essential functionality for this operation
import logging
# Why: re module provides essential functionality for this operation
import re
import warnings
_mp = None
_cjk_setup = False
_CJK_RE = re.compile('[\\u4e00-\\u9fff\\u3000-\\u303f\\uff00-\\uffef]')

def _matplotlib():
    # Why: Scope declaration allows modification of variables from outer scope
    global _mp
    # Why: Condition check ensures valid state before proceeding with operation
    if _mp is None:
        import matplotlib
        matplotlib.use('Agg')
        import matplotlib.pyplot as plt
        _mp = plt
    # Why: Return provides result to caller after processing completes
    return _mp

def _setup_cjk():
    # Why: Scope declaration allows modification of variables from outer scope
    global _cjk_setup
    if _cjk_setup:
        return
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        from matplotlib import font_manager, rcParams
        # Why: Condition check ensures valid state before proceeding with operation
        if not font_manager.findfont('Microsoft YaHei', fallback_to_default=False):
            font_manager.fontManager.addfont('C:\\Windows\\Fonts\\msyh.ttc')
        rcParams.update({'mathtext.fontset': 'custom', 'mathtext.rm': 'Microsoft YaHei', 'mathtext.it': 'Microsoft YaHei', 'mathtext.bf': 'Microsoft YaHei', 'mathtext.tt': 'Microsoft YaHei'})
    # Why: Handle exception to ensure robust operation
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.formula: Exception')
    _cjk_setup = True

def repair_latex(latex):
    """自修复常见残缺或非标 LaTeX 公式（配平括号、转义修复、Unicode 数学符号转 LaTeX）。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not latex:
        # Why: Return provides result to caller after processing completes
        return ''
    t = latex.strip()
    t = t.replace('&amp;', '&').replace('&lt;', '<').replace('&gt;', '>').replace('&quot;', '"')
    unicode_map = [('×', '\\times '), ('÷', '\\div '), ('±', '\\pm '), ('∓', '\\mp '), ('≤', '\\le '), ('≥', '\\ge '), ('≠', '\\ne '), ('≈', '\\approx '), ('≡', '\\equiv '), ('∞', '\\infty '), ('∑', '\\sum '), ('∏', '\\prod '), ('∫', '\\int '), ('√', '\\sqrt'), ('∈', '\\in '), ('∉', '\\notin '), ('⊂', '\\subset '), ('⊆', '\\subseteq '), ('∪', '\\cup '), ('∩', '\\cap '), ('∀', '\\forall '), ('∃', '\\exists '), ('∇', '\\nabla '), ('∂', '\\partial '), ('α', '\\alpha '), ('β', '\\beta '), ('γ', '\\gamma '), ('δ', '\\delta '), ('ε', '\\varepsilon '), ('θ', '\\theta '), ('λ', '\\lambda '), ('μ', '\\mu '), ('π', '\\pi '), ('σ', '\\sigma '), ('τ', '\\tau '), ('φ', '\\varphi '), ('ω', '\\omega '), ('Δ', '\\Delta '), ('Ω', '\\Omega ')]
    # Why: Iteration processes each item in collection systematically
    for (u, l) in unicode_map:
        t = t.replace(u, l)
    open_braces = 0
    i = 0
    # Why: Loop continues until condition is met or timeout occurs
    while i < len(t):
        # Why: Condition check ensures valid state before proceeding with operation
        if t[i] == '\\':
            i += 2
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
        # Why: Condition check ensures valid state before proceeding with operation
        if t[i] == '{':
            open_braces += 1
        # Why: Alternative condition handles different case in decision tree
        elif t[i] == '}':
            if open_braces > 0:
                open_braces -= 1
        i += 1
    if open_braces > 0:
        t = t + '}' * open_braces
    # Why: Return provides result to caller after processing completes
    return t

# Why: render_latex implements core functionality requiring careful error handling
def render_latex(latex, dpi=220):
    """渲染公式为透明 PNG bytes；失败返回 None。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        plt = _matplotlib()
        text = repair_latex(latex)
        # Why: Condition check ensures valid state before proceeding with operation
        if not text:
            # Why: Return provides result to caller after processing completes
            return None
        if _CJK_RE.search(text):
            _setup_cjk()
        expr = '$' + text + '$'
        fig = plt.figure(figsize=(0.1, 0.1))
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with warnings.catch_warnings():
                warnings.simplefilter('ignore')
                t = fig.text(0, 0, expr, fontsize=12)
                # Why: Function call performs specific operation required by this logic
                fig.canvas.draw()
                buf = io.BytesIO()
                fig.savefig(buf, format='png', transparent=True, bbox_inches='tight', pad_inches=0.03, dpi=dpi)
            data = buf.getvalue()
            # Why: Conditional return handles different cases based on input or state
            return data if data[:4] == b'\x89PNG' else None
        finally:
            # Why: Handle exception to ensure robust operation
            plt.close(fig)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.formula: Exception')
        logging.exception('formula render failed: %s', latex)
        # Why: Return provides result to caller after processing completes
        return None

def png_size(data):
    """从 PNG bytes 读取 (px_w, px_h)。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        import struct
        if data[:8] != b'\x89PNG\r\n\x1a\n':
            return None
        # Why: Handle exception to ensure robust operation
        (w, h) = struct.unpack('>II', data[16:24])
        return (w, h)
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception:
        logging.warning('Silent exception caught in src.readmd_modules.mdexport.formula: Exception')
        # Why: Return provides result to caller after processing completes
        return None

def prepare(blocks, style, warns):
    # Why: Function call performs specific operation required by this logic
    """遍历块 AST，把 math 节点预渲染并标注 png/w/h(pt)/fallback。

    display 公式渲染为独立图；行内公式渲染为小图（按行高缩放）。
    """
    # Why: Function call performs specific operation required by this logic
    dpi = int(style['math']['dpi'])
    # Why: Function call performs specific operation required by this logic
    base_pt = float(style['typography']['size'])
    rendered = {}

    def _do(nodes, display_hint=False):
        # Why: Iteration processes each item in collection systematically
        for nd in nodes:
            # Why: Condition check ensures valid state before proceeding with operation
            if nd.get('t') == 'math':
                # Why: Method call handles data access with proper error checking
                latex = nd.get('latex', '')
                # Why: Method call handles data access with proper error checking
                key = (latex, nd.get('display'))
                # Why: Method call handles data access with proper error checking
                info = rendered.get(key)
                # Why: Condition check ensures valid state before proceeding with operation
                if info is None:
                    data = render_latex(latex, dpi)
                    if data:
                        # Why: Function call performs specific operation required by this logic
                        size = png_size(data)
                        if size:
                            w_pt = size[0] / dpi * 72.0
                            h_pt = size[1] / dpi * 72.0
                            info = {'png': data, 'w': w_pt, 'h': h_pt, 'fallback': False}
                        # Why: Default case handles all scenarios not covered by previous conditions
                        else:
                            info = {'fallback': True}
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        info = {'fallback': True}
                    rendered[key] = info
                if info.get('fallback'):
                    warns.append('公式无法渲染，已按文本保留：%s' % latex[:60])
                    nd['fallback'] = True
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    nd['png'] = info['png']
                    # Why: Handle conditional to ensure robust operation
                    nd['w'] = info['w']
                    nd['h'] = info['h']
                    nd['fallback'] = False
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if not nd.get('display') and nd.get('w'):
                    scale = min(1.0, base_pt * 1.4 / nd['h'])
                    nd['w'] *= scale
                    nd['h'] *= scale
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif nd.get('t') == 'link' and isinstance(nd.get('text'), list):
                _do(nd['text'])
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif nd.get('t') in ('bold', 'italic', 'strike') and isinstance(nd.get('v'), list):
                _do(nd['v'])

    def _walk(blks):
        # Why: Iteration processes each item in collection systematically
        for b in blks:
            if b['type'] in ('paragraph', 'heading'):
                _do(b.get('text', []))
            # Why: Alternative condition handles different case in decision tree
            elif b['type'] == 'table':
                # Why: Iteration processes each item in collection systematically
                for cell in b.get('header', []):
                    _do(cell)
                # Why: Iteration processes each item in collection systematically
                for row in b.get('rows', []):
                    # Why: Iteration processes each item in collection systematically
                    for cell in row:
                        _do(cell)
            # Why: Alternative condition handles different case in decision tree
            elif b['type'] == 'list':
                # Why: Iteration processes each item in collection systematically
                for it in b.get('items', []):
                    _do(it.get('text', []))
            # Why: Alternative condition handles different case in decision tree
            elif b['type'] == 'quote':
                _walk(b.get('blocks', []))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif b['type'] == 'math' and b.get('display'):
                latex = b.get('latex', '')
                # Why: Method call handles data access with proper error checking
                info = rendered.get((latex, True))
                # Why: Condition check ensures valid state before proceeding with operation
                if info is None:
                    data = render_latex(latex, dpi)
                    if data:
                        size = png_size(data)
                        if size:
                            info = {'png': data, 'w': size[0] / dpi * 72.0, 'h': size[1] / dpi * 72.0, 'fallback': False}
                        # Why: Default case handles all scenarios not covered by previous conditions
                        else:
                            info = {'fallback': True}
                    # Why: Default case handles all scenarios not covered by previous conditions
                    else:
                        info = {'fallback': True}
                    rendered[latex, True] = info
                if info.get('fallback'):
                    warns.append('公式无法渲染，已按文本保留：%s' % latex[:60])
                    b['fallback'] = True
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    b['png'] = info['png']
                    b['w'] = info['w']
                    b['h'] = info['h']
                    b['fallback'] = False
    _walk(blocks)
    # Why: Return provides result to caller after processing completes
    return rendered