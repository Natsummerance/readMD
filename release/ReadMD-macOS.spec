# -*- mode: python ; coding: utf-8 -*-
# ReadMD macOS .app bundle spec
import os
import sys
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

ROOT_DIR = os.path.abspath(os.path.join(SPECPATH, '..'))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)
try:
    import readmd
    VERSION = getattr(readmd, 'VERSION', '2.3.7-beta.5')
except Exception:
    VERSION = os.environ.get('READMD_VERSION', '2.3.7-beta.5')

modules_dir = os.path.join(ROOT_DIR, 'src', 'readmd_modules')
module_datas = []
if os.path.isdir(modules_dir):
    for f in os.listdir(modules_dir):
        fp = os.path.join(modules_dir, f)
        if f.endswith('.py') and not f.startswith('windows_native') and f != '__pycache__':
            module_datas.append((fp, 'src/readmd_modules'))
        elif os.path.isdir(fp) and f != '__pycache__':
            module_datas.append((fp, f'src/readmd_modules/{f}'))

datas = [
    (os.path.join(ROOT_DIR, 'assets'), 'assets'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_core'), 'src/readmd_core'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_fix.py'), 'src'),
] + module_datas
hiddenimports = ['src.readmd_fix', 'src.readmd_core', 'Vision', 'Quartz', 'Foundation', 'objc']
datas += collect_data_files('magika')
datas += collect_data_files('docx')
datas += collect_data_files('reportlab')
datas += collect_data_files('matplotlib')
datas += collect_data_files('trafilatura')
hiddenimports += collect_submodules('src.readmd_core')
hiddenimports += [m for m in collect_submodules('src.readmd_modules') if not m.endswith('windows_native')]

a = Analysis(
    [os.path.join(ROOT_DIR, 'readmd.py')],
    pathex=[ROOT_DIR],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'winrt', 'pywinrt', 'winreg', 'win32api', 'win32con', 'win32gui', 'win32process',
        'src.readmd_modules.windows_native',
        'installer',
    ],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='ReadMD',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=True,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
    icon=[os.path.join(ROOT_DIR, 'assets', 'ReadMD.icns')],
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name='ReadMD',
)

# macOS .app bundle
app = BUNDLE(
    coll,
    name='ReadMD.app',
    icon=os.path.join(ROOT_DIR, 'assets', 'ReadMD.icns'),
    bundle_identifier='io.github.natsummerance.readmd',
    version=VERSION,
    info_plist={
        'CFBundleIdentifier': 'io.github.natsummerance.readmd',
        'CFBundleName': 'ReadMD',
        'CFBundleDisplayName': 'ReadMD',
        'CFBundleVersion': VERSION,
        'CFBundleShortVersionString': VERSION,

        'CFBundlePackageType': 'APPL',
        'NSPrincipalClass': 'NSApplication',
        'NSHighResolutionCapable': True,
        'CFBundleDocumentTypes': [
            {
                'CFBundleTypeName': 'Markdown Document',
                'CFBundleTypeRole': 'Editor',
                'LSHandlerRank': 'Owner',
                'LSItemContentTypes': ['net.daringfireball.markdown', 'public.text'],
                'CFBundleTypeExtensions': ['md', 'markdown', 'mdown', 'mkd', 'mdx'],
            }
        ],
    },
)

