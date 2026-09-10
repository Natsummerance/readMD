# -*- coding: utf-8 -*-
"""扫描前端 JS 里所有 i18n 调用点，找出字典中不存在的 key。

存在的意义：assets/js/core/i18n.js 在查不到 key 时返回 key 本身，所以任何缺失
都会在 46 种语言里直接把 `plugin.installFailed` 打到界面上，而 tools/check_i18n_keys.py
只看得到 index.html 的 data-i18n*，看不到 JS 调用点。这条门禁补的正是那个洞。

用法：
    python tools/check_js_i18n_keys.py            # 缺失则退出码 1
    python tools/check_js_i18n_keys.py --json      # 机器可读输出
    python tools/check_js_i18n_keys.py --all-locales  # 额外校验 46 语种 parity
"""

import argparse
import json
import os
import re
import sys

if sys.platform == 'win32' and hasattr(sys.stdout, 'reconfigure'):
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
        sys.stderr.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(ROOT_DIR, 'assets', 'i18n')
META_FILE = os.path.join(I18N_DIR, 'meta.json')

SCAN_DIRS = [os.path.join(ROOT_DIR, 'assets', 'js')]
SCAN_FILES = [os.path.join(ROOT_DIR, 'assets', 'app.js')]

# _t('x') / _t("x") / i18n.t('x') / window.i18n.t("x")，允许第一个参数后紧跟逗号传参。
# 末尾 lookahead 排除 `'前缀' + 变量` 这种拼接写法：那不是完整 key。
LITERAL_RE = re.compile(
    r'(?:^|[^\w$.])(?:_t|t)\(\s*(["\'])([A-Za-z0-9_]+\.[A-Za-z0-9_.]+)\1(?!\s*\+)'
)
# 第一个参数不是字符串字面量：静态扫描无法判定，单独列出而不是当作通过
DYNAMIC_RE = re.compile(r'(?:^|[^\w$.])(?:_t|t)\(\s*(?![\'"`])\S')
CONCAT_RE = re.compile(r'(?:^|[^\w$.])(?:_t|t)\(\s*(["\'])([A-Za-z0-9_]+\.[A-Za-z0-9_.]*)\1\s*\+')
T_MEMBER_RE = re.compile(r'\.t\(\s*(["\'])([A-Za-z0-9_]+\.[A-Za-z0-9_.]+)\1(?!\s*\+)')


def js_sources():
    paths = list(SCAN_FILES)
    for base in SCAN_DIRS:
        for dirpath, _dirnames, filenames in os.walk(base):
            for name in sorted(filenames):
                if name.endswith('.js'):
                    paths.append(os.path.join(dirpath, name))
    return sorted(dict.fromkeys(p for p in paths if os.path.isfile(p)))


def strip_comments(line, in_block):
    """去掉 // 与 /* */ 注释内容，返回 (可扫描文本, 是否仍在块注释中)。"""
    out = []
    i = 0
    while i < len(line):
        if in_block:
            end = line.find('*/', i)
            if end < 0:
                return ''.join(out), True
            i = end + 2
            in_block = False
            continue
        if line.startswith('/*', i):
            in_block = True
            i += 2
            continue
        if line.startswith('//', i):
            break
        out.append(line[i])
        i += 1
    return ''.join(out), in_block


def collect_keys():
    """返回 (literal: {key: [locations]}, dynamic: [locations], prefixes: {prefix: [locations]})。"""
    literal = {}
    dynamic = []
    prefixes = {}
    for path in js_sources():
        rel = os.path.relpath(path, ROOT_DIR).replace('\\', '/')
        with open(path, 'r', encoding='utf-8') as handle:
            lines = handle.read().splitlines()
        in_block = False
        for number, raw in enumerate(lines, 1):
            text, in_block = strip_comments(raw, in_block)
            for match in LITERAL_RE.finditer(text):
                literal.setdefault(match.group(2), []).append(f'{rel}:{number}')
            for match in T_MEMBER_RE.finditer(text):
                literal.setdefault(match.group(2), []).append(f'{rel}:{number}')
            for match in CONCAT_RE.finditer(text):
                prefixes.setdefault(match.group(2), []).append(f'{rel}:{number}')
            if DYNAMIC_RE.search(text):
                dynamic.append(f'{rel}:{number}')
    return literal, dynamic, prefixes


def check_locale_parity(keys):
    """确认每个 key 在全部 46 个 locale 中都存在，返回 {locale: [missing]}。"""
    meta = json.load(open(META_FILE, 'r', encoding='utf-8'))
    gaps = {}
    for lang in meta:
        fp = os.path.join(I18N_DIR, f'{lang}.json')
        if not os.path.isfile(fp):
            gaps[lang] = ['<文件缺失>']
            continue
        present = set(json.load(open(fp, 'r', encoding='utf-8')).keys())
        missing = sorted(keys - present)
        if missing:
            gaps[lang] = missing
    return gaps


def main():
    parser = argparse.ArgumentParser(description='前端 i18n 调用点门禁')
    parser.add_argument('--json', action='store_true', help='输出 JSON')
    parser.add_argument('--all-locales', action='store_true',
                        help='额外校验缺失 key 在 46 语种中的覆盖')
    parser.add_argument('--allow-list', default='',
                        help='逗号分隔的豁免 key（动态拼接 key 的已知值）')
    args = parser.parse_args()

    allowed = {k.strip() for k in args.allow_list.split(',') if k.strip()}
    literal, dynamic, prefixes = collect_keys()
    en_path = os.path.join(I18N_DIR, 'en.json')
    en_keys = set(json.load(open(en_path, 'r', encoding='utf-8')).keys())

    found = set(literal)
    missing = {k: literal[k] for k in sorted(found - en_keys - allowed)}
    # 拼接式 key 只能校验到前缀：前缀下一个词条都没有时，该命名空间必然整体泄漏
    empty_prefixes = {p: where for p, where in sorted(prefixes.items())
                      if not any(k.startswith(p) for k in en_keys)}

    if args.json:
        print(json.dumps({
            'scanned_files': len(js_sources()),
            'literal_keys': len(found),
            'missing': missing,
            'empty_prefixes': empty_prefixes,
            'dynamic_call_sites': dynamic,
            'concat_call_sites': prefixes,
        }, ensure_ascii=False, indent=2))
        sys.exit(1 if missing or empty_prefixes else 0)

    print(f'[*] 扫描 JS 文件: {len(js_sources())} 个')
    print(f'[*] 字面量 i18n key: {len(found)} 个 (调用点 '
          f'{sum(len(v) for v in literal.values())} 处)')
    print(f'[*] 基准字典 en.json: {len(en_keys)} 词条')

    if missing:
        print(f'\n[!] JS 调用点缺失 {len(missing)} 个 key —— 界面会直接显示点号 key：')
        for key, where in missing.items():
            print(f'    {key}  <-  {", ".join(where[:4])}'
                  + (f'  (+{len(where) - 4} 处)' if len(where) > 4 else ''))
    else:
        print('[OK] 所有 JS 字面量 key 均存在于 en.json')

    if empty_prefixes:
        print(f'\n[!] 拼接 key 的前缀在 en.json 中一个词条都没有：')
        for prefix, where in empty_prefixes.items():
            print(f'    {prefix}<变量>  <-  {", ".join(where[:4])}')

    if prefixes:
        print(f'\n[~] {len(prefixes)} 个拼接式 key 前缀仅能校验到前缀，后缀需人工确认：')
        for prefix, where in sorted(prefixes.items()):
            hit = sum(1 for k in en_keys if k.startswith(prefix))
            print(f'    {prefix}*  ({hit} 词条)  <-  {", ".join(where[:3])}')

    if dynamic:
        print(f'\n[~] {len(dynamic)} 处动态 key 调用无法静态校验，需人工确认对应值已在字典中：')
        for site in dynamic:
            print(f'    {site}')

    if args.all_locales:
        gaps = check_locale_parity(found)
        if gaps:
            print(f'\n[!] {len(gaps)} 个 locale 缺少 JS 用到的 key：')
            for lang, missed in sorted(gaps.items()):
                print(f'    {lang}: {len(missed)} 缺 -> {missed[:6]}')
        else:
            print('[OK] JS 用到的 key 在全部 locale 中均已覆盖')

    sys.exit(1 if (missing or empty_prefixes) else 0)


if __name__ == '__main__':
    main()
