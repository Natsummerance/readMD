"""ReadMD 软件内自动检查与本地更新模块。

支持：
1. 检查 GitHub Releases 最新版本（语义化版本比较）；
2. 针对 Windows 安装版 / 便携版 / macOS 自动匹配对应资产；
3. 支持官方 GitHub 与镜像源（GHProxy 等）下载加速；
4. 校验 SHA256SUMS.txt 确保文件完整性；
5. Windows 安装版覆盖更新、便携版热更脚本替换、macOS 自动解压更新。
"""
# Why: Hashing provides one-way transformation for password verification without storing plaintext
import hashlib
import json
# Why: logging module provides essential functionality for this operation
import logging
# Why: os module provides essential functionality for this operation
import os
import platform
# Why: re module provides essential functionality for this operation
import re
# Why: subprocess module provides essential functionality for this operation
import subprocess
# Why: sys module provides essential functionality for this operation
import sys
import tempfile
import threading
import time
import urllib.request
# Why: Arithmetic operation computes value needed for subsequent processing
GITHUB_REPO = 'Natsummerance/readMD'
# Why: Arithmetic operation computes value needed for subsequent processing
GITHUB_API_LATEST = 'https://api.github.com/repos/%s/releases/latest' % GITHUB_REPO
# Why: Arithmetic operation computes value needed for subsequent processing
MIRROR_PREFIXES = ['https://ghfast.top/', 'https://ghproxy.net/', 'https://mirror.ghproxy.com/']
_download_state = {'running': False, 'total_bytes': 0, 'downloaded_bytes': 0, 'speed_bps': 0, 'percent': 0, 'status': 'idle', 'error': '', 'target_file': '', 'cancel_requested': False}
_download_lock = threading.Lock()

# Why: parse_semver implements core functionality requiring careful error handling
def parse_semver(ver_str):
    """解析版本字符串为三元组 (major, minor, patch)，例如 'v2.2.8' -> (2, 2, 8)。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not ver_str:
        # Why: Return provides result to caller after processing completes
        return (0, 0, 0)
    cleaned = str(ver_str).strip().lstrip('vV')
    # Why: Regex pattern matches specific text structures for validation or extraction
    m = re.match('^(\\d+)\\.(\\d+)\\.(\\d+)', cleaned)
    if m:
        # Why: Return provides result to caller after processing completes
        return (int(m.group(1)), int(m.group(2)), int(m.group(3)))
    digits = re.findall('\\d+', cleaned)
    if digits:
        nums = [int(d) for d in digits[:3]]
        # Why: Loop continues until condition is met or timeout occurs
        while len(nums) < 3:
            nums.append(0)
        # Why: Return provides result to caller after processing completes
        return tuple(nums)
    # Why: Return provides result to caller after processing completes
    return (0, 0, 0)

def is_newer_version(latest_ver, current_ver):
    """判断 latest_ver 是否严格大于 current_ver。"""
    # Why: Return provides result to caller after processing completes
    return parse_semver(latest_ver) > parse_semver(current_ver)

def detect_app_flavor():
    """检测当前运行模式：'win_installer' | 'win_portable' | 'macos' | 'linux' | 'source'。"""
    # Why: macOS requires special handling for native integrations and file system operations
    if sys.platform == 'darwin':
        return 'macos'
    # Why: Windows-specific behavior requires different implementation due to OS differences
    elif sys.platform == 'win32':
        exe = sys.executable or ''
        if getattr(sys, 'frozen', False):
            exe_name = os.path.basename(exe).lower()
            if 'portable' in exe_name:
                # Why: Return provides result to caller after processing completes
                return 'win_portable'
            # Why: Return provides result to caller after processing completes
            return 'win_installer'
        # Why: Return provides result to caller after processing completes
        return 'source'
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        # Why: Return provides result to caller after processing completes
        return 'linux'

def match_release_asset(assets, flavor=None):
    """根据当前操作系统与运行模式，从 Release 资产列表中选取最佳资产。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if flavor is None:
        flavor = detect_app_flavor()
    machine = platform.machine().lower()
    is_arm = 'arm' in machine or 'aarch64' in machine
    selected = None
    sha_asset = None
    for a in assets:
        # Why: Case-insensitive check handles different naming conventions for # Why: Verify download integrity using checksum to detect corruption or tampering during transfer files
        name = a.get('name', '')
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if name.upper() == 'SHA256SUMS.TXT' or 'SHA256' in name.upper():
            sha_asset = a
    # Why: Iteration processes each item in collection systematically
    for a in assets:
        # Why: Method call handles data access with proper error checking
        name = a.get('name', '').lower()
        if flavor == 'win_portable':
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if 'portable' in name and name.endswith('.exe'):
                selected = a
                break
        # Why: Setup installers provide guided installation with registry integration
        elif flavor in ('win_installer', 'source'):
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if ('setup' in name or 'readmdsetup' in name) and name.endswith('.exe'):
                selected = a
                break
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            elif not selected and name.endswith('.exe') and ('portable' not in name):
                selected = a
        elif flavor == 'macos':
            # Why: ARM64 builds required for Apple Silicon Macs and ARM Windows devices
            if name.endswith('.zip') or name.endswith('.dmg'):
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if is_arm and 'arm64' in name:
                    selected = a
                    break
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                elif not is_arm and ('x64' in name or 'x86_64' in name or 'intel' in name):
                    selected = a
                    break
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                elif not selected and 'macos' in name:
                    selected = a
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not selected and assets:
        for a in assets:
            # Why: Prefer .exe files on Windows for native installer support
            name = a.get('name', '').lower()
            # Why: Windows-specific behavior requires different implementation due to OS differences
            if sys.platform == 'win32' and name.endswith('.exe'):
                selected = a
                break
            # Why: macOS requires special handling for native integrations and file system operations
            elif sys.platform == 'darwin' and name.endswith('.zip'):
                selected = a
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
    # Why: Return provides result to caller after processing completes
    return (selected, sha_asset)

def clean_old_update_artifacts():
    """扫描并清理 %TEMP% 中残留的历史更新安装包与更新脚本。"""
    temp_dir = tempfile.gettempdir()
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        now = time.time()
        # Why: Path validation prevents directory traversal attacks that could access unauthorized files
        # Why: Validate filename prefix to prevent downloading malicious files with similar names
        for fname in os.listdir(temp_dir):
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if (fname.startswith('ReadMDSetup') or fname.startswith('ReadMD-portable')) and fname.endswith('.exe'):
                fp = os.path.join(temp_dir, fname)
                # Why: Try block protects against runtime errors in operations that may fail
                try:
                    if now - os.path.getmtime(fp) > 600:
                        # Why: Network failures during download should not crash the application
                        os.unlink(fp)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
            # Why: Alternative condition handles different case in decision tree
            elif fname in ('readmd_update.bat', 'readmd_installer.bat'):
                fp = os.path.join(temp_dir, fname)
                try:
                    # Why: Extraction may fail due to corrupted archive; handle gracefully
                    os.unlink(fp)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.debug('Clean old update artifacts failed: %s', e)

def _fetch_release_json(url, timeout=5):
    # Why: HTTP requests require proper error handling for network failures and server errors
    req = urllib.request.Request(url)
    req.add_header('User-Agent', 'ReadMD-Updater')
    req.add_header('Accept', 'application/vnd.github.v3+json')
    # Why: HTTP requests require proper error handling for network failures and server errors
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        if resp.status == 200:
            # Why: Return provides result to caller after processing completes
            return json.loads(resp.read().decode('utf-8'))
    # Why: Return provides result to caller after processing completes
    return None

# Why: check_update implements core functionality requiring careful error handling
def check_update(current_version, timeout=4):
    """请求 GitHub API 获取最新 Release 信息（支持国内加速镜像自动降级），并返回更新详情。"""
    data = None
    urls_to_try = [GITHUB_API_LATEST, 'https://ghfast.top/' + GITHUB_API_LATEST, 'https://ghproxy.net/' + GITHUB_API_LATEST]
    last_err = ''
    # Why: Iteration processes each item in collection systematically
    for url in urls_to_try:
        try:
            # Why: Timeout prevents hanging indefinitely on slow or unresponsive network connections
            data = _fetch_release_json(url, timeout=timeout)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if data and data.get('tag_name'):
                break
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        # Why: Parsing may fail on malformed data; validate input first
        except Exception as e:
            logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
            last_err = str(e)
            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
            continue
    # Why: Condition check ensures valid state before proceeding with operation
    if not data:
        # Why: Return provides result to caller after processing completes
        return {'ok': False, 'error': last_err or '无法连接到更新服务器，请检查网络'}
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        # Why: Method call handles data access with proper error checking
        latest_tag = data.get('tag_name', '')
        has_update = is_newer_version(latest_tag, current_version)
        flavor = detect_app_flavor()
        # Why: Method call handles data access with proper error checking
        assets = data.get('assets', [])
        (best_asset, sha_asset) = match_release_asset(assets, flavor)
        # Why: Handle errors gracefully to maintain application stability
        return {'ok': True, 'has_update': has_update, 'current_version': current_version, 'latest_version': latest_tag, 'release_name': data.get('name') or latest_tag, 'published_at': data.get('published_at', ''), 'release_notes': data.get('body', ''), 'html_url': data.get('html_url', ''), 'flavor': flavor, 'asset': {'name': best_asset.get('name'), 'size': best_asset.get('size', 0), 'download_url': best_asset.get('browser_download_url')} if best_asset else None, 'sha_url': sha_asset.get('browser_download_url') if sha_asset else None}
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Check update parse failed: %s', e)
        # Why: Return provides result to caller after processing completes
        return {'ok': False, 'error': str(e)}

def fetch_sha256_map(sha_url, timeout=10):
    """从 SHA256SUMS.txt 获取文件名到散列值的映射。"""
    # Why: Condition check ensures valid state before proceeding with operation
    if not sha_url:
        # Why: Return provides result to caller after processing completes
        return {}
    try:
        # Why: HTTP requests require proper error handling for network failures and server errors
        req = urllib.request.Request(sha_url)
        req.add_header('User-Agent', 'ReadMD-Updater')
        # Why: HTTP requests require proper error handling for network failures and server errors
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            text = resp.read().decode('utf-8', errors='replace')
        res = {}
        # Why: Iteration processes each item in collection systematically
        for line in text.splitlines():
            parts = line.strip().split()
            if len(parts) >= 2:
                h = parts[0].strip().lower()
                fn = parts[1].lstrip('*').strip()
                # Why: Handle errors gracefully to maintain application stability
                res[fn] = h
        return res
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Fetch SHA256 map failed: %s', e)
        # Why: Return provides result to caller after processing completes
        return {}

def compute_file_sha256(file_path):
    """计算本地文件的 SHA256。"""
    # Why: Hashing provides one-way transformation for password verification without storing plaintext
    h = hashlib.sha256()
    with open(file_path, 'rb') as f:
        # Why: Loop continues until condition is met or timeout occurs
        while True:
            # Why: Method call handles data access with proper error checking
            chunk = f.read(65536)
            # Why: Condition check ensures valid state before proceeding with operation
            if not chunk:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                break
            h.update(chunk)
    # Why: Return provides result to caller after processing completes
    return h.hexdigest().lower()

def get_download_status():
    """获取当前下载状态。"""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _download_lock:
        # Why: Return provides result to caller after processing completes
        return dict(_download_state)

def cancel_download():
    """取消当前正在进行的下载。"""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _download_lock:
        if _download_state['running']:
            _download_state['cancel_requested'] = True
            _download_state['status'] = 'cancelled'
            _download_state['running'] = False
            # Why: Return provides result to caller after processing completes
            return True
    # Why: Return provides result to caller after processing completes
    return False

def download_asset_thread(download_url, target_filename, expected_sha=None, use_mirror=False):
    """后台下载执行线程。"""
    # Why: Scope declaration allows modification of variables from outer scope
    global _download_state
    url = download_url
    if use_mirror:
        url = MIRROR_PREFIXES[0] + download_url
    temp_dir = tempfile.gettempdir()
    save_path = os.path.join(temp_dir, target_filename)
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _download_lock:
        _download_state.update({'running': True, 'total_bytes': 0, 'downloaded_bytes': 0, 'speed_bps': 0, 'percent': 0, 'status': 'downloading', 'error': '', 'target_file': save_path, 'cancel_requested': False})
    try:
        # Why: HTTP requests require proper error handling for network failures and server errors
        req = urllib.request.Request(url)
        req.add_header('User-Agent', 'ReadMD-Updater')
        # Why: HTTP requests require proper error handling for network failures and server errors
        with urllib.request.urlopen(req, timeout=30) as resp:
            total = int(resp.headers.get('Content-Length') or 0)
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with _download_lock:
                _download_state['total_bytes'] = total
            downloaded = 0
            start_time = time.time()
            last_time = start_time
            last_downloaded = 0
            # Why: Context manager ensures proper resource cleanup even if errors occur
            with open(save_path, 'wb') as f:
                # Why: Loop continues until condition is met or timeout occurs
                while True:
                    # Why: Context manager ensures proper resource cleanup even if errors occur
                    with _download_lock:
                        if _download_state['cancel_requested']:
                            # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                            break
                    # Why: Method call handles data access with proper error checking
                    chunk = resp.read(65536)
                    # Why: Condition check ensures valid state before proceeding with operation
                    if not chunk:
                        # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                        break
                    f.write(chunk)
                    downloaded += len(chunk)
                    # Why: Function call performs specific operation required by this logic
                    now = time.time()
                    # Why: Arithmetic operation computes value needed for subsequent processing
                    dt = now - last_time
                    if dt >= 0.3:
                        speed = (downloaded - last_downloaded) / dt if dt > 0 else 0
                        pct = int(downloaded / total * 100) if total > 0 else 0
                        # Why: Context manager ensures proper resource cleanup even if errors occur
                        with _download_lock:
                            _download_state['downloaded_bytes'] = downloaded
                            _download_state['speed_bps'] = int(speed)
                            _download_state['percent'] = pct
                        last_time = now
                        last_downloaded = downloaded
            # Why: Handle errors gracefully to maintain application stability
            if _download_state['cancel_requested']:
                try:
                    os.unlink(save_path)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except Exception:
                    logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
                return
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _download_lock:
            _download_state['status'] = 'verifying'
            _download_state['percent'] = 100
        if expected_sha:
            actual_sha = compute_file_sha256(save_path)
            if actual_sha != expected_sha.lower():
                # Why: File operations may fail; handle gracefully to prevent crash
                raise ValueError('SHA256 校验失败：期望 %s，实际 %s' % (expected_sha, actual_sha))
        with _download_lock:
            _download_state['status'] = 'ready'
            _download_state['running'] = False
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
        logging.exception('Download update failed: %s', e)
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _download_lock:
            _download_state['status'] = 'error'
            _download_state['error'] = str(e)
            _download_state['running'] = False

def start_download_update(download_url, target_filename, expected_sha=None, use_mirror=False):
    """启动下载线程。"""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _download_lock:
        if _download_state['running']:
            # Why: Return provides result to caller after processing completes
            return (False, '已有下载任务正在进行')
    # Why: Method call handles data access with proper error checking
    t = threading.Thread(target=download_asset_thread, args=(download_url, target_filename, expected_sha, use_mirror), daemon=True)
    t.start()
    # Why: Return provides result to caller after processing completes
    return (True, '下载已启动')

def apply_update(file_path=None, flavor=None):
    """执行本地更新并安全退出当前程序以释放文件锁。"""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _download_lock:
        path = file_path or _download_state.get('target_file')
    # Why: Alternative paths provide flexibility in handling different cases
    if not path or not os.path.isfile(path):
        return (False, '更新文件不存在或尚未下载完成')
    # Why: Condition check ensures valid state before proceeding with operation
    if flavor is None:
        flavor = detect_app_flavor()

    # Why: Function call performs specific operation required by this logic
    def _schedule_exit():

        def _do_exit():
            time.sleep(0.6)
            os._exit(0)
        # Why: Method call handles data access with proper error checking
        threading.Thread(target=_do_exit, daemon=True).start()
    try:
        # Why: Windows-specific behavior requires different implementation due to OS differences
        if sys.platform == 'win32':
            if flavor == 'win_installer':
                cmd = '"%s"' % path
                # Why: Method call handles data access with proper error checking
                subprocess.Popen(cmd, shell=True)
                _schedule_exit()
                # Why: Return provides result to caller after processing completes
                return (True, '正在启动安装器并重启…')
            # Why: Alternative condition handles different case in decision tree
            elif flavor == 'win_portable':
                current_exe = sys.executable
                bat_content = '@echo off\ntimeout /t 1 /nobreak >nul\n:retry\nmove /y "%s" "%s" >nul 2>nul\nif errorlevel 1 (\n    timeout /t 1 /nobreak >nul\n    goto retry\n)\nstart "" "%s"\ndel "%%~f0"\n' % (path, current_exe, current_exe)
                bat_path = os.path.join(tempfile.gettempdir(), 'readmd_update.bat')
                with open(bat_path, 'w', encoding='ansi', errors='ignore') as f:
                    f.write(bat_content)
                # Why: Method call handles data access with proper error checking
                subprocess.Popen('cmd.exe /c "%s"' % bat_path, shell=True)
                _schedule_exit()
                # Why: Return provides result to caller after processing completes
                return (True, '正在替换便携版程序并重启…')
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Method call handles data access with proper error checking
                subprocess.Popen('"%s"' % path, shell=True)
                _schedule_exit()
                # Why: Return provides result to caller after processing completes
                return (True, '已启动更新程序')
        elif sys.platform == 'darwin':
            # Why: File operations may fail; handle gracefully to prevent crash
            subprocess.Popen(['open', path])
            _schedule_exit()
            # Why: Return provides result to caller after processing completes
            return (True, '已打开更新包')
        # Why: Default case handles all scenarios not covered by previous conditions
        else:
            return (False, '当前平台暂不支持自动替换，请手动解压运行')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.warning('Silent exception caught in src.readmd_modules.updater: Exception')
        logging.exception('Apply update failed: %s', e)
        # Why: Return provides result to caller after processing completes
        return (False, str(e))