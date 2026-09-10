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
import io
import signal
import threading
import time
import tokenize
from typing import Any, Dict, List, Optional

try:  # Unix-only resource ceilings; Windows uses process-group teardown below.
    import resource as _resource
except ImportError:  # pragma: no cover - exercised on Windows builds
    _resource = None

if sys.platform == 'win32':
    try:
        import ctypes
        from ctypes import wintypes
        _kernel32 = ctypes.windll.kernel32
        _JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE = 0x2000
        _JobObjectExtendedLimitInformation = 9

        _kernel32.CreateJobObjectW.restype = wintypes.HANDLE
        _kernel32.CreateJobObjectW.argtypes = [wintypes.LPVOID, wintypes.LPCWSTR]
        _kernel32.SetInformationJobObject.restype = wintypes.BOOL
        _kernel32.SetInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, wintypes.LPVOID, wintypes.DWORD]
        _kernel32.AssignProcessToJobObject.restype = wintypes.BOOL
        _kernel32.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
        _kernel32.CloseHandle.restype = wintypes.BOOL
        _kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
        _kernel32.TerminateJobObject.restype = wintypes.BOOL
        _kernel32.TerminateJobObject.argtypes = [wintypes.HANDLE, wintypes.UINT]

        class _JOBOBJECT_BASIC_LIMIT_INFORMATION(ctypes.Structure):
            _fields_ = [
                ('PerProcessUserTimeLimit', wintypes.LARGE_INTEGER),
                ('PerJobUserTimeLimit', wintypes.LARGE_INTEGER),
                ('LimitFlags', wintypes.DWORD),
                ('MinimumWorkingSetSize', ctypes.c_size_t),
                ('MaximumWorkingSetSize', ctypes.c_size_t),
                ('ActiveProcessLimit', wintypes.DWORD),
                ('Affinity', ctypes.c_size_t),
                ('PriorityClass', wintypes.DWORD),
                ('SchedulingClass', wintypes.DWORD),
            ]

        class _IO_COUNTERS(ctypes.Structure):
            _fields_ = [
                ('ReadOperationCount', ctypes.c_uint64),
                ('WriteOperationCount', ctypes.c_uint64),
                ('OtherOperationCount', ctypes.c_uint64),
                ('ReadTransferCount', ctypes.c_uint64),
                ('WriteTransferCount', ctypes.c_uint64),
                ('OtherTransferCount', ctypes.c_uint64),
            ]

        class _JOBOBJECT_EXTENDED_LIMIT_INFORMATION(ctypes.Structure):
            _fields_ = [
                ('BasicLimitInformation', _JOBOBJECT_BASIC_LIMIT_INFORMATION),
                ('IoInfo', _IO_COUNTERS),
                ('ProcessMemoryLimit', ctypes.c_size_t),
                ('JobMemoryLimit', ctypes.c_size_t),
                ('PeakProcessMemoryLimit', ctypes.c_size_t),
                ('PeakJobMemoryLimit', ctypes.c_size_t),
            ]

        def _create_job_object():
            job = _kernel32.CreateJobObjectW(None, None)
            if not job:
                return None
            info = _JOBOBJECT_EXTENDED_LIMIT_INFORMATION()
            info.BasicLimitInformation.LimitFlags = _JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            success = _kernel32.SetInformationJobObject(
                job, _JobObjectExtendedLimitInformation,
                ctypes.byref(info), ctypes.sizeof(info)
            )
            if not success:
                _kernel32.CloseHandle(job)
                return None
            return job
    except Exception:
        _kernel32 = None
        _create_job_object = lambda: None
else:
    _kernel32 = None
    _create_job_object = lambda: None


EXECUTION_TIMEOUT = 10  # 最大超时秒数
MAX_OUTPUT_CHARS = 200_000
MAX_TIMEOUT_SECONDS = 10
MAX_MEMORY_BYTES = 512 * 1024 * 1024
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_CHILD_PROCESSES = 32
MAX_SQL_ROWS = 5000

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
    re.compile(r'(?i)(?:(?<![A-Za-z0-9_])[A-Za-z]:[\\/]|\\\\[^\\s]+)'),
    re.compile(r'''(?ix)(?:^|["'\s])/(?:etc|home|root|tmp|var|usr|opt|workspace|mnt|proc|sys)(?:[/\s"']|$)'''),
    re.compile(r'(?i)(?:^|[\"\'\s])\.\.[\\/]'),
    # File/process APIs are denied rather than relying on a caller-provided
    # cwd. This also closes dynamic-import and Node.js module escape hatches.
    re.compile(r'''(?ix)\b(?:__import__|importlib|pathlib|open|io\.open|os\.(?:chdir|listdir|walk|scandir|remove|unlink|rename|replace|makedirs|mkdir|rmdir|system|popen|exec|spawn)|shutil\.|subprocess\.|ctypes\.|winreg\.|tempfile\.)'''),
    re.compile(r'''(?ix)\bos\.environ(?:\b|\[)'''),
    re.compile(r'''(?ix)(?:require\s*\(\s*['"](?:node:)?(?:fs|fs/promises|child_process|module)['"]|from\s+['"](?:node:)?(?:fs|fs/promises|child_process|module)['"]|\bprocess\.(?:binding|dlopen|env|exec|spawn)|\b(?:Deno|Bun)\.)'''),
    re.compile(r'''(?ix)\b(?:type|copy|xcopy|move|del|erase|dir|cat|cp|mv|rm|rmdir|find|grep|dd)\s+[^\n]*[/\\.]'''),
)


def _limit_child_resources(timeout: int, runtime: Optional[str] = None) -> None:
    """Apply best-effort OS resource ceilings before starting user code.

    Unix kernels enforce CPU, address-space, file-size and child-process
    limits.  Windows has no stdlib equivalent; its process group is still
    killed recursively on timeout and the packaged runner should be treated
    as a convenience executor, not a hostile-code sandbox.
    """
    if _resource is None:
        return
    cpu = max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS)) + 1
    limits = [
        ('RLIMIT_CPU', cpu),
        ('RLIMIT_FSIZE', MAX_FILE_BYTES),
        ('RLIMIT_NPROC', MAX_CHILD_PROCESSES),
    ]
    if runtime != 'node':
        limits.append(('RLIMIT_AS', MAX_MEMORY_BYTES))
    elif hasattr(_resource, 'RLIMIT_DATA'):
        limits.append(('RLIMIT_DATA', MAX_MEMORY_BYTES))

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


def _terminate_posix_group(proc):
    if proc is None:
        return
    try:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except OSError:
        if proc.poll() is None:
            try:
                proc.kill()
            except Exception:
                pass
    try:
        proc.wait(timeout=0.5)
    except Exception:
        pass


def _run_process(cmd: List[str], cwd: Optional[str] = None, timeout: int = EXECUTION_TIMEOUT, runtime: Optional[str] = None) -> Dict[str, Any]:
    """底层安全进程调用与 UTF-8 管道捕获（有界流式内存保护与全路径截断）。"""
    env = {key: os.environ[key] for key in _SAFE_ENV_KEYS if os.environ.get(key)}
    env['PYTHONIOENCODING'] = 'utf-8'
    env['PYTHONUTF8'] = '1'
    env['NODE_OPTIONS'] = '--no-warnings --max-old-space-size=128' if runtime == 'node' else '--no-warnings'
    timeout = max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS))

    job = None
    proc = None
    sel = None
    try:
        popen_kwargs = {}
        is_posix = sys.platform != 'win32'
        if is_posix:
            popen_kwargs['start_new_session'] = True
            popen_kwargs['preexec_fn'] = lambda: _limit_child_resources(timeout, runtime=runtime)
        else:
            popen_kwargs['creationflags'] = getattr(subprocess, 'CREATE_NEW_PROCESS_GROUP', 0)
            try:
                job = _create_job_object()
            except Exception:
                job = None

        proc = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=not is_posix,
            encoding=None if is_posix else 'utf-8',
            errors=None if is_posix else 'replace',
            env=env,
            cwd=cwd,
            **popen_kwargs
        )

        if not is_posix and job and _kernel32:
            try:
                proc_handle = getattr(proc, '_handle', None)
                if proc_handle:
                    assigned = _kernel32.AssignProcessToJobObject(job, wintypes.HANDLE(int(proc_handle)))
                    if not assigned:
                        _kernel32.CloseHandle(job)
                        job = None
            except Exception:
                if job:
                    _kernel32.CloseHandle(job)
                    job = None

        if is_posix:
            import codecs
            import selectors
            sel = selectors.DefaultSelector()
            stdout_fd = proc.stdout.fileno()
            stderr_fd = proc.stderr.fileno()
            os.set_blocking(stdout_fd, False)
            os.set_blocking(stderr_fd, False)

            decoders = {
                stdout_fd: codecs.getincrementaldecoder('utf-8')(errors='replace'),
                stderr_fd: codecs.getincrementaldecoder('utf-8')(errors='replace'),
            }

            stdout_chunks: List[str] = []
            stderr_chunks: List[str] = []
            stdout_truncated = [False]
            stderr_truncated = [False]
            stdout_total = 0
            stderr_total = 0

            def _append_text(text: str, stream_name: str):
                nonlocal stdout_total, stderr_total
                if not text:
                    return
                if stream_name == 'stdout':
                    if stdout_total < MAX_OUTPUT_CHARS:
                        rem = MAX_OUTPUT_CHARS - stdout_total
                        stdout_chunks.append(text[:rem])
                        stdout_total += min(len(text), rem)
                        if len(text) > rem:
                            stdout_truncated[0] = True
                    else:
                        stdout_truncated[0] = True
                else:
                    if stderr_total < MAX_OUTPUT_CHARS:
                        rem = MAX_OUTPUT_CHARS - stderr_total
                        stderr_chunks.append(text[:rem])
                        stderr_total += min(len(text), rem)
                        if len(text) > rem:
                            stderr_truncated[0] = True
                    else:
                        stderr_truncated[0] = True

            sel.register(stdout_fd, selectors.EVENT_READ, data='stdout')
            sel.register(stderr_fd, selectors.EVENT_READ, data='stderr')
            open_fds = {stdout_fd, stderr_fd}

            deadline = time.monotonic() + timeout
            timed_out = False
            child_exited = False

            while open_fds:
                now = time.monotonic()
                if now >= deadline:
                    timed_out = True
                    break

                if not child_exited and proc.poll() is not None:
                    child_exited = True
                    deadline = min(deadline, now + 0.1)

                remaining = max(0.0, deadline - time.monotonic())
                events = sel.select(timeout=min(remaining, 0.05))

                if not events and child_exited:
                    break

                for key, _ in events:
                    fd = key.fd
                    stream_name = key.data
                    try:
                        chunk = os.read(fd, 4096)
                    except (OSError, BlockingIOError):
                        chunk = b''

                    if not chunk:
                        try:
                            tail = decoders[fd].decode(b'', final=True)
                            _append_text(tail, stream_name)
                        except Exception:
                            pass
                        try:
                            sel.unregister(fd)
                        except Exception:
                            pass
                        open_fds.discard(fd)
                        continue

                    text_chunk = decoders[fd].decode(chunk, final=False)
                    _append_text(text_chunk, stream_name)

            for fd, stream_name in ((stdout_fd, 'stdout'), (stderr_fd, 'stderr')):
                if fd in open_fds:
                    try:
                        tail = decoders[fd].decode(b'', final=True)
                        _append_text(tail, stream_name)
                    except Exception:
                        pass

            try:
                sel.close()
            except Exception:
                pass

            if not timed_out and proc.poll() is None:
                rem = max(0.0, deadline - time.monotonic())
                try:
                    proc.wait(timeout=rem)
                except subprocess.TimeoutExpired:
                    timed_out = True

            if timed_out or proc.poll() is None:
                _terminate_posix_group(proc)
                exit_code = -1
            else:
                exit_code = proc.poll()

            try:
                proc.stdout.close()
            except Exception:
                pass
            try:
                proc.stderr.close()
            except Exception:
                pass

            stdout = "".join(stdout_chunks)
            stderr = "".join(stderr_chunks)

        else:
            stdout_chunks: List[str] = []
            stderr_chunks: List[str] = []
            stdout_truncated = [False]
            stderr_truncated = [False]

            def _bounded_reader(stream, chunks: List[str], trunc_flag: List[bool], limit: int):
                total = 0
                try:
                    while True:
                        chunk = stream.read(4096)
                        if not chunk:
                            break
                        if total < limit:
                            rem = limit - total
                            chunks.append(chunk[:rem])
                            total += min(len(chunk), rem)
                            if len(chunk) > rem:
                                trunc_flag[0] = True
                        else:
                            trunc_flag[0] = True
                except Exception:
                    pass
                finally:
                    try:
                        stream.close()
                    except Exception:
                        pass

            t_out = threading.Thread(target=_bounded_reader, args=(proc.stdout, stdout_chunks, stdout_truncated, MAX_OUTPUT_CHARS))
            t_err = threading.Thread(target=_bounded_reader, args=(proc.stderr, stderr_chunks, stderr_truncated, MAX_OUTPUT_CHARS))
            t_out.daemon = True
            t_err.daemon = True
            t_out.start()
            t_err.start()

            timed_out = False
            try:
                exit_code = proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                exit_code = -1
            except Exception:
                exit_code = -1

            # 无论正常退出还是超时异常，彻底清理进程树与关闭管道
            if job and _kernel32:
                try:
                    _kernel32.TerminateJobObject(job, 0)
                except Exception:
                    pass
                try:
                    _kernel32.CloseHandle(job)
                    job = None
                except Exception:
                    pass
            try:
                if proc.poll() is None:
                    subprocess.run(['taskkill', '/T', '/F', '/PID', str(proc.pid)],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   check=False)
            except Exception:
                pass

            t_out.join(timeout=1.0)
            t_err.join(timeout=1.0)

            if not t_out.is_alive():
                try:
                    if proc.stdout and not proc.stdout.closed:
                        proc.stdout.close()
                except Exception:
                    pass
            if not t_err.is_alive():
                try:
                    if proc.stderr and not proc.stderr.closed:
                        proc.stderr.close()
                except Exception:
                    pass

            stdout = "".join(stdout_chunks)
            stderr = "".join(stderr_chunks)

        if timed_out:
            return {
                "ok": False,
                "error_code": "execution_timeout",
                "error": f"代码执行超时 (超过 {timeout} 秒限制)",
                "stdout": stdout[:MAX_OUTPUT_CHARS].strip(),
                "stderr": stderr[:MAX_OUTPUT_CHARS].strip(),
                "images": [],
                "exit_code": -1
            }

        truncated = stdout_truncated[0] or stderr_truncated[0] or len(stdout) > MAX_OUTPUT_CHARS or len(stderr) > MAX_OUTPUT_CHARS
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
    finally:
        if sys.platform != 'win32' and proc is not None:
            _terminate_posix_group(proc)
            for stream in (proc.stdout, proc.stderr):
                if stream is not None:
                    try:
                        stream.close()
                    except Exception:
                        pass
        if sel is not None:
            try:
                sel.close()
            except Exception:
                pass
        if job and _kernel32:
            try:
                _kernel32.CloseHandle(job)
            except Exception:
                pass



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
    try:
        path = os.path.join(script_dir, 'main' + suffix)
        with open(path, 'w', encoding='utf-8', newline='\n') as handle:
            handle.write(content)
        return path, script_dir
    except Exception:
        shutil.rmtree(script_dir, ignore_errors=True)
        raise


def _cleanup_temp_script(path: Optional[str], script_dir: Optional[str]):
    if script_dir:
        shutil.rmtree(script_dir, ignore_errors=True)
    elif path:
        try:
            os.remove(path)
        except OSError:
            pass


def _strip_literals_and_comments(source: str, lang: str = "python") -> str:
    norm_lang = str(lang or "python").lower().strip().lstrip('.')
    if norm_lang in ('python', 'py'):
        try:
            tokens = tokenize.tokenize(io.BytesIO(source.encode('utf-8')).readline)
            parts = []
            for tok in tokens:
                if tok.type in (tokenize.STRING, tokenize.COMMENT):
                    parts.append('\n' * tok.string.count('\n'))
                else:
                    parts.append(tok.string)
            return ' '.join(parts)
        except Exception:
            pass

    def _replacer(match):
        s = match.group(0)
        return '\n' * s.count('\n')

    pattern = re.compile(
        r'/\*[\s\S]*?\*/|//[^\n]*|#[^\n]*|'
        r'"(?:\\.|[^"\\])*"|'
        r"'(?:\\.|[^'\\])*'|"
        r'`(?:\\.|[^`\\])*`',
        re.MULTILINE
    )
    return pattern.sub(_replacer, source)


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
    cleaned_source = _strip_literals_and_comments(source, lang=lang)
    if any(pattern.search(cleaned_source) for pattern in _NETWORK_PATTERNS):
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


def _split_sql_statements(sql: str) -> List[str]:
    """Split SQL script into statements respecting quoted strings, escapes, and comments."""
    statements: List[str] = []
    current: List[str] = []
    in_single_quote = False
    in_double_quote = False
    in_backtick = False
    in_line_comment = False
    in_block_comment = False
    i = 0
    n = len(sql)

    while i < n:
        ch = sql[i]
        nxt = sql[i + 1] if i + 1 < n else ''

        if in_line_comment:
            current.append(ch)
            if ch == '\n':
                in_line_comment = False
            i += 1
            continue

        if in_block_comment:
            current.append(ch)
            if ch == '*' and nxt == '/':
                current.append(nxt)
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue

        if in_single_quote:
            current.append(ch)
            if ch == "'":
                if nxt == "'":
                    current.append(nxt)
                    i += 2
                    continue
                in_single_quote = False
            i += 1
            continue

        if in_double_quote:
            current.append(ch)
            if ch == '"':
                if nxt == '"':
                    current.append(nxt)
                    i += 2
                    continue
                in_double_quote = False
            i += 1
            continue

        if in_backtick:
            current.append(ch)
            if ch == '`':
                in_backtick = False
            i += 1
            continue

        if ch == '-' and nxt == '-':
            current.append(ch)
            current.append(nxt)
            in_line_comment = True
            i += 2
            continue
        elif ch == '/' and nxt == '*':
            current.append(ch)
            current.append(nxt)
            in_block_comment = True
            i += 2
            continue
        elif ch == "'":
            current.append(ch)
            in_single_quote = True
            i += 1
            continue
        elif ch == '"':
            current.append(ch)
            in_double_quote = True
            i += 1
            continue
        elif ch == '`':
            current.append(ch)
            in_backtick = True
            i += 1
            continue
        elif ch == ';':
            stmt = "".join(current).strip()
            if stmt:
                statements.append(stmt)
            current = []
            i += 1
            continue
        else:
            current.append(ch)
            i += 1

    stmt = "".join(current).strip()
    if stmt:
        statements.append(stmt)
    return statements


def _build_sql_runner_source() -> str:
    """生成隔离子进程执行 SQL 的轻量脚本源码。"""
    return f'''# -*- coding: utf-8 -*-
import sqlite3
import sys
import os
from typing import List

MAX_OUTPUT_CHARS = {MAX_OUTPUT_CHARS}
MAX_SQL_ROWS = {MAX_SQL_ROWS}
MAX_CELL_CHARS = 500

def _split_sql_statements(sql: str) -> List[str]:
    statements = []
    current = []
    in_single_quote = False
    in_double_quote = False
    in_backtick = False
    in_line_comment = False
    in_block_comment = False
    i = 0
    n = len(sql)
    while i < n:
        ch = sql[i]
        nxt = sql[i + 1] if i + 1 < n else ''
        if in_line_comment:
            current.append(ch)
            if ch == '\\n':
                in_line_comment = False
            i += 1
            continue
        if in_block_comment:
            current.append(ch)
            if ch == '*' and nxt == '/':
                current.append(nxt)
                in_block_comment = False
                i += 2
                continue
            i += 1
            continue
        if in_single_quote:
            current.append(ch)
            if ch == "'":
                if nxt == "'":
                    current.append(nxt)
                    i += 2
                    continue
                in_single_quote = False
            i += 1
            continue
        if in_double_quote:
            current.append(ch)
            if ch == '"':
                if nxt == '"':
                    current.append(nxt)
                    i += 2
                    continue
                in_double_quote = False
            i += 1
            continue
        if in_backtick:
            current.append(ch)
            if ch == '`':
                in_backtick = False
            i += 1
            continue
        if ch == '-' and nxt == '-':
            current.append(ch)
            current.append(nxt)
            in_line_comment = True
            i += 2
            continue
        elif ch == '/' and nxt == '*':
            current.append(ch)
            current.append(nxt)
            in_block_comment = True
            i += 2
            continue
        elif ch == "'":
            current.append(ch)
            in_single_quote = True
            i += 1
            continue
        elif ch == '"':
            current.append(ch)
            in_double_quote = True
            i += 1
            continue
        elif ch == '`':
            current.append(ch)
            in_backtick = True
            i += 1
            continue
        elif ch == ';':
            stmt = ''.join(current).strip()
            if stmt:
                statements.append(stmt)
            current = []
            i += 1
            continue
        current.append(ch)
        i += 1
    last = ''.join(current).strip()
    if last:
        statements.append(last)
    return statements

def format_cell(val):
    if val is None:
        return "NULL"
    if isinstance(val, (bytes, bytearray, memoryview)):
        return f"<BLOB {{len(val)}} bytes>"
    s = str(val)
    if len(s) > MAX_CELL_CHARS:
        return s[:MAX_CELL_CHARS - 3] + "..."
    return s

def run_sql():
    sql_file = os.path.join(os.path.dirname(__file__), "query.sql")
    with open(sql_file, "r", encoding="utf-8") as f:
        code = f.read()

    con = sqlite3.connect(":memory:")
    cur = con.cursor()
    statements = _split_sql_statements(code)
    results = []
    any_truncated = False

    try:
        for stmt in statements:
            if not stmt.strip():
                continue
            cur.execute(stmt)
            if cur.description:
                headers = [d[0] for d in cur.description]
                buffered_rows = []
                truncated_rows = False
                col_widths = [len(h) for h in headers]
                total_row_chars = 0

                while True:
                    batch = cur.fetchmany(100)
                    if not batch:
                        break
                    for row in batch:
                        formatted = [format_cell(v) for v in row]
                        for idx, cell in enumerate(formatted):
                            if len(cell) > col_widths[idx]:
                                col_widths[idx] = len(cell)
                        buffered_rows.append(formatted)
                        total_row_chars += sum(len(c) for c in formatted) + len(formatted) * 3
                        if len(buffered_rows) >= MAX_SQL_ROWS or total_row_chars >= MAX_OUTPUT_CHARS:
                            truncated_rows = True
                            any_truncated = True
                            break
                    if truncated_rows:
                        break

                header_line = " | ".join(h.ljust(col_widths[i]) for i, h in enumerate(headers))
                sep_line = "-+-".join("-" * col_widths[i] for i in range(len(headers)))
                row_lines = [" | ".join(val.ljust(col_widths[i]) for i, val in enumerate(r)) for r in buffered_rows]
                table_str = f"{{header_line}}\\n{{sep_line}}\\n" + "\\n".join(row_lines)
                if truncated_rows:
                    table_str += f"\\n[Output truncated at {{len(buffered_rows)}} rows]"
                results.append(table_str)
            else:
                results.append(f"Query OK, {{cur.rowcount}} rows affected.")
        con.commit()
    except sqlite3.OperationalError as e:
        sys.stderr.write(str(e))
        sys.exit(1)
    except Exception as e:
        sys.stderr.write(str(e))
        sys.exit(1)
    finally:
        try:
            con.close()
        except Exception:
            pass

    out_text = "\\n\\n".join(results)
    if any_truncated or len(out_text) > MAX_OUTPUT_CHARS:
        sys.stderr.write("__READMD_SQL_TRUNCATED__\\n")
    sys.stdout.write(out_text[:MAX_OUTPUT_CHARS])

if __name__ == '__main__':
    run_sql()
'''


def _execute_sql_chunk(code: str, cwd: Optional[str] = None, timeout: int = EXECUTION_TIMEOUT) -> Dict[str, Any]:
    """隔离子进程安全执行 SQL 并进行单元格与累积字符有界流式保护。"""
    timeout = max(1, min(int(timeout or EXECUTION_TIMEOUT), MAX_TIMEOUT_SECONDS))
    tmp_script, script_dir = _write_temp_script('.py', _build_sql_runner_source(), cwd)
    try:
        sql_path = os.path.join(script_dir, 'query.sql')
        with open(sql_path, 'w', encoding='utf-8') as f:
            f.write(code)

        res = _run_process([sys.executable, tmp_script], cwd=cwd, timeout=timeout)
        res["lang"] = "sql"
        if not res["ok"]:
            if res.get("error_code") == "execution_timeout":
                res["error"] = f"SQL 执行超时 (超过 {timeout} 秒限制)"
            elif not res.get("error"):
                res["error"] = f"SQL 执行错误: {res.get('stderr') or 'Unknown error'}"
            return res

        stderr = res.get("stderr", "")
        truncated = "__READMD_SQL_TRUNCATED__" in stderr or res.get("warning") == "output_truncated"
        if "__READMD_SQL_TRUNCATED__" in stderr:
            res["stderr"] = stderr.replace("__READMD_SQL_TRUNCATED__", "").strip()
        if truncated:
            res["warning"] = "output_truncated"
        return res
    finally:
        _cleanup_temp_script(tmp_script, script_dir)


execute_sql_chunk = _execute_sql_chunk


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
            res = _run_process([node_bin, tmp_script], cwd=cwd, timeout=timeout, runtime='node')
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
        return _execute_sql_chunk(code, cwd=cwd, timeout=timeout)


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
