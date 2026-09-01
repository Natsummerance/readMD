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
import signal
from typing import Any, Dict, List, Optional

try:  # Unix-only resource ceilings; Windows uses process-group teardown below.
    import resource as _resource
except ImportError:  # pragma: no cover - exercised on Windows builds
    _resource = None

EXECUTION_TIMEOUT = 10  # 最大超时秒数
MAX_OUTPUT_CHARS = 200_000
MAX_TIMEOUT_SECONDS = 10
MAX_MEMORY_BYTES = 512 * 1024 * 1024
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_CHILD_PROCESSES = 32

# Code chunks run without inherited credentials or service configuration.  A
# child process receives only the variables needed to find runtimes and write
# temporary output; API keys, proxy settings and arbitrary user variables are
# deliberately excluded.
_SAFE_ENV_KEYS = (
    'PATH', 'PATHEXT', 'SYSTEMROOT', 'SYSTEMDRIVE', 'COMSPEC',
    'TEMP', 'TMP', 'TMPDIR', 'USERPROFILE', 'HOME', 'LANG', 'LC_ALL',
)
_NETWORK_PATTERNS = (
    re.compile(r'(?i)\b(?:requests|httpx|urllib(?:\.request)?|socket|ftplib|aiohttp)\b'),
    re.compile(r'''(?ix)(?:require\s*\(\s*['"](?:node:)?(?:http|https|net|tls|dns|dgram|undici)['"]|from\s+['"](?:node:)?(?:http|https|net|tls|dns|dgram|undici)['"]|\bfetch\s*\()'''),
    re.compile(r'(?i)\b(?:curl|wget|Invoke-WebRequest|Invoke-RestMethod|nc|netcat|ping|nslookup|dig|tracert|netsh)\b'),
    re.compile(r'(?i)\bhttps?://'),
)
_PATH_ESCAPE_PATTERNS = (
    re.compile(r'(?i)(?:[A-Za-z]:[\\/]|\\\\[^\\s]+)'),
    re.compile(r'''(?ix)(?:^|["'\s])/(?:etc|home|root|tmp|var|usr|opt|workspace|mnt|proc|sys)(?:[/\s"']|$)'''),
    re.compile(r'(?i)(?:^|[\"\'\s])\.\.[\\/]'),
    # File/process APIs are denied rather than relying on a caller-provided
    # cwd. This also closes dynamic-import and Node.js module escape hatches.
    re.compile(r'''(?ix)\b(?:__import__|importlib|pathlib|open|io\.open|os\.(?:chdir|listdir|walk|scandir|remove|unlink|rename|replace|makedirs|mkdir|rmdir|system|popen|exec|spawn)|shutil\.|subprocess\.|ctypes\.|winreg\.|tempfile\.)'''),
    re.compile(r'''(?ix)\bos\.environ(?:\b|\[)'''),
    re.compile(r'''(?ix)(?:require\s*\(\s*['"](?:node:)?(?:fs|fs/promises|child_process|module)['"]|from\s+['"](?:node:)?(?:fs|fs/promises|child_process|module)['"]|\bprocess\.(?:binding|dlopen|env|exec|spawn)|\b(?:Deno|Bun)\.)'''),
    re.compile(r'''(?ix)\b(?:type|copy|xcopy|move|del|erase|dir|cat|cp|mv|rm|rmdir|find|grep|dd)\s+[^\n]*[/\\.]'''),
)


def _limit_child_resources(timeout: int) -> None:
    """Apply best-effort OS resource ceilings before starting user code.

    Unix kernels enforce CPU, address-space, file-size and child-process
    limits.  Windows has no stdlib equivalent; its process group is still
    killed recursively on timeout and the packaged runner should be treated
    as a convenience executor, not a hostile-code sandbox.
    """
    if _resource is None:
        return
    cpu = max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS)) + 1
    limits = (
        ('RLIMIT_CPU', cpu),
        ('RLIMIT_AS', MAX_MEMORY_BYTES),
        ('RLIMIT_FSIZE', MAX_FILE_BYTES),
        ('RLIMIT_NPROC', MAX_CHILD_PROCESSES),
    )
    for name, ceiling in limits:
        kind = getattr(_resource, name, None)
        if kind is None:
            continue
        try:
            hard = _resource.getrlimit(kind)[1]
            maximum = ceiling if hard == _resource.RLIM_INFINITY else min(ceiling, hard)
            _resource.setrlimit(kind, (maximum, maximum))
        except (OSError, ValueError):
            # A restricted host may not permit one of the optional limits.
            continue

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
    env = {key: os.environ[key] for key in _SAFE_ENV_KEYS if os.environ.get(key)}
    env['PYTHONIOENCODING'] = 'utf-8'
    env['PYTHONUTF8'] = '1'
    env['NODE_OPTIONS'] = '--no-warnings'
    timeout = max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS))

    try:
        popen_kwargs = {}
        if sys.platform == 'win32':
            popen_kwargs['creationflags'] = getattr(subprocess, 'CREATE_NEW_PROCESS_GROUP', 0)
        else:
            popen_kwargs['start_new_session'] = True
            popen_kwargs['preexec_fn'] = lambda: _limit_child_resources(timeout)
        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding='utf-8',
            errors='replace',
            env=env,
            cwd=cwd,
            **popen_kwargs
        )

        try:
            stdout, stderr = proc.communicate(timeout=timeout)
            exit_code = proc.returncode
        except subprocess.TimeoutExpired:
            if sys.platform == 'win32':
                subprocess.run(['taskkill', '/T', '/F', '/PID', str(proc.pid)],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               check=False)
            else:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except Exception:
                    proc.kill()
            stdout, stderr = proc.communicate()
            return {
                "ok": False,
                "error_code": "execution_timeout",
                "error": f"代码执行超时 (超过 {timeout} 秒限制)",
                "stdout": stdout,
                "stderr": stderr,
                "images": [],
                "exit_code": -1
            }

        truncated = len(stdout) > MAX_OUTPUT_CHARS or len(stderr) > MAX_OUTPUT_CHARS
        return {
            "ok": exit_code == 0,
            "stdout": stdout[:MAX_OUTPUT_CHARS].strip(),
            "stderr": stderr[:MAX_OUTPUT_CHARS].strip(),
            "images": [],
            "exit_code": exit_code,
            "warning": "output_truncated" if truncated else None,
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
                         timeout: int = EXECUTION_TIMEOUT,
                         cwd: Optional[str] = None) -> Dict[str, Any]:
    """安全执行 Python 代码块并捕获文本输出与 Matplotlib 图像。"""
    wrapped_code = MATPLOTLIB_WRAPPER.format(user_code=code) if capture_plot else code

    tmp_script, script_dir = _write_temp_script('.py', wrapped_code, cwd)

    try:
        res = _run_process([sys.executable, tmp_script], cwd=cwd, timeout=timeout)
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
        _cleanup_temp_script(tmp_script, script_dir)


def _allowed_cwd(cwd: Optional[str]) -> Optional[str]:
    """Return a permitted working directory, or None for a fresh sandbox.

    Explicit working directories are restricted to the configured ReadMD data
    root (or the system temporary directory).  This keeps the compatibility
    ``cwd`` argument while preventing arbitrary file-system traversal.
    """
    if not cwd:
        return None
    candidate = os.path.realpath(os.path.abspath(str(cwd)))
    if not os.path.isdir(candidate):
        raise ValueError("cwd_not_found")
    roots = [os.path.realpath(tempfile.gettempdir())]
    configured_root = os.environ.get('READMD_DATA_DIR')
    # ``realpath('')`` resolves to the process cwd.  Never let an unset data
    # root accidentally turn the repository/current directory into an allowed
    # execution workspace.
    if configured_root:
        roots.insert(0, os.path.realpath(configured_root))
    roots = [r for r in roots if r and os.path.isdir(r)]
    if not any(candidate == root or candidate.startswith(root + os.sep) for root in roots):
        raise ValueError("cwd_not_allowed")
    return candidate


def _write_temp_script(suffix: str, content: str, cwd: Optional[str]):
    """Write a transient script in a disposable directory."""
    script_dir = tempfile.mkdtemp(prefix='readmd-script-', dir=cwd or None)
    path = os.path.join(script_dir, 'main' + suffix)
    with open(path, 'w', encoding='utf-8', newline='\n') as handle:
        handle.write(content)
    return path, script_dir


def _cleanup_temp_script(path: Optional[str], script_dir: Optional[str]):
    if script_dir:
        shutil.rmtree(script_dir, ignore_errors=True)
    elif path:
        try:
            os.remove(path)
        except OSError:
            pass


def execute_code_chunk(code: str, lang: str = "python", capture_plot: bool = True,
                       timeout: int = EXECUTION_TIMEOUT,
                       cwd: Optional[str] = None) -> Dict[str, Any]:
    """多语言统一代码块调度执行器。

    The public argument order is intentionally ``code, lang``.  Callers that
    used the old accidental ``lang, code`` order are fixed at their boundary.
    Every invocation runs in a disposable temporary directory unless an
    explicitly allowed ReadMD data/temp directory is supplied.
    """
    source = str(code or '')
    if any(pattern.search(source) for pattern in _NETWORK_PATTERNS):
        return {
            "ok": False, "error": "network_not_allowed", "stdout": "",
            "stderr": "network_not_allowed", "images": [], "exit_code": 1,
            "lang": str(lang or "python")
        }
    if any(pattern.search(source) for pattern in _PATH_ESCAPE_PATTERNS):
        return {
            "ok": False, "error": "path_access_not_allowed", "stdout": "",
            "stderr": "path_access_not_allowed", "images": [], "exit_code": 1,
            "lang": str(lang or "python")
        }
    try:
        explicit_cwd = _allowed_cwd(cwd)
    except ValueError as exc:
        return {
            "ok": False, "error": str(exc), "stdout": "", "stderr": str(exc),
            "images": [], "exit_code": 1, "lang": str(lang or "python")
        }
    sandbox = tempfile.mkdtemp(prefix='readmd-code-')
    run_cwd = explicit_cwd or sandbox
    try:
        return _execute_code_chunk(code, lang=lang, capture_plot=capture_plot,
                                   timeout=timeout, cwd=run_cwd)
    finally:
        shutil.rmtree(sandbox, ignore_errors=True)


def _execute_code_chunk(code: str, lang: str = "python", capture_plot: bool = True,
                        timeout: int = EXECUTION_TIMEOUT,
                        cwd: Optional[str] = None) -> Dict[str, Any]:
    """Internal dispatcher; ``cwd`` has already passed the sandbox gate."""
    normalized_lang = lang.lower().strip().lstrip('.')

    # 1. Python 调度
    if normalized_lang in ('python', 'py'):
        res = execute_python_chunk(code, capture_plot=capture_plot, timeout=timeout, cwd=cwd)
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
        tmp_script, script_dir = _write_temp_script('.js', code, cwd)
        try:
            res = _run_process([node_bin, tmp_script], cwd=cwd, timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            _cleanup_temp_script(tmp_script, script_dir)

    # 3. Shell / Bash / PowerShell 调度
    elif normalized_lang in ('bash', 'sh', 'shell', 'powershell', 'cmd', 'bat'):
        if sys.platform == 'win32':
            cmd = ['powershell', '-Command', code] if normalized_lang == 'powershell' else ['cmd', '/c', code]
        else:
            cmd = ['/bin/bash', '-c', code] if os.path.exists('/bin/bash') else ['/bin/sh', '-c', code]
        res = _run_process(cmd, cwd=cwd, timeout=timeout)
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
        tmp_script, script_dir = _write_temp_script('.R', code, cwd)
        try:
            res = _run_process([r_bin, tmp_script], cwd=cwd, timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            _cleanup_temp_script(tmp_script, script_dir)

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
        # 如果没有 package main，自动包装
        if 'package main' not in code:
            code = f"package main\nimport \"fmt\"\nfunc main() {{\n{code}\n}}"
        tmp_script, script_dir = _write_temp_script('.go', code, cwd)
        try:
            res = _run_process([go_bin, 'run', tmp_script], cwd=cwd, timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            _cleanup_temp_script(tmp_script, script_dir)

    # 7. Rust 脚本化调度
    elif normalized_lang in ('rust', 'rs'):
        rust_script = shutil.which('rust-script')
        if rust_script:
            res = _run_process([rust_script, '-e', code], cwd=cwd, timeout=timeout)
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
        if 'fn main()' not in code:
            code = f"fn main() {{\n{code}\n}}"
        tmp_script, script_dir = _write_temp_script('.rs', code, cwd)
        out_bin = tmp_script[:-3] + ('.exe' if sys.platform == 'win32' else '')
        try:
            c_res = _run_process([rustc_bin, tmp_script, '-o', out_bin], cwd=cwd, timeout=timeout)
            if not c_res['ok'] or c_res['exit_code'] != 0:
                c_res["lang"] = normalized_lang
                return c_res
            res = _run_process([out_bin], cwd=cwd, timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            _cleanup_temp_script(tmp_script, script_dir)

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
        if 'main(' not in code:
            header = "#include <iostream>\nusing namespace std;\n" if suffix == '.cpp' else "#include <stdio.h>\n"
            code = f"{header}int main() {{\n{code}\nreturn 0;\n}}"
        tmp_script, script_dir = _write_temp_script(suffix, code, cwd)
        out_bin = tmp_script[:-len(suffix)] + ('.exe' if sys.platform == 'win32' else '')
        try:
            c_res = _run_process([compiler, tmp_script, '-o', out_bin], cwd=cwd, timeout=timeout)
            if not c_res['ok'] or c_res['exit_code'] != 0:
                c_res["lang"] = normalized_lang
                return c_res
            res = _run_process([out_bin], cwd=cwd, timeout=timeout)
            res["lang"] = normalized_lang
            return res
        finally:
            _cleanup_temp_script(tmp_script, script_dir)

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
