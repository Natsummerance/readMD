# -*- mode: python ; coding: utf-8 -*-
# ReadMD Windows spec（onedir / onefile 共用，冻结态插件安装依赖本文件）
#
# 打包后 sys.executable 指向 ReadMD.exe，插件中心不能再用
# `[sys.executable, '-m', 'pip', ...]` 起子进程——那会把 pip 参数喂回主程序自己的
# argparse。plugin_manager 因此在冻结态改为进程内 runpy 跑 pip，而这条路成立的
# 前提是 pip 真的在包里：既要 pip 的全部子模块，也要 pip/_vendor/certifi/cacert.pem，
# 缺后者时每一次 HTTPS 装包请求都会以证书错误告终。
#
# CI 用一个文件覆盖四种 Windows 产物：
#   READMD_ARTIFACT_NAME   产物名，默认 ReadMD（ReadMD-arm64 / ReadMD-portable 等）
#   READMD_ONEFILE=1       产出单文件 exe，跳过 COLLECT
import os
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

ROOT_DIR = os.path.abspath(os.path.join(SPECPATH, '..'))
ARTIFACT_NAME = os.environ.get('READMD_ARTIFACT_NAME', 'ReadMD')
ONEFILE = os.environ.get('READMD_ONEFILE') == '1'

datas = [
    (os.path.join(ROOT_DIR, 'assets'), 'assets'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_core'), 'src/readmd_core'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_modules'), 'src/readmd_modules'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_fix.py'), 'src'),
    (os.path.join(ROOT_DIR, 'VERSION'), '.'),
]
hiddenimports = ['src.readmd_fix', 'src.readmd_core', 'runpy']
for _bundled in ('magika', 'docx', 'reportlab', 'matplotlib', 'trafilatura', 'pip'):
    datas += collect_data_files(_bundled)
    hiddenimports += collect_submodules(_bundled)

a = Analysis(
    [os.path.join(ROOT_DIR, 'readmd.py')],
    pathex=[ROOT_DIR],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)

# 过滤并修正 a.binaries 中的 MSVC 运行时库：
# 避免由第三方 wheel（如旧版 winrt 等）引入陈旧的 MSVCP140.dll（如 VS2017 14.16），
# 该陈旧版本与现代编译的 C 扩展（如 onnxruntime）存在底层 ABI 冲突并触发 0xC0000005 闪退。
# 统一强制使用 System32 下宿主系统最新的现代 MSVC 运行时。
sys32_dir = os.path.join(os.environ.get('WINDIR', 'C:\\Windows'), 'System32')
vc_runtimes = {
    'msvcp140.dll', 'msvcp140_1.dll', 'msvcp140_2.dll', 'msvcp140_atomic_wait.dll',
    'vcruntime140.dll', 'vcruntime140_1.dll', 'vcomp140.dll'
}
fixed_binaries = []
for dest, src, typ in a.binaries:
    base = os.path.basename(dest).lower()
    if base in vc_runtimes:
        sys32_path = os.path.join(sys32_dir, os.path.basename(dest))
        if os.path.exists(sys32_path):
            fixed_binaries.append((dest, sys32_path, typ))
            continue
    fixed_binaries.append((dest, src, typ))
a.binaries = fixed_binaries

pyz = PYZ(a.pure)

if ONEFILE:
    exe = EXE(
        pyz,
        a.scripts,
        a.binaries,
        a.datas,
        [],
        name=ARTIFACT_NAME,
        debug=False,
        bootloader_ignore_signals=False,
        strip=False,
        upx=True,
        console=False,
        disable_windowed_traceback=False,
        argv_emulation=False,
        target_arch=None,
        codesign_identity=None,
        entitlements_file=None,
        icon=[os.path.join(ROOT_DIR, 'assets', 'readmd.ico')],
    )
else:
    exe = EXE(
        pyz,
        a.scripts,
        [],
        exclude_binaries=True,
        name=ARTIFACT_NAME,
        debug=False,
        bootloader_ignore_signals=False,
        strip=False,
        upx=True,
        console=False,
        disable_windowed_traceback=False,
        argv_emulation=False,
        target_arch=None,
        codesign_identity=None,
        entitlements_file=None,
        icon=[os.path.join(ROOT_DIR, 'assets', 'readmd.ico')],
    )
    coll = COLLECT(
        exe,
        a.binaries,
        a.datas,
        strip=False,
        upx=True,
        upx_exclude=[],
        name=ARTIFACT_NAME,
    )
