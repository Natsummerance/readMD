# -*- coding: utf-8 -*-
"""批量补齐 / 清洗 assets/i18n 下 46 个语种字典。

两类问题分开处理：

1. `--missing`：zh-CN.json（i18n_sync 的基准字典）里有的 key，某个语种文件里没有。
   前端 i18n.t() 查不到 key 时会把 key 原样打到界面上，所以缺词条 = 46 种语言同时泄漏。
2. `--english-leak`：某个语种的词条值与 en.json 完全相同，而 zh-CN.json 与 en.json 不同。
   这说明该词条当年根本没翻译，只是把英文抄了一遍。

`tools/i18n_sync.py --google` 只做第 1 类，且当译文恰好等于中文原文时（zh-TW/zh-HK 很常见）
会拒绝写入，导致繁体语种永远补不上；本工具对简中/繁体目标允许同值写入。
"""

import argparse
import json
import os
import re
import sys
import time

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(ROOT_DIR, 'assets', 'i18n')
META_FILE = os.path.join(I18N_DIR, 'meta.json')
BASE_FILE = os.path.join(I18N_DIR, 'zh-CN.json')
EN_FILE = os.path.join(I18N_DIR, 'en.json')
SOURCE_LANG = 'zh-CN'

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from i18n_sync import google_translate_text  # noqa: E402

PLACEHOLDER_RE = re.compile(r'\{[A-Za-z0-9_]+\}')

# 产品名 / 品牌名 / 文件名默认值：这些值刻意保持拉丁字母，机翻会破坏可识别性
ENGLISH_LEAK_SKIP_KEYS = (
    re.compile(r'^plugin\.[A-Za-z0-9_\-]+\.name$'),
    re.compile(r'\.ffmpeg'),
    re.compile(r'^export\.defaultExportName$'),
    re.compile(r'^menu\.lang'),
)


def load_json(path):
    with open(path, 'r', encoding='utf-8') as handle:
        return json.load(handle)


def save_json(path, data):
    with open(path, 'w', encoding='utf-8', newline='\r\n') as handle:
        json.dump(data, handle, ensure_ascii=False, indent=2)
        handle.write('\n')


def locales(meta):
    for lang in sorted(meta):
        if lang == SOURCE_LANG:
            continue
        path = os.path.join(I18N_DIR, f'{lang}.json')
        if os.path.isfile(path):
            yield lang, path


def usable(text, source, lang):
    """判定机翻结果是否可写入：占位符必须齐全，且非中文语种不得退回中文原文。"""
    if not text or not text.strip():
        return False
    if set(PLACEHOLDER_RE.findall(text)) != set(PLACEHOLDER_RE.findall(source)):
        return False
    if text == source and not lang.startswith('zh'):
        return False
    return True


def fill_missing(langs, base, only_keys, sleep):
    changed = 0
    for lang, path in langs:
        data = load_json(path)
        missing = [k for k in base if k not in data]
        if only_keys:
            missing = [k for k in missing if k in only_keys]
        if not missing:
            continue
        for key in missing:
            source = base[key]
            if not isinstance(source, str):
                continue
            time.sleep(sleep)
            value = google_translate_text(source, lang, SOURCE_LANG)
            if usable(value, source, lang):
                data[key] = value
        new_data = {k: data[k] for k in sorted(data)}
        if new_data != load_json(path):
            save_json(path, new_data)
            added = [k for k in missing if k in new_data]
            print(f'[OK] {lang:<8} 补齐 {len(added)}/{len(missing)} 词条')
            changed += 1
        else:
            print(f'[--] {lang:<8} 翻译未产出可用结果，保持缺失: {missing[:4]}')
    return changed


def sweep_english(langs, base, en, sleep):
    changed = 0
    for lang, path in langs:
        data = load_json(path)
        leaks = [k for k, v in data.items()
                 if isinstance(v, str) and k in en and v == en[k]
                 and isinstance(base.get(k), str) and base[k] != en[k]
                 and not any(p.match(k) for p in ENGLISH_LEAK_SKIP_KEYS)]
        if not leaks:
            continue
        for key in leaks:
            source = base[key]
            time.sleep(sleep)
            value = google_translate_text(source, lang, SOURCE_LANG)
            if usable(value, source, lang) and value != en[key]:
                data[key] = value
        if {k: data[k] for k in sorted(data)} != {k: load_json(path)[k] for k in sorted(load_json(path))}:
            save_json(path, {k: data[k] for k in sorted(data)})
            print(f'[OK] {lang:<8} 重译 {len(leaks)} 条英文残留')
            changed += 1
    return changed


def main():
    parser = argparse.ArgumentParser(description='i18n 缺词条补齐 / 英文残留清洗')
    parser.add_argument('--missing', action='store_true', help='补齐 zh-CN 有而目标语种缺的词条')
    parser.add_argument('--english-leak', action='store_true', help='把等于英文原值的词条重译')
    parser.add_argument('--keys', default='', help='逗号分隔，仅处理这些 key')
    parser.add_argument('--langs', default='', help='逗号分隔，仅处理这些语种')
    parser.add_argument('--sleep', type=float, default=0.15, help='每次请求间隔秒数')
    args = parser.parse_args()

    if not (args.missing or args.english_leak):
        parser.error('至少指定 --missing 或 --english-leak')

    base = load_json(BASE_FILE)
    en = load_json(EN_FILE)
    meta = load_json(META_FILE)
    wanted = {k.strip() for k in args.keys.split(',') if k.strip()}
    only_langs = {l.strip() for l in args.langs.split(',') if l.strip()}
    langs = [(l, p) for l, p in locales(meta) if not only_langs or l in only_langs]

    if args.missing:
        print(f'[*] 补齐缺词条：{len(langs)} 语种，基准 {len(base)} 词条')
        fill_missing(langs, base, wanted, args.sleep)
    if args.english_leak:
        print(f'[*] 清洗英文残留：{len(langs)} 语种')
        sweep_english(langs, base, en, args.sleep)


if __name__ == '__main__':
    main()
