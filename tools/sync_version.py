# -*- coding: utf-8 -*-
"""ReadMD 全局统一版本配置与全生态跨平台一键同步工具 (Global Unified Version Sync Engine)

Single Source of Truth: .env (READMD_VERSION) 或根目录 VERSION 文件

支持平台与配置矩阵：
1. 统一配置文件: .env / .env.example / VERSION
2. Python 主服务与核心运行时: readmd.py / src/readmd_core/config.py / packages/mcp-server/readmd_mcp_server.py
3. Windows 打包与安装器: installer/setup_app.py / scripts/windows/package.bat / scripts/windows/build_win7.bat
4. macOS App Bundle: release/ReadMD-macOS.spec
5. Linux & 信创全架构: scripts/linux/build_linux.sh / build_rpm.sh / PKGBUILD / linglong.yaml / packages/linglong/linglong.yaml
6. VSCode 扩展: packages/vscode-extension/package.json
7. 鸿蒙 HarmonyOS NEXT: packages/harmonyos-app/package.json 和 AppScope/app.json5
8. GitHub Actions CI/CD 流水线: .github/workflows/release.yml
9. 前端界面与 UI 显示: assets/index.html / assets/js/features/updater.js / assets/readmd.boot.js
10. 发版说明与多语言 README: release/release_notes.md / README.md / README.en.md / README.ja.md / README.zh-TW.md
11. 官网与分发矩阵: website/public/**/*.html / sitemap.xml / site.webmanifest / feed.xml / _headers
"""

import argparse
import base64
import datetime
import hashlib
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load_env_version():
    """从根目录 .env 文件或环境变量加载版本号。"""
    env_file = os.path.join(ROOT, '.env')
    if os.path.isfile(env_file):
        with open(env_file, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line.startswith('READMD_VERSION=') or line.startswith('VERSION='):
                    val = line.split('=', 1)[1].strip().strip('\'"')
                    if val:
                        return val
    ver_file = os.path.join(ROOT, 'VERSION')
    if os.path.isfile(ver_file):
        with open(ver_file, 'r', encoding='utf-8') as f:
            val = f.read().strip()
            if val:
                return val
    return os.environ.get('READMD_VERSION', '')


def parse_semver(ver: str):
    """提取主次修订号与后缀。"""
    clean_ver = ver.strip().lstrip('vV')
    m = re.match(r'^(\d+)\.(\d+)\.(\d+)(?:[-.]([a-zA-Z0-9.]+))?$', clean_ver)
    if m:
        major, minor, patch = int(m.group(1)), int(m.group(2)), int(m.group(3))
        extra = m.group(4) or ''
        return major, minor, patch, extra
    raise ValueError('invalid semantic version: %s' % ver)


def generate_env_block(target_ver: str, existing_env: str = '') -> str:
    """根据目标版本生成统一的多平台版本矩阵环境变量块，同时保留现有密钥等自定义配置。"""
    major, minor, patch, extra = parse_semver(target_ver)
    triplet = f"{major}.{minor}.{patch}"

    digits = re.findall(r'\d+', extra) if extra else []
    linglong_extra = digits[0] if digits else ('1' if extra else '0')
    linglong_ver = f"{major}.{minor}.{patch}.{linglong_extra}"
    harmony_code = major * 10000 + minor * 100 + patch
    today = datetime.date.today().strftime('%Y-%m-%d')

    version_keys = [
        'READMD_VERSION',
        'READMD_VERSION_TAG',
        'READMD_VERSION_SEMVER',
        'READMD_VERSION_TRIPLET',
        'READMD_VERSION_MAJOR',
        'READMD_VERSION_MINOR',
        'READMD_VERSION_PATCH',
        'READMD_VERSION_PRERELEASE',
        'READMD_VERSION_LINGLONG',
        'READMD_VERSION_HARMONY_CODE',
        'READMD_VERSION_HARMONY_NAME',
        'READMD_VERSION_WINDOWS_FILEVER',
        'READMD_VERSION_VSCODE',
        'READMD_VERSION_RPM_REL',
        'READMD_VERSION_ARCH_REL',
        'READMD_VERSION_BUILD_DATE',
    ]

    env_lines = [
        '# ReadMD 全局统一版本配置 (Single Source of Truth for Global Versioning)',
        '# 修改此文件中的 READMD_VERSION 或运行 python tools/sync_version.py <version> 即可一键同步全库所有文件',
        '',
        f'READMD_VERSION={target_ver}',
        f'READMD_VERSION_TAG=v{target_ver}',
        f'READMD_VERSION_SEMVER={target_ver}',
        f'READMD_VERSION_TRIPLET={triplet}',
        f'READMD_VERSION_MAJOR={major}',
        f'READMD_VERSION_MINOR={minor}',
        f'READMD_VERSION_PATCH={patch}',
        f'READMD_VERSION_PRERELEASE={extra}',
        f'READMD_VERSION_LINGLONG={linglong_ver}',
        f'READMD_VERSION_HARMONY_CODE={harmony_code}',
        f'READMD_VERSION_HARMONY_NAME={target_ver}',
        f'READMD_VERSION_WINDOWS_FILEVER={linglong_ver}',
        f'READMD_VERSION_VSCODE={target_ver}',
        'READMD_VERSION_RPM_REL=1',
        'READMD_VERSION_ARCH_REL=1',
        f'READMD_VERSION_BUILD_DATE={today}',
        ''
    ]

    # 提取非版本控制的自定义环境变量（如 API KEY、URL 等）
    custom_lines = []
    if existing_env:
        for line in existing_env.splitlines():
            line_str = line.strip()
            if not line_str or line_str.startswith('#'):
                continue
            k = line_str.split('=', 1)[0].strip()
            if k not in version_keys and k != 'VERSION':
                custom_lines.append(line.strip())

    if custom_lines:
        env_lines.append('# 自定义本地凭据与运行时环境')
        env_lines.extend(custom_lines)
        env_lines.append('')

    return '\n'.join(env_lines)


def bundle_readmd_boot():
    """重新编译打包 assets/readmd.boot.js。"""
    sources = [
        'vendor/marked.min.js', 'js/core/state.js', 'js/core/i18n.js',
        'js/core/dialog.js', 'js/core/settings.js', 'js/core/modules.js',
        'js/core/tabs.js', 'js/core/history.js', 'js/core/dragdrop.js',
        'js/reader/formula.js', 'js/reader/fixes.js', 'js/reader/toc.js',
        'js/reader/search.js', 'js/reader/folder.js', 'js/reader/render.js',
        'js/editor/preview.js', 'js/editor/image.js', 'js/editor/editor.js',
        'js/features/ai.js', 'js/features/share.js', 'js/features/convert.js',
        'js/features/ocr.js', 'js/features/web.js', 'js/features/clipboard.js',
        'js/features/export.js', 'js/features/updater.js', 'app.js'
    ]
    out_path = os.path.join(ROOT, 'assets', 'readmd.boot.js')
    chunks = []
    for s in sources:
        sp = os.path.join(ROOT, 'assets', s)
        if os.path.isfile(sp):
            with open(sp, 'rb') as f:
                chunks.append(f.read())
    with open(out_path, 'wb') as out:
        out.write(b'\n;\n'.join(chunks) + b'\n')


def sync_all(target_ver: str, check_only: bool = False) -> bool:
    target_ver = target_ver.strip().lstrip('vV')
    major, minor, patch, extra = parse_semver(target_ver)
    triplet = f"{major}.{minor}.{patch}"

    digits = re.findall(r'\d+', extra) if extra else []
    linglong_extra = digits[0] if digits else ('1' if extra else '0')
    linglong_ver = f"{major}.{minor}.{patch}.{linglong_extra}"
    harmony_code = major * 10000 + minor * 100 + patch

    diffs = []

    # 1. .env & .env.example
    env_path = os.path.join(ROOT, '.env')
    old_env = ''
    if os.path.isfile(env_path):
        with open(env_path, 'r', encoding='utf-8') as f:
            old_env = f.read()
    new_env = generate_env_block(target_ver, old_env)
    if old_env.strip() != new_env.strip():
        diffs.append((env_path, old_env, new_env))

    example_path = os.path.join(ROOT, '.env.example')
    old_example = ''
    if os.path.isfile(example_path):
        with open(example_path, 'r', encoding='utf-8') as f:
            old_example = f.read()
    new_example = generate_env_block(target_ver, '')
    if old_example.strip() != new_example.strip():
        diffs.append((example_path, old_example, new_example))

    # VERSION
    vpath = os.path.join(ROOT, 'VERSION')
    vcontent = f'{target_ver}\n'
    if os.path.isfile(vpath):
        with open(vpath, 'r', encoding='utf-8') as f:
            old = f.read()
        if old.strip() != target_ver:
            diffs.append((vpath, old, vcontent))
    else:
        diffs.append((vpath, '', vcontent))

    # 2. Python 主服务与核心运行时（均直接从 .env / config.py 动态解析，无需硬编码 fallback）
    mcp_server_py = os.path.join(ROOT, 'packages', 'mcp-server', 'readmd_mcp_server.py')
    if os.path.isfile(mcp_server_py):
        with open(mcp_server_py, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'("name":\s*"readmd-mcp-server",\s*"version":\s*")[^"]+"', f'\\g<1>{target_ver}"', src)
        if new_src != src:
            diffs.append((mcp_server_py, src, new_src))

    # 3. Linux & 信创脚本
    for frel in ['scripts/linux/linglong.yaml', 'packages/linglong/linglong.yaml']:
        ll_path = os.path.join(ROOT, frel)
        if os.path.isfile(ll_path):
            with open(ll_path, 'r', encoding='utf-8') as f:
                src = f.read()
            new_src = re.sub(r'(version:\s*)[0-9.]+', f'\\g<1>{linglong_ver}', src)
            if new_src != src:
                diffs.append((ll_path, src, new_src))

    pkgbuild_path = os.path.join(ROOT, 'scripts', 'linux', 'PKGBUILD')
    if os.path.isfile(pkgbuild_path):
        with open(pkgbuild_path, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'(pkgver=)[^\n]+', f'\\g<1>{triplet}', src)
        if new_src != src:
            diffs.append((pkgbuild_path, src, new_src))

    # 6. VSCode 扩展 & 鸿蒙应用
    vscode_pkg = os.path.join(ROOT, 'packages', 'vscode-extension', 'package.json')
    if os.path.isfile(vscode_pkg):
        with open(vscode_pkg, 'r', encoding='utf-8') as f:
            data = json.load(f)
        if data.get('version') != target_ver:
            data['version'] = target_ver
            new_content = json.dumps(data, ensure_ascii=False, indent=2) + '\n'
            diffs.append((vscode_pkg, '', new_content))

    harmony_pkg = os.path.join(ROOT, 'packages', 'harmonyos-app', 'package.json')
    if os.path.isfile(harmony_pkg):
        with open(harmony_pkg, 'r', encoding='utf-8') as f:
            data = json.load(f)
        if data.get('version') != target_ver:
            data['version'] = target_ver
            new_content = json.dumps(data, ensure_ascii=False, indent=2) + '\n'
            diffs.append((harmony_pkg, '', new_content))
    # DevEco package manifests are JSON5, so keep their formatting and update
    # only the declared package version.
    for harmony_manifest in ('packages/harmonyos-app/oh-package.json5',
                             'packages/harmonyos-app/entry/oh-package.json5'):
        manifest_path = os.path.join(ROOT, harmony_manifest)
        if os.path.isfile(manifest_path):
            with open(manifest_path, 'r', encoding='utf-8') as f:
                src = f.read()
            new_src = re.sub(r'("version"\s*:\s*")[^"]+("),', rf'\g<1>{target_ver}\g<2>,', src, count=1)
            if new_src != src:
                diffs.append((manifest_path, src, new_src))

    ui_tests_pkg = os.path.join(ROOT, 'ui-tests', 'package.json')
    if os.path.isfile(ui_tests_pkg):
        with open(ui_tests_pkg, 'r', encoding='utf-8') as f:
            data = json.load(f)
        if data.get('version') != target_ver:
            data['version'] = target_ver
            diffs.append((ui_tests_pkg, '', json.dumps(data, ensure_ascii=False, indent=2) + '\n'))

    app_json5 = os.path.join(ROOT, 'packages', 'harmonyos-app', 'AppScope', 'app.json5')
    if os.path.isfile(app_json5):
        with open(app_json5, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'"versionCode":\s*\d+', f'"versionCode": {harmony_code}', src)
        new_src = re.sub(r'"versionName":\s*"[^"]+"', f'"versionName": "{target_ver}"', new_src)
        if new_src != src:
            diffs.append((app_json5, src, new_src))

    # 7. CI/CD 流水线
    release_yml = os.path.join(ROOT, '.github', 'workflows', 'release.yml')
    if os.path.isfile(release_yml):
        with open(release_yml, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r"READMD_VERSION:\s*'[^']+'", f"READMD_VERSION: '{target_ver}'", src)
        if new_src != src:
            diffs.append((release_yml, src, new_src))

    # 8. 前端界面与 UI 显示
    index_path = os.path.join(ROOT, 'assets', 'index.html')
    if os.path.isfile(index_path):
        with open(index_path, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'<html([^>]*\bdata-version=")[^"]+"', rf'<html\g<1>{target_ver}"', src)
        new_src = re.sub(r'(<link[^>]*href="/assets/style\.css\?v=)[^"]+(")', rf'\g<1>{target_ver}\g<2>', new_src)
        new_src = re.sub(r'(<span id="status-version"[^>]*>)v[^<]+(</span>)', rf'\g<1>v{target_ver}\g<2>', new_src)
        new_src = re.sub(r'(id="menu-version-label">)当前版本 v[^<]+(</em>)', rf'\g<1>当前版本 v{target_ver}\g<2>', new_src)
        new_src = re.sub(r'(<script[^>]*src="/assets/readmd\.boot\.js\?v=)[^"]+(")', rf'\g<1>{target_ver}\g<2>', new_src)
        if new_src != src:
            diffs.append((index_path, src, new_src))

    updater_js = os.path.join(ROOT, 'assets', 'js', 'features', 'updater.js')
    if os.path.isfile(updater_js):
        with open(updater_js, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r"(typeof VERSION !== 'undefined' \? VERSION : ')[^']+'\)", f"\\g<1>{target_ver}')", src)
        if new_src != src:
            diffs.append((updater_js, src, new_src))

    # 9. 发版说明与多语言 README
    for rname in ('README.md', 'README.en.md', 'README.ja.md', 'README.zh-TW.md'):
        rpath = os.path.join(ROOT, rname)
        if not os.path.isfile(rpath):
            continue
        with open(rpath, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'badge/version-v[0-9a-zA-Z.-]+-3b6ef5', f'badge/version-v{target_ver}-3b6ef5', src)
        new_src = re.sub(r'(releases/download/v)[0-9a-zA-Z.-]+', f'\\g<1>{target_ver}', new_src)
        new_src = re.sub(r'(releases/tag/v)[0-9a-zA-Z.-]+', f'\\g<1>{target_ver}', new_src)
        new_src = re.sub(r'(?P<prefix>-)v(?P<version>\d[0-9a-zA-Z.-]*)(?P<suffix>\.(?:exe|zip|AppImage|deb|vsix)\b)', f'\\g<prefix>v{target_ver}\\g<suffix>', new_src)
        new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_amd64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_arm64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(vscode-)[0-9a-zA-Z.-]+(\.vsix)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(server-)[0-9a-zA-Z.-]+(\.zip)', f'\\g<1>{target_ver}\\g<2>', new_src)
        if new_src != src:
            diffs.append((rpath, src, new_src))

    rel_notes = os.path.join(ROOT, 'release', 'release_notes.md')
    if os.path.isfile(rel_notes):
        with open(rel_notes, 'r', encoding='utf-8') as f:
            src = f.read()
        new_src = re.sub(r'(# ReadMD v)[0-9a-zA-Z.-]+', f'\\g<1>{target_ver}', src)
        new_src = re.sub(r'(?P<prefix>-)v(?P<version>\d[0-9a-zA-Z.-]*)(?P<suffix>\.(?:exe|zip|AppImage|deb|vsix)\b)', f'\\g<prefix>v{target_ver}\\g<suffix>', new_src)
        new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_amd64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_arm64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(vscode-)[0-9a-zA-Z.-]+(\.vsix)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(server-)[0-9a-zA-Z.-]+(\.zip)', f'\\g<1>{target_ver}\\g<2>', new_src)
        if new_src != src:
            diffs.append((rel_notes, src, new_src))

    # 10. 官网与分发页面 (Website)
    website_public = os.path.join(ROOT, 'website', 'public')
    if os.path.isdir(website_public):
        for root_dir, _, files in os.walk(website_public):
            for file in sorted(files):
                if file.endswith(('.html', '.xml', '.txt', '.json')):
                    fpath = os.path.join(root_dir, file)
                    with open(fpath, 'r', encoding='utf-8') as f:
                        src = f.read()
                    new_src = re.sub(r'(?P<prefix>-)v(?P<version>\d[0-9a-zA-Z.-]*)(?P<suffix>\.(?:exe|zip|AppImage|deb|vsix)\b)', f'\\g<prefix>v{target_ver}\\g<suffix>', src)
                    new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_amd64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
                    new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_arm64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
                    new_src = re.sub(r'(vscode-)[0-9a-zA-Z.-]+(\.vsix)', f'\\g<1>{target_ver}\\g<2>', new_src)
                    new_src = re.sub(r'(server-)[0-9a-zA-Z.-]+(\.zip)', f'\\g<1>{target_ver}\\g<2>', new_src)
                    new_src = re.sub(r'(releases/tag/v)[0-9a-zA-Z.-]+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'(releases/download/v)[0-9a-zA-Z.-]+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'("softwareVersion":\s*")[^"]+"', f'\\g<1>{target_ver}"', new_src)
                    new_src = re.sub(r'("artifactSection":\s*"v)[^"]+"', f'\\g<1>{target_ver}"', new_src)
                    new_src = re.sub(r'(ReadMD\s+v)2\.3\.7-beta\.\d+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'(version\s+)2\.3\.7-beta\.\d+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'(版本[：:\s]+)2\.3\.7-beta\.\d+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'(バージョン[：:\s]+)2\.3\.7-beta\.\d+', f'\\g<1>{target_ver}', new_src)
                    new_src = re.sub(r'2\.3\.7-beta\.\d+', target_ver, new_src)
                    if new_src != src:
                        diffs.append((fpath, src, new_src))

        # Update _headers CSP sha256 hash for JSON-LD in index.html
        site_index = os.path.join(website_public, 'index.html')
        headers_file = os.path.join(website_public, '_headers')
        if os.path.isfile(site_index) and os.path.isfile(headers_file):
            with open(site_index, 'r', encoding='utf-8') as f:
                idx_content = f.read()
            for p, _, nc in diffs:
                if p == site_index:
                    idx_content = nc
                    break
            m = re.search(r'(?s)<script type="application/ld\+json">(.*?)</script>', idx_content)
            if m:
                digest = hashlib.sha256(m.group(1).encode('utf-8')).digest()
                csp_hash = 'sha256-' + base64.b64encode(digest).decode('ascii')
                with open(headers_file, 'r', encoding='utf-8') as f:
                    h_content = f.read()
                new_h_content = re.sub(r"'sha256-[^']+'", f"'{csp_hash}'", h_content)
                if new_h_content != h_content:
                    diffs.append((headers_file, h_content, new_h_content))

    if check_only:
        if diffs:
            print(f'[FAIL] Found {len(diffs)} files out of sync with target version {target_ver}:')
            for fpath, _, _ in diffs:
                print(f' - {os.path.relpath(fpath, ROOT)}')
            return False
        else:
            print(f'[OK] All platform files are 100% synchronized with version {target_ver}!')
            return True

    # 写入更新
    for fpath, _, content in diffs:
        with open(fpath, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f'[SYNC] Updated: {os.path.relpath(fpath, ROOT)}')

    # 自动重构前端 boot.js bundle
    bundle_readmd_boot()
    print('[SYNC] Rebundled assets/readmd.boot.js')

    print(f'\n[SUCCESS] Successfully synchronized all platform version files to {target_ver}!')
    return True


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='ReadMD Unified Version Synchronization Tool')
    parser.add_argument('version', nargs='?', default=None, help='Target version (e.g. 2.3.7 or 2.4.0)')
    parser.add_argument('--check', action='store_true', help='Check if all files are in sync without writing')
    args = parser.parse_args()

    ver = args.version or load_env_version()
    try:
        success = sync_all(ver, check_only=args.check)
    except ValueError as exc:
        print('[ERROR] %s' % exc, file=sys.stderr)
        sys.exit(2)
    sys.exit(0 if success else 1)
