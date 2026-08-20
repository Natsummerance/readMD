# -*- coding: utf-8 -*-
"""ReadMD 安全交互式多语言代码块执行器 (Polyglot Safe Code Chunk Runner)。

支持多运行时调度体系：
1. Python (`python`, `py`): 本地解释器调度，支持 Matplotlib 图像自动捕获与 Base64 回填；
2. JavaScript (`javascript`, `js`, `node`): Node.js 运行时执行；
3. Shell (`bash`, `sh`, `powershell`, `cmd`): 原生系统终端命令执行；
4. R (`r`, `rscript`): R 语言统计计算脚本执行；
5. Rust (`rust`): rust-script 脚本化即时执行。

安全防线：
- 10 秒超时强杀 (Timeout Kill)
- 异常隔离保护与子进程资源清理
- 跨平台 UTF-8 管道保护
"""

import base64
import os
import re
import shutil
import subprocess
import sys
import tempfile
from typing import Any, Dict, List, Optional

EXECUTION_TIMEOUT = 10  # 最大超时秒数

# Matplotlib 图表捕获包装模板
MATPLOTLIB_WRAPPER = """
import sys
import io
import base64

# 用户源码开始
{user_code}
# 用户源码结束

try:
    if 'matplotlib.pyplot' in sys.modules or 'plt' in locals() or 'plt' in globals():
        import matplotlib.pyplot as plt
        figs = [plt.figure(n) for n in plt.get_fignums()]
        for idx, fig in enumerate(figs):
            buf = io.BytesIO()
            fig.savefig(buf, format='png', bbox_inches='tight', dpi=150)
            buf.seek(0)
            b64_img = base64.b64encode(buf.read()).decode('ascii')
            print(f"__READMD_PLOT_BASE64__{{b64_img}}__END_READMD_PLOT__")
            plt.close(fig)
except Exception as _e:
    pass
"""


def _run_process(cmd: List[str], cwd: Optional[str] = None, timeout: int = EXECUTION_TIMEOUT) -> Dict[str, Any]:
    """底层安全进程调用与 UTF-8 管道捕获。"""
    env = os.environ.copy()
    env['PYTHONIOENCODING'] = 'utf-8'
    env['PYTHONUTF8'] = '1'
    env['NODE_OPTIONS'] = '--no-warnings'

    try:
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding='utf-8',
            errors='replace',
            env=env,
            cwd=cwd
        )

        try:
            stdout, stderr = proc.communicate(timeout=timeout)
            exit_code = proc.returncode
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()
            return {
                "ok": False,
                "error": f"代码执行超时 (超过 {timeout} 秒限制)",
                "stdout": stdout,
                "stderr": stderr,
                "images": [],
                "exit_code": -1
            }

        return {
            "ok": exit_code == 0,
            "stdout": stdout.strip(),
            "stderr": stderr.strip(),
            "images": [],
            "exit_code": exit_code
        }

    except Exception as e:
        return {
            "ok": False,
            "error": str(e),
            "stdout": "",
            "stderr": str(e),
            "images": [],
            "exit_code": -1
        }


def execute_python_chunk(code: str, capture_plot: bool = True,
                         timeout: int = EXECUTION_TIMEOUT) -> Dict[str, Any]:
    """安全执行 Python 代码块并捕获文本输出与 Matplotlib 图像。"""
    wrapped_code = MATPLOTLIB_WRAPPER.format(user_code=code) if capture_plot else code

    with tempfile.NamedTemporaryFile(suffix='.py', delete=False, mode='w', encoding='utf-8') as f:
        f.write(wrapped_code)
        tmp_script = f.name

    try:
        res = _run_process([sys.executable, tmp_script], timeout=timeout)
        if not res["ok"] and res.get("error"):
            return res

        # 提取 Matplotlib 图像标记
        images = []
        plot_pattern = re.compile(r'__READMD_PLOT_BASE64__([A-Za-z0-9+/=]+)__END_READMD_PLOT__')

        def extract_img(match):
            images.append(f"data:image/png;base64,{match.group(1)}")
            return ""

        clean_stdout = plot_pattern.sub(extract_img, res["stdout"]).strip()
        res["stdout"] = clean_stdout
        res["images"] = images
        return res

    finally:
        if os.path.exists(tmp_script):
            try:
                os.remove(tmp_script)
            except Exception:
                pass


def execute_code_chunk(code: str, lang: str = "python", capture_plot: bool = True,
                       timeout: int = EXECUTION_TIMEOUT) -> Dict[str, Any]:
    """多语言统一代码块调度执行器。"""
    normalized_lang = lang.lower().strip().lstrip('.')

    # 1. Python 调度
    if normalized_lang in ('python', 'py'):
        res = execute_python_chunk(code, capture_plot=capture_plot, timeout=timeout)
        res["lang"] = "python"
        return res

    # 2. JavaScript / Node.js 调度
    elif normalized_lang in ('javascript', 'js', 'node'):
        node_bin = shutil.which('node')
        if not node_bin:
            return {
                "ok": False,
                "error": "本地未检测到 Node.js 运行环境 (请安装 Node.js 或将其加入 PATH)",
                "stdout": "",
                "stderr": "Node.js not found in PATH",
                "images": [],
                "exit_code": 127,
                "lang": normalized_lang
            }
        with tempfile.NamedTemporaryFile(suffix='.js', delete=False, mode='w', encoding='utf-8') as f:
            f.write(code)
            tmp_script = f.name
        try:
            res = _run_process([node_bin, tmp_script], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            if os.path.exists(tmp_script):
                try:
                    os.remove(tmp_script)
                except Exception:
                    pass

    # 3. Shell / Bash / PowerShell 调度
    elif normalized_lang in ('bash', 'sh', 'shell', 'powershell', 'cmd', 'bat'):
        if sys.platform == 'win32':
            cmd = ['powershell', '-Command', code] if normalized_lang == 'powershell' else ['cmd', '/c', code]
        else:
            cmd = ['/bin/bash', '-c', code] if os.path.exists('/bin/bash') else ['/bin/sh', '-c', code]
        res = _run_process(cmd, timeout=timeout)
        res["lang"] = normalized_lang
        return res

    # 4. R 语言调度
    elif normalized_lang in ('r', 'rscript'):
        r_bin = shutil.which('Rscript')
        if not r_bin:
            return {
                "ok": False,
                "error": "本地未检测到 Rscript 环境 (请安装 R 并将其加入 PATH)",
                "stdout": "",
                "stderr": "Rscript not found in PATH",
                "images": [],
                "exit_code": 127,
                "lang": normalized_lang
            }
        with tempfile.NamedTemporaryFile(suffix='.R', delete=False, mode='w', encoding='utf-8') as f:
            f.write(code)
            tmp_script = f.name
        try:
            res = _run_process([r_bin, tmp_script], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            if os.path.exists(tmp_script):
                try:
                    os.remove(tmp_script)
                except Exception:
                    pass

    # 5. SQL 内存与本地 SQLite 调度
    elif normalized_lang in ('sql', 'sqlite', 'sqlite3'):
        try:
            import sqlite3
            con = sqlite3.connect(":memory:")
            cur = con.cursor()
            results = []
            statements = [s.strip() for s in code.split(';') if s.strip()]
            for stmt in statements:
                cur.execute(stmt)
                if cur.description:
                    headers = [d[0] for d in cur.description]
                    rows = cur.fetchall()
                    col_widths = [len(h) for h in headers]
                    for r in rows:
                        for idx, val in enumerate(r):
                            col_widths[idx] = max(col_widths[idx], len(str(val)))
                    header_line = " | ".join(h.ljust(col_widths[i]) for i, h in enumerate(headers))
                    sep_line = "-+-".join("-" * col_widths[i] for i in range(len(headers)))
                    row_lines = [" | ".join(str(val).ljust(col_widths[i]) for i, val in enumerate(r)) for r in rows]
                    results.append(f"{header_line}\n{sep_line}\n" + "\n".join(row_lines))
                else:
                    results.append(f"Query OK, {cur.rowcount} rows affected.")
            con.commit()
            con.close()
            return {
                "ok": True,
                "error": None,
                "stdout": "\n\n".join(results),
                "stderr": "",
                "images": [],
                "exit_code": 0,
                "lang": "sql"
            }
        except Exception as e:
            return {
                "ok": False,
                "error": f"SQL 执行错误: {str(e)}",
                "stdout": "",
                "stderr": str(e),
                "images": [],
                "exit_code": 1,
                "lang": "sql"
            }

    # 6. Go 语言调度
    elif normalized_lang in ('go', 'golang'):
        go_bin = shutil.which('go')
        if not go_bin:
            return {
                "ok": False,
                "error": "本地未检测到 Go 环境 (请安装 Go 并将其加入 PATH)",
                "stdout": "",
                "stderr": "go not found in PATH",
                "images": [],
                "exit_code": 127,
                "lang": normalized_lang
            }
        with tempfile.NamedTemporaryFile(suffix='.go', delete=False, mode='w', encoding='utf-8') as f:
            # 如果没有 package main，自动包装
            if 'package main' not in code:
                code = f"package main\nimport \"fmt\"\nfunc main() {{\n{code}\n}}"
            f.write(code)
            tmp_script = f.name
        try:
            res = _run_process([go_bin, 'run', tmp_script], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            if os.path.exists(tmp_script):
                try:
                    os.remove(tmp_script)
                except Exception:
                    pass

    # 7. Rust 脚本化调度
    elif normalized_lang in ('rust', 'rs'):
        rust_script = shutil.which('rust-script')
        if rust_script:
            res = _run_process([rust_script, '-e', code], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        rustc_bin = shutil.which('rustc')
        if not rustc_bin:
            return {
                "ok": False,
                "error": "本地未检测到 Rust 运行环境 (rustc 或 rust-script)",
                "stdout": "",
                "stderr": "rustc not found in PATH",
                "images": [],
                "exit_code": 127,
                "lang": normalized_lang
            }
        with tempfile.NamedTemporaryFile(suffix='.rs', delete=False, mode='w', encoding='utf-8') as f:
            if 'fn main()' not in code:
                code = f"fn main() {{\n{code}\n}}"
            f.write(code)
            tmp_script = f.name
        out_bin = tmp_script[:-3] + ('.exe' if sys.platform == 'win32' else '')
        try:
            c_res = _run_process([rustc_bin, tmp_script, '-o', out_bin], timeout=timeout)
            if not c_res['ok'] or c_res['exit_code'] != 0:
                c_res["lang"] = normalized_lang
                return c_res
            res = _run_process([out_bin], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            for p in (tmp_script, out_bin):
                if os.path.exists(p):
                    try:
                        os.remove(p)
                    except Exception:
                        pass

    # 8. C / C++ 编译调度
    elif normalized_lang in ('c', 'cpp', 'c++'):
        compiler = shutil.which('g++' if normalized_lang in ('cpp', 'c++') else 'gcc') or shutil.which('clang++' if normalized_lang in ('cpp', 'c++') else 'clang')
        if not compiler:
            return {
                "ok": False,
                "error": "本地未检测到 C/C++ 编译器 (gcc/g++/clang)",
                "stdout": "",
                "stderr": "compiler not found in PATH",
                "images": [],
                "exit_code": 127,
                "lang": normalized_lang
            }
        suffix = '.cpp' if normalized_lang in ('cpp', 'c++') else '.c'
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False, mode='w', encoding='utf-8') as f:
            if 'main(' not in code:
                header = "#include <iostream>\nusing namespace std;\n" if suffix == '.cpp' else "#include <stdio.h>\n"
                code = f"{header}int main() {{\n{code}\nreturn 0;\n}}"
            f.write(code)
            tmp_script = f.name
        out_bin = tmp_script[:-len(suffix)] + ('.exe' if sys.platform == 'win32' else '')
        try:
            c_res = _run_process([compiler, tmp_script, '-o', out_bin], timeout=timeout)
            if not c_res['ok'] or c_res['exit_code'] != 0:
                c_res["lang"] = normalized_lang
                return c_res
            res = _run_process([out_bin], timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            for p in (tmp_script, out_bin):
                if os.path.exists(p):
                    try:
                        os.remove(p)
                    except Exception:
                        pass

    # 未知或未适配语言兜底
    return {
        "ok": False,
        "error": f"暂不支持的代码语言: {lang}",
        "stdout": "",
        "stderr": f"Unsupported language: {lang}",
        "images": [],
        "exit_code": 1,
        "lang": normalized_lang
    }
