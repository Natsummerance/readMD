# -*- coding: utf-8 -*-
"""ReadMD Win7 兼容版：下载并解包固定版 WebView2 109 运行时。

来源：GitHub westinyang/WebView2RuntimeArchive 109.0.1518.78（109 分支 = Win7 最后支持线；
Microsoft Update Catalog 上的 109.0.1518.140 无法直接下载，故取同分支 109.0.1518.78）。
产物：installer/webview2_runtime/ 直接包含 msedgewebview2.exe（已展开 wrapper 目录）。

用法：
  python tools\bundle_runtime.py            # 已有运行时则跳过
  python tools\bundle_runtime.py --force   # 强制重新下载
"""
import os
import shutil
import subprocess
import sys
import urllib.request


RUNTIME_VER = '109.0.1518.78'
CAB_URL = ('https://github.com/westinyang/WebView2RuntimeArchive/releases/download/'
          '109.0.1518.78/Microsoft.WebView2.FixedVersionRuntime.109.0.1518.78.x64.cab')
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEST = os.path.join(ROOT, 'installer', 'webview2_runtime')
DL = os.path.join(ROOT, 'installer', 'webview2_runtime_dl')
CAB = os.path.join(DL, 'wv2.cab')
WRAP = os.path.join(DEST, 'Microsoft.WebView2.FixedVersionRuntime.%s.x64' % RUNTIME_VER)


def already_have():
    return os.path.isfile(os.path.join(DEST, 'msedgewebview2.exe'))


def download(url, path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    print('downloading %s ...' % url, flush=True)
    req = urllib.request.Request(url, headers={'User-Agent': 'readmd-win7-bundle'})
    with urllib.request.urlopen(req, timeout=1800) as r, open(path, 'wb') as f:
        shutil.copyfileobj(r, f)
    print('downloaded %d bytes' % os.path.getsize(path), flush=True)


def extract_cab(cab, dest):
    os.makedirs(dest, exist_ok=True)
    rc = subprocess.call(['expand.exe', cab, '-F:*', dest])
    if rc != 0:
        raise SystemExit('expand.exe 解包失败（%s）' % cab)


def flatten(wrap, dest):
    if os.path.isdir(wrap):
        for name in os.listdir(wrap):
            src = os.path.join(wrap, name)
            tgt = os.path.join(dest, name)
            if os.path.isdir(src):
                shutil.rmtree(tgt, ignore_errors=True)
                shutil.copytree(src, tgt)
            else:
                shutil.move(src, tgt)
        shutil.rmtree(wrap, ignore_errors=True)


def main():
    force = '--force' in sys.argv
    if already_have() and not force:
        print('runtime already present at %s (skip)' % DEST)
        return 0
    if not already_have():
        download(CAB_URL, CAB)
        extract_cab(CAB, DEST)
        flatten(WRAP, DEST)
        shutil.rmtree(DL, ignore_errors=True)
    if not already_have():
        raise SystemExit('运行时解包后仍未找到 msedgewebview2.exe')
    total = sum(os.path.getsize(os.path.join(r, f)) for r, _d, fs in os.walk(DEST) for f in fs)
    print('runtime OK: %s (%.1f MB)' % (DEST, total / 1048576))
    return 0


if __name__ == '__main__':
    sys.exit(main())
