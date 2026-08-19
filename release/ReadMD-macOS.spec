# -*- mode: python ; coding: utf-8 -*-
# ReadMD macOS .app bundle spec
import os
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

ROOT_DIR = os.path.abspath(os.path.join(SPECPATH, '..'))

datas = [
    (os.path.join(ROOT_DIR, 'assets'), 'assets'),
    (os.path.join(ROOT_DIR, 'src', 'readmd_fix.py'), 'src'),
]
hiddenimports = ['src.readmd_fix', 'Vision', 'Quartz', 'Foundation', 'objc']
datas += collect_data_files('magika')
datas += collect_data_files('docx')
datas += collect_data_files('reportlab')
datas += collect_data_files('matplotlib')
datas += collect_data_files('trafilatura')
hiddenimports += collect_submodules('src.readmd_modules')

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
        'winrt', 'pywinrt', 'winreg',
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
    version='2.2.9',
    info_plist={
        'CFBundleName': 'ReadMD',
        'CFBundleDisplayName': 'ReadMD',
        'CFBundleVersion': '2.2.9',
        'CFBundleShortVersionString': '2.2.9',



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

