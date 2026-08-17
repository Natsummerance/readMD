# -*- mode: python ; coding: utf-8 -*-
# ReadMD macOS .app bundle spec
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

datas = [('assets', 'assets'), ('readmd_fix.py', '.')]
hiddenimports = ['readmd_fix', 'Vision', 'Quartz', 'Foundation', 'objc']
datas += collect_data_files('magika')
datas += collect_data_files('docx')
datas += collect_data_files('trafilatura')
hiddenimports += collect_submodules('readmd_modules')

a = Analysis(
    ['readmd.py'],
    pathex=[],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'winrt', 'pywinrt', 'winreg',
        'readmd_modules.windows_native',
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
    icon=['assets/ReadMD.icns'],
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
    icon='assets/ReadMD.icns',
    bundle_identifier='io.github.natsummerance.readmd',
    version='2.2.2',
    info_plist={
        'CFBundleName': 'ReadMD',
        'CFBundleDisplayName': 'ReadMD',
        'CFBundleVersion': '2.2.2',
        'CFBundleShortVersionString': '2.2.2',
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
