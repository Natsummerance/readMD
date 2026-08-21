# -*- coding: utf-8 -*-
"""ReadMD 统一版本号同步工具 (Unified Version Sync Tool)

Single Source of Truth: .env (READMD_VERSION) 或根目录 VERSION 文件

支持平台与配置矩阵：
1. Python 主服务 (readmd.py)
2. Windows 安装器 (installer/setup_app.py)
3. VSCode 扩展 (packages/vscode-extension/package.json)
4. 鸿蒙 HarmonyOS NEXT (packages/harmonyos-app/AppScope/app.json5)
5. 统信/深度 玲珑打包 (packages/linglong/linglong.yaml)
6. GitHub Actions CI/CD 流水线 (.github/workflows/release.yml)
7. 多语言 README (README.md, README.en.md, README.ja.md)
8. 发版说明 (release/release_notes.md)
"""

import argparse
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
    return os.environ.get('READMD_VERSION') or '2.3.7-beta.1'


def parse_semver(ver: str):
    """提取主次修订号与后缀。"""
    m = re.match(r'^(\d+)\.(\d+)\.(\d+)(?:[-.]([a-zA-Z0-9.]+))?$', ver.strip())
    if m:
        major, minor, patch = int(m.group(1)), int(m.group(2)), int(m.group(3))
        extra = m.group(4) or ''
        return major, minor, patch, extra
    return 2, 3, 7, ''


def sync_all(target_ver: str, check_only: bool = False) -> bool:
    target_ver = target_ver.strip().lstrip('vV')
    major, minor, patch, extra = parse_semver(target_ver)
    
    # 玲珑版本规范：4段数字 x.y.z.b
    linglong_extra = '0'
    if extra:
        digits = re.findall(r'\d+', extra)
        if digits:
            linglong_extra = digits[0]
        else:
            linglong_extra = '1'
    linglong_ver = f'{major}.{minor}.{patch}.{linglong_extra}'
    
    # 鸿蒙 versionCode: 20307
    harmony_code = major * 10000 + minor * 100 + patch
    
    diffs = []
    
    # 1. .env & .env.example & VERSION
    for fpath in [os.path.join(ROOT, '.env'), os.path.join(ROOT, '.env.example')]:
        content = f'READMD_VERSION={target_ver}\n'
        if os.path.isfile(fpath):
            with open(fpath, 'r', encoding='utf-8') as f:
                old = f.read()
            if old != content:
                diffs.append((fpath, old, content))
        else:
            diffs.append((fpath, '', content))
            
    vpath = os.path.join(ROOT, 'VERSION')
    vcontent = f'{target_ver}\n'
    if os.path.isfile(vpath):
        with open(vpath, 'r', encoding='utf-8') as f:
            old = f.read()
        if old.strip() != target_ver:
            diffs.append((vpath, old, vcontent))
    else:
        diffs.append((vpath, '', vcontent))

    # 2. readmd.py
    readmd_path = os.path.join(ROOT, 'readmd.py')
    with open(readmd_path, 'r', encoding='utf-8') as f:
        src = f.read()
    new_src = re.sub(r"(_env_or_bundle_version\(\)\s*or\s*')[^']+'\)", f"\\g<1>{target_ver}')", src)
    if new_src != src:
        diffs.append((readmd_path, src, new_src))

    # 3. installer/setup_app.py
    setup_path = os.path.join(ROOT, 'installer', 'setup_app.py')
    with open(setup_path, 'r', encoding='utf-8') as f:
        src = f.read()
    new_src = re.sub(r"(_env_or_bundle_version\(\)\s*or\s*')[^']+'\)", f"\\g<1>{target_ver}')", src)
    if new_src != src:
        diffs.append((setup_path, src, new_src))

    # 4. packages/vscode-extension/package.json
    vscode_pkg = os.path.join(ROOT, 'packages', 'vscode-extension', 'package.json')
    with open(vscode_pkg, 'r', encoding='utf-8') as f:
        data = json.load(f)
    if data.get('version') != target_ver:
        data['version'] = target_ver
        new_content = json.dumps(data, ensure_ascii=False, indent=2) + '\n'
        diffs.append((vscode_pkg, '', new_content))

    # 5. packages/harmonyos-app/AppScope/app.json5
    app_json5 = os.path.join(ROOT, 'packages', 'harmonyos-app', 'AppScope', 'app.json5')
    with open(app_json5, 'r', encoding='utf-8') as f:
        src = f.read()
    new_src = re.sub(r'"versionCode":\s*\d+', f'"versionCode": {harmony_code}', src)
    new_src = re.sub(r'"versionName":\s*"[^"]+"', f'"versionName": "{target_ver}"', new_src)
    if new_src != src:
        diffs.append((app_json5, src, new_src))

    # 6. packages/linglong/linglong.yaml
    linglong_yaml = os.path.join(ROOT, 'packages', 'linglong', 'linglong.yaml')
    with open(linglong_yaml, 'r', encoding='utf-8') as f:
        src = f.read()
    new_src = re.sub(r'(version:\s*)[0-9.]+', f'\\g<1>{linglong_ver}', src)
    if new_src != src:
        diffs.append((linglong_yaml, src, new_src))

    # 7. .github/workflows/release.yml
    release_yml = os.path.join(ROOT, '.github', 'workflows', 'release.yml')
    with open(release_yml, 'r', encoding='utf-8') as f:
        src = f.read()
    new_src = re.sub(r"READMD_VERSION:\s*'[^']+'", f"READMD_VERSION: '{target_ver}'", src)
    if new_src != src:
        diffs.append((release_yml, src, new_src))

    # 8. README files
    for rname in ('README.md', 'README.en.md', 'README.ja.md'):
        rpath = os.path.join(ROOT, rname)
        if not os.path.isfile(rpath):
            continue
        with open(rpath, 'r', encoding='utf-8') as f:
            src = f.read()
        # 替换 badge
        new_src = re.sub(r'badge/version-v[0-9a-zA-Z.-]+-3b6ef5', f'badge/version-v{target_ver}-3b6ef5', src)
        # 替换下载资产后缀
        new_src = re.sub(r'(-v)[0-9a-zA-Z.-]+(\.(?:exe|zip|AppImage|deb|hap|vsix))', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(_)[0-9a-zA-Z.-]+(_amd64\.deb)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(vscode-)[0-9a-zA-Z.-]+(\.vsix)', f'\\g<1>{target_ver}\\g<2>', new_src)
        new_src = re.sub(r'(server-)[0-9a-zA-Z.-]+(\.zip)', f'\\g<1>{target_ver}\\g<2>', new_src)
        if new_src != src:
            diffs.append((rpath, src, new_src))

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

    print(f'\n[SUCCESS] Successfully synchronized all platform version files to {target_ver}!')
    return True


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='ReadMD Unified Version Synchronization Tool')
    parser.add_argument('version', nargs='?', default=None, help='Target version (e.g. 2.3.7-beta.1)')
    parser.add_argument('--check', action='store_true', help='Check if all files are in sync without writing')
    args = parser.parse_args()

    ver = args.version or load_env_version()
    success = sync_all(ver, check_only=args.check)
    sys.exit(0 if success else 1)
