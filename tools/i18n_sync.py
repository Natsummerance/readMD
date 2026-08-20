import logging
'ReadMD 全球化多语言 (i18n) 自动同步与多模型翻译工具。\n\n支持：\n1. 以 assets/i18n/zh-CN.json 为基准源字典；\n2. 自动化同步校验 45+ 语种词条完整性（缺少 Key 自动告警或补齐）；\n3. 支持 Google Translate 翻译引擎；\n4. 兼容 OpenAI 标准 API（DeepSeek / Qwen / Mimo / Gemini / GLM 等轻量大模型）；\n5. 注入专业 Markdown 与软件 UI 语境 Prompt，严格保留 {placeholder} 占位符结构。\n'
import argparse
import json
import os
import re
import sys
import time
import urllib.request
import urllib.parse
if sys.platform == 'win32' and hasattr(sys.stdout, 'reconfigure'):
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
        sys.stderr.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        logging.warning('Silent exception caught in tools.i18n_sync: Exception')
ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
I18N_DIR = os.path.join(ROOT_DIR, 'assets', 'i18n')
META_FILE = os.path.join(I18N_DIR, 'meta.json')
BASE_FILE = os.path.join(I18N_DIR, 'zh-CN.json')

def load_json(fp):
    if not os.path.isfile(fp):
        return {}
    with open(fp, 'r', encoding='utf-8') as f:
        return json.load(f)

def save_json(fp, data):
    os.makedirs(os.path.dirname(fp), exist_ok=True)
    with open(fp, 'w', encoding='utf-8') as f:
        json.dump(data, f, ensure_ascii=False, indent=2)
        f.write('\n')

def google_translate_text(text, target_lang, source_lang='zh-CN'):
    """使用 Google Translate 免费 API 进行高质量快速翻译。"""
    if not text or not text.strip():
        return text
    placeholders = re.findall('\\{[a-zA-Z0-9_]+\\}', text)
    masked_text = text
    for (i, p) in enumerate(placeholders):
        masked_text = masked_text.replace(p, f'__PH_{i}__')
    glang = target_lang.split('-')[0]
    if target_lang in ('zh-TW', 'zh-HK'):
        glang = 'zh-TW'
    elif target_lang == 'zh-CN':
        glang = 'zh-CN'
    elif target_lang == 'he':
        glang = 'iw'
    elif target_lang == 'fil':
        glang = 'tl'
    url = 'https://translate.googleapis.com/translate_a/single?client=gtx&sl=' + source_lang + '&tl=' + glang + '&dt=t&q=' + urllib.parse.quote(masked_text)
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read().decode('utf-8'))
            translated = ''.join([item[0] for item in data[0] if item[0]])
            for (i, p) in enumerate(placeholders):
                translated = re.sub(f'__\\s*PH_{i}\\s*__', p, translated)
                translated = translated.replace(f'__PH_{i}__', p)
            return translated
    except Exception as e:
        logging.warning('Silent exception caught in tools.i18n_sync: Exception')
        print(f'[WARN] Google Translate error for {target_lang}: {e}')
        return text

def llm_translate_dict(source_dict, target_lang, target_lang_name, api_base, api_key, model='deepseek-chat'):
    """使用兼容 OpenAI 协议的大模型（DeepSeek / Qwen / Mimo 等）批量高质量翻译字典。"""
    system_prompt = f'You are a professional localization and internationalization expert for ReadMD, a modern high-performance Markdown reader and editor desktop application.\nTranslate the following UI strings from Chinese into {target_lang_name} ({target_lang}).\n\nSTRICT RULES:\n1. Maintain professional, natural, native software terminology (e.g. File, Edit, View, Table of Contents, Markdown, LaTeX, Export, OCR, Settings).\n2. DO NOT change or translate placeholders in curly braces like {{count}}, {{name}}, {{version}}, {{time}}, {{percent}}, {{mb}}, {{fmt}}.\n3. Keep concise UI button lengths suitable for compact menus and toolbars.\n4. Output valid JSON ONLY matching the exact input keys. Do not include markdown codeblocks or extra text.'
    user_prompt = f'Target language: {target_lang_name} ({target_lang})\nInput JSON:\n{json.dumps(source_dict, ensure_ascii=False, indent=2)}'
    headers = {'Content-Type': 'application/json', 'Authorization': f'Bearer {api_key}'}
    payload = {'model': model, 'messages': [{'role': 'system', 'content': system_prompt}, {'role': 'user', 'content': user_prompt}], 'temperature': 0.1}
    req_url = api_base.rstrip('/') + '/chat/completions'
    req = urllib.request.Request(req_url, data=json.dumps(payload).encode('utf-8'), headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            res_data = json.loads(resp.read().decode('utf-8'))
            content = res_data['choices'][0]['message']['content'].strip()
            content = re.sub('^```json\\s*', '', content)
            content = re.sub('\\s*```$', '', content)
            return json.loads(content)
    except Exception as e:
        logging.warning('Silent exception caught in tools.i18n_sync: Exception')
        print(f'[ERROR] LLM translate failed for {target_lang}: {e}')
        return None

def validate_all_locales():
    """验证全部语种字典完整性。"""
    base = load_json(BASE_FILE)
    meta = load_json(META_FILE)
    base_keys = set(base.keys())
    all_passed = True
    print(f'[*] 基准词条数 (zh-CN): {len(base_keys)}')
    print(f'[*] 语种总数: {len(meta)}')
    for (lang, m) in meta.items():
        fp = os.path.join(I18N_DIR, f'{lang}.json')
        if not os.path.isfile(fp):
            print(f"[-] 缺失语种文件: {lang}.json ({m.get('name')})")
            all_passed = False
            continue
        d = load_json(fp)
        d_keys = set(d.keys())
        missing = base_keys - d_keys
        extra = d_keys - base_keys
        if missing:
            print(f'[!] {lang}.json 缺少 {len(missing)} 个词条: {list(missing)[:5]}...')
            all_passed = False
        else:
            print(f"[OK] {lang:<8} {m.get('name'):<12} ({m.get('native')}): 100% 完整 ({len(d_keys)} 词条)")
    return all_passed

def main():
    parser = argparse.ArgumentParser(description='ReadMD i18n 多语言同步与翻译工具')
    parser.add_argument('--validate-only', action='store_true', help='仅校验字典完整性')
    parser.add_argument('--google', action='store_true', help='使用 Google Translate 自动补齐缺失词条')
    parser.add_argument('--llm', action='store_true', help='使用 LLM (DeepSeek/Qwen/OpenAI) 自动翻译')
    parser.add_argument('--api-base', default='https://api.deepseek.com/v1', help='OpenAI 兼容 API Base URL')
    parser.add_argument('--api-key', default=os.environ.get('OPENAI_API_KEY') or os.environ.get('DEEPSEEK_API_KEY', ''), help='API 密钥')
    parser.add_argument('--model', default='deepseek-chat', help='模型名称')
    args = parser.parse_args()
    if args.validate_only:
        ok = validate_all_locales()
        sys.exit(0 if ok else 1)
    print('[*] 启动 i18n 字典检查与同步...')
    validate_all_locales()
if __name__ == '__main__':
    main()