# -*- coding: utf-8 -*-
"""ReadMD 软件内自动检查与本地更新模块。

支持：
1. 检查 GitHub Releases 最新版本（语义化版本比较）；
2. 针对 Windows 安装版 / 便携版 / macOS 自动匹配对应资产；
3. 支持官方 GitHub 与镜像源（GHProxy 等）下载加速；
4. 校验 SHA256SUMS.txt 确保文件完整性；
5. Windows 安装版覆盖更新、便携版热更脚本替换、macOS 自动解压更新。
"""

import hashlib
import json
import logging
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from urllib.parse import urlparse

GITHUB_REPO = 'Natsummerance/readMD'
GITHUB_API_LATEST = f'https://api.github.com/repos/{GITHUB_REPO}/releases/latest'

# 常用开源 GitHub 加速镜像前缀（仅在用户选择或网络重试时使用）
MIRROR_PREFIXES = [
    'https://ghfast.top/',
    'https://ghproxy.net/',
    'https://mirror.ghproxy.com/',
]

_download_state = {
    'running': False,
    'total_bytes': 0,
    'downloaded_bytes': 0,
    'speed_bps': 0,
    'percent': 0,
    'status': 'idle',  # idle | downloading | verifying | ready | error | cancelled
    'error': '',
    'target_file': '',
    'cancel_requested': False,
}
_download_lock = threading.Lock()


def parse_semver(ver_str):
    """解析版本字符串为三元组 (major, minor, patch)，例如 'v2.2.8' -> (2, 2, 8)。"""
    if not ver_str:
        return (0, 0, 0)
    cleaned = str(ver_str).strip().lstrip('vV')
    m = re.match(r'^(\d+)\.(\d+)\.(\d+)', cleaned)
    if m:
        return (int(m.group(1)), int(m.group(2)), int(m.group(3)))
    digits = re.findall(r'\d+', cleaned)
    if digits:
        nums = [int(d) for d in digits[:3]]
        while len(nums) < 3:
            nums.append(0)
        return tuple(nums)
    return (0, 0, 0)


def is_newer_version(latest_ver, current_ver):
    """判断 latest_ver 是否严格大于 current_ver。"""
    return parse_semver(latest_ver) > parse_semver(current_ver)


def detect_app_flavor():
    """检测当前运行模式：'win_installer' | 'win_portable' | 'macos' | 'linux' | 'source'。"""
    if sys.platform == 'darwin':
        return 'macos'
    elif sys.platform == 'win32':
        exe = sys.executable or ''
        if getattr(sys, 'frozen', False):
            # PyInstaller 运行环境
            exe_name = os.path.basename(exe).lower()
            if 'portable' in exe_name:
                return 'win_portable'
            return 'win_installer'
        return 'source'
    else:
        return 'linux'


def match_release_asset(assets, flavor=None):
    """根据当前操作系统与运行模式，从 Release 资产列表中选取最佳资产。"""
    if flavor is None:
        flavor = detect_app_flavor()

    machine = platform.machine().lower()
    is_arm = ('arm' in machine or 'aarch64' in machine)

    selected = None
    sha_asset = None

    for a in assets:
        name = a.get('name', '')
        if name.upper() == 'SHA256SUMS.TXT' or 'SHA256' in name.upper():
            sha_asset = a

    for a in assets:
        name = a.get('name', '').lower()
        if flavor == 'win_portable':
            if 'portable' in name and name.endswith('.exe'):
                selected = a
                break
        elif flavor in ('win_installer', 'source'):
            if ('setup' in name or 'readmdsetup' in name) and name.endswith('.exe'):
                selected = a
                break
            elif not selected and name.endswith('.exe') and 'portable' not in name:
                selected = a
        elif flavor == 'macos':
            if name.endswith('.zip') or name.endswith('.dmg'):
                if is_arm and 'arm64' in name:
                    selected = a
                    break
                elif not is_arm and ('x64' in name or 'x86_64' in name or 'intel' in name):
                    selected = a
                    break
                elif not selected and 'macos' in name:
                    selected = a

    # 如果没匹配到精准架构，选取最接近的 exe 或 zip
    if not selected and assets:
        for a in assets:
            name = a.get('name', '').lower()
            if sys.platform == 'win32' and name.endswith('.exe'):
                selected = a
                break
            elif sys.platform == 'darwin' and name.endswith('.zip'):
                selected = a
                break

    return selected, sha_asset


def check_update(current_version, timeout=8):
    """请求 GitHub API 获取最新 Release 信息，并返回更新详情。"""
    try:
        req = urllib.request.Request(GITHUB_API_LATEST)
        req.add_header('User-Agent', f'ReadMD/{current_version}')
        req.add_header('Accept', 'application/vnd.github.v3+json')
        
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            if resp.status != 200:
                return {'ok': False, 'error': f'HTTP {resp.status}'}
            data = json.loads(resp.read().decode('utf-8'))

        latest_tag = data.get('tag_name', '')
        has_update = is_newer_version(latest_tag, current_version)
        flavor = detect_app_flavor()
        assets = data.get('assets', [])
        best_asset, sha_asset = match_release_asset(assets, flavor)

        return {
            'ok': True,
            'has_update': has_update,
            'current_version': current_version,
            'latest_version': latest_tag,
            'release_name': data.get('name') or latest_tag,
            'published_at': data.get('published_at', ''),
            'release_notes': data.get('body', ''),
            'html_url': data.get('html_url', ''),
            'flavor': flavor,
            'asset': {
                'name': best_asset.get('name'),
                'size': best_asset.get('size', 0),
                'download_url': best_asset.get('browser_download_url'),
            } if best_asset else None,
            'sha_url': sha_asset.get('browser_download_url') if sha_asset else None,
        }
    except Exception as e:
        logging.warning('Check update failed: %s', e)
        return {'ok': False, 'error': str(e)}


def fetch_sha256_map(sha_url, timeout=10):
    """从 SHA256SUMS.txt 获取文件名到散列值的映射。"""
    if not sha_url:
        return {}
    try:
        req = urllib.request.Request(sha_url)
        req.add_header('User-Agent', 'ReadMD-Updater')
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            text = resp.read().decode('utf-8', errors='replace')
        res = {}
        for line in text.splitlines():
            parts = line.strip().split()
            if len(parts) >= 2:
                h = parts[0].strip().lower()
                fn = parts[1].lstrip('*').strip()
                res[fn] = h
        return res
    except Exception as e:
        logging.warning('Fetch SHA256 map failed: %s', e)
        return {}


def compute_file_sha256(file_path):
    """计算本地文件的 SHA256。"""
    h = hashlib.sha256()
    with open(file_path, 'rb') as f:
        while True:
            chunk = f.read(65536)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest().lower()


def get_download_status():
    """获取当前下载状态。"""
    with _download_lock:
        return dict(_download_state)


def cancel_download():
    """取消当前正在进行的下载。"""
    with _download_lock:
        if _download_state['running']:
            _download_state['cancel_requested'] = True
            _download_state['status'] = 'cancelled'
            _download_state['running'] = False
            return True
    return False


def download_asset_thread(download_url, target_filename, expected_sha=None, use_mirror=False):
    """后台下载执行线程。"""
    global _download_state
    url = download_url
    if use_mirror:
        # 使用加速前缀
        url = MIRROR_PREFIXES[0] + download_url

    temp_dir = tempfile.gettempdir()
    save_path = os.path.join(temp_dir, target_filename)

    with _download_lock:
        _download_state.update({
            'running': True,
            'total_bytes': 0,
            'downloaded_bytes': 0,
            'speed_bps': 0,
            'percent': 0,
            'status': 'downloading',
            'error': '',
            'target_file': save_path,
            'cancel_requested': False,
        })

    try:
        req = urllib.request.Request(url)
        req.add_header('User-Agent', 'ReadMD-Updater')
        
        with urllib.request.urlopen(req, timeout=30) as resp:
            total = int(resp.headers.get('Content-Length') or 0)
            with _download_lock:
                _download_state['total_bytes'] = total

            downloaded = 0
            start_time = time.time()
            last_time = start_time
            last_downloaded = 0

            with open(save_path, 'wb') as f:
                while True:
                    with _download_lock:
                        if _download_state['cancel_requested']:
                            break

                    chunk = resp.read(65536)
                    if not chunk:
                        break
                    f.write(chunk)
                    downloaded += len(chunk)

                    now = time.time()
                    dt = now - last_time
                    if dt >= 0.3:
                        speed = (downloaded - last_downloaded) / dt if dt > 0 else 0
                        pct = int((downloaded / total * 100)) if total > 0 else 0
                        with _download_lock:
                            _download_state['downloaded_bytes'] = downloaded
                            _download_state['speed_bps'] = int(speed)
                            _download_state['percent'] = pct
                        last_time = now
                        last_downloaded = downloaded

            if _download_state['cancel_requested']:
                try:
                    os.unlink(save_path)
                except Exception:
                    pass
                return

        # 下载完成，校验 SHA256
        with _download_lock:
            _download_state['status'] = 'verifying'
            _download_state['percent'] = 100

        if expected_sha:
            actual_sha = compute_file_sha256(save_path)
            if actual_sha != expected_sha.lower():
                raise ValueError(f'SHA256 校验失败：期望 {expected_sha}，实际 {actual_sha}')

        with _download_lock:
            _download_state['status'] = 'ready'
            _download_state['running'] = False

    except Exception as e:
        logging.exception('Download update failed: %s', e)
        with _download_lock:
            _download_state['status'] = 'error'
            _download_state['error'] = str(e)
            _download_state['running'] = False


def start_download_update(download_url, target_filename, expected_sha=None, use_mirror=False):
    """启动下载线程。"""
    with _download_lock:
        if _download_state['running']:
            return False, '已有下载任务正在进行'
    t = threading.Thread(
        target=download_asset_thread,
        args=(download_url, target_filename, expected_sha, use_mirror),
        daemon=True,
    )
    t.start()
    return True, '下载已启动'


def apply_update(file_path=None, flavor=None):
    """执行本地更新并准备重启。"""
    with _download_lock:
        path = file_path or _download_state.get('target_file')
    if not path or not os.path.isfile(path):
        return False, '更新文件不存在或尚未下载完成'

    if flavor is None:
        flavor = detect_app_flavor()

    try:
        if sys.platform == 'win32':
            if flavor == 'win_installer':
                # 运行安装包进行覆盖安装
                cmd = f'"{path}" /UPDATE'
                subprocess.Popen(cmd, shell=True)
                return True, '正在启动安装器…'
            elif flavor == 'win_portable':
                # 便携版热替换脚本
                current_exe = sys.executable
                bat_content = f"""@echo off
timeout /t 1 /nobreak >nul
:retry
move /y "{path}" "{current_exe}" >nul 2>nul
if errorlevel 1 (
    timeout /t 1 /nobreak >nul
    goto retry
)
start "" "{current_exe}"
del "%~f0"
"""
                bat_path = os.path.join(tempfile.gettempdir(), 'readmd_update.bat')
                with open(bat_path, 'w', encoding='ansi', errors='ignore') as f:
                    f.write(bat_content)
                subprocess.Popen(f'cmd.exe /c "{bat_path}"', shell=True)
                return True, '正在替换便携版程序…'
            else:
                # 源码/开发模式下直接启动新安装器
                subprocess.Popen(f'"{path}"', shell=True)
                return True, '已启动更新程序'
        elif sys.platform == 'darwin':
            # macOS 解压并打开
            subprocess.Popen(['open', path])
            return True, '已打开更新包'
        else:
            return False, '当前平台暂不支持自动替换，请手动解压运行'
    except Exception as e:
        logging.exception('Apply update failed: %s', e)
        return False, str(e)
