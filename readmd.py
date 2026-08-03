#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""ReadMD —— 轻量级本地 Markdown 阅读器。

特性：
  - 本地 127.0.0.1 HTTP 服务 + pywebview 原生窗口，秒开
  - 渲染前自动修正常见错误（表格 / 加粗 / 公式 / 标题），只影响显示
  - 自动刷新、目录、搜索、主题、字号、最近文件、文件夹浏览、打印
  - 全部资源离线（marked + MathJax 已内置），无需联网

用法：
  python readmd.py [文件.md]        # 打开文件（或空启动）
  python readmd.py --browser [文件] # 用默认浏览器打开（无 pywebview 时兜底）
  python readmd.py --selftest       # 自测（修正器 + 本地服务）
"""

import argparse
import json
import logging
import mimetypes
import os
import re
import secrets
import socket
import subprocess
import sys
import time
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, quote, unquote, urlparse

import readmd_fix
import readmd_modules as RM

APP_DIR = sys._MEIPASS if getattr(sys, 'frozen', False) else os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(os.environ.get('APPDATA') or os.path.expanduser('~'), 'ReadMD')
SETTINGS_FILE = os.path.join(DATA_DIR, 'settings.json')
RECENT_FILE = os.path.join(DATA_DIR, 'recent.json')
PROMPTS_FILE = os.path.join(DATA_DIR, 'prompts.json')
HISTORY_FILE = os.path.join(DATA_DIR, 'chat_history.json')
LOG_FILE = os.path.join(DATA_DIR, 'readmd.log')
VERSION = '1.2.0'

MD_EXTS = ('.md', '.markdown', '.mdown', '.mkd', '.mdx', '.txt')


def safe_print(*args, **kwargs):
    try:
        if sys.stdout is not None:
            print(*args, **kwargs)
    except Exception:
        pass


def setup_logging():
    try:
        os.makedirs(DATA_DIR, exist_ok=True)
        logging.basicConfig(
            filename=LOG_FILE, level=logging.INFO, encoding='utf-8',
            format='%(asctime)s %(levelname)s %(message)s')
    except Exception:
        pass


def load_json(path, default):
    try:
        with open(path, 'r', encoding='utf-8') as f:
            return json.load(f)
    except Exception:
        return default


def save_json(path, data):
    try:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        tmp = path + '.tmp'
        with open(tmp, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        os.replace(tmp, path)
    except Exception as e:
        logging.exception('save_json failed: %s', path)



# ---------------------------------------------------------------- AI 模板 / 历史会话

# 内置 Prompt 模板（只读；可覆盖为自定义版本，或另存为自定义模板）
BUILTIN_PROMPTS = [
    {"id": "quick_read", "name": "快速阅读", "action": "quick_read",
     "system": "你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。",
     "user": ""},
    {"id": "polish", "name": "润色", "action": "polish",
     "system": "你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "modify", "name": "修改", "action": "modify",
     "system": "你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。只输出修改后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "expand", "name": "扩充", "action": "expand",
     "system": "你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。只输出扩充后的完整文档，不要加任何解释。",
     "user": ""},
    {"id": "continue", "name": "续写", "action": "continue",
     "system": "你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。",
     "user": ""},
    {"id": "translate", "name": "翻译", "action": "translate",
     "system": "你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。",
     "user": ""},
    {"id": "ask", "name": "提问", "action": "ask",
     "system": "你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。",
     "user": ""},
    {"id": "summary", "name": "总结要点", "action": "ask",
     "system": "你是文档总结助手。用 5 条以内要点概括用户文档的核心内容，输出为 Markdown 列表；最后用一句话总结全文。",
     "user": ""},
    {"id": "outline", "name": "生成大纲", "action": "ask",
     "system": "你是文档策划。为用户文档生成层级目录大纲（# / ## / ###），只输出大纲，不要其他内容。",
     "user": ""},
    {"id": "weekly", "name": "生成周报", "action": "ask",
     "system": "你是周报助手。根据用户给出的工作内容，整理成结构化周报：本周完成 / 下周计划 / 风险与求助。只输出周报正文。",
     "user": ""},
    {"id": "to_english", "name": "翻译成英文", "action": "translate",
     "system": "你是专业翻译。将用户给出的文档翻译成英文，保留 Markdown 结构、表格与代码块，只输出译文。",
     "user": ""},
    {"id": "code_review", "name": "代码审查", "action": "ask",
     "system": "你是资深代码审查员。审查用户文档中的代码块：指出 bug、安全隐患、可读性问题，并给出修改建议与示例代码。用 Markdown 输出。",
     "user": ""},
    {"id": "action_items", "name": "提取行动项", "action": "ask",
     "system": "你是任务管理助手。从用户文档中提取可执行行动项，用 Markdown 表格输出：事项 / 负责人 / 截止时间 / 优先级。",
     "user": ""},
    {"id": "fix_format", "name": "修正 Markdown 格式", "action": "modify",
     "system": "你是 Markdown 格式专家。修正文档中的格式问题：表格对齐、加粗符号配对、公式写法、标题层级。只输出修正后的完整文档，不要解释。",
     "user": ""},
]


def load_prompts():
    """内置 + 自定义模板合并；自定义可覆盖同名内置。"""
    d = load_json(PROMPTS_FILE, {})
    customs = d.get('templates', [])
    by_id = {t.get('id'): t for t in customs}
    merged = []
    seen = set()
    for b in BUILTIN_PROMPTS:
        bid = b.get('id')
        seen.add(bid)
        merged.append(dict(by_id.get(bid, b), builtin=True))
    for c in customs:
        cid = c.get('id')
        if cid in seen:
            continue
        merged.append(dict(c, builtin=False))
    return {'templates': merged}


def save_prompt(template):
    """新增 / 更新模板。id 为空时自动生成；内置 id 表示覆盖内置模板。"""
    t = dict(template or {})
    if not t.get('id'):
        t['id'] = 't_%d' % int(time.time() * 1000)
    if not t.get('name'):
        t['name'] = '未命名模板'
    t.pop('builtin', None)
    d = load_json(PROMPTS_FILE, {})
    customs = [c for c in d.get('templates', []) if c.get('id') != t.get('id')]
    customs.append(t)
    save_json(PROMPTS_FILE, {'templates': customs})
    return t


def delete_prompt(prompt_id):
    d = load_json(PROMPTS_FILE, {})
    d['templates'] = [t for t in d.get('templates', []) if t.get('id') != prompt_id]
    save_json(PROMPTS_FILE, d)
    return True


def load_history(limit=50):
    d = load_json(HISTORY_FILE, {'sessions': []})
    return d.get('sessions', [])[:limit]


def save_session(session):
    """新增 / 更新会话（按 id upsert），限制会话 50 个、消息 60 条。"""
    s = dict(session or {})
    now = time.time()
    if not s.get('id'):
        s['id'] = 'h_%d' % int(now * 1000)
    s['created'] = s.get('created') or now
    s['updated'] = now
    msgs = (s.get('messages') or [])[-60:]
    s['messages'] = msgs
    s['msgCount'] = len(msgs)
    sessions = [x for x in load_history(500) if x.get('id') != s['id']]
    sessions.insert(0, s)
    save_json(HISTORY_FILE, {'sessions': sessions[:50]})
    return s


def delete_session(session_id):
    sessions = [x for x in load_history(500) if x.get('id') != session_id]
    save_json(HISTORY_FILE, {'sessions': sessions})
    return True


def read_text(path):
    """按编码优先级读取文本文件（UTF-8 / GB18030 / Big5 / Latin-1）。"""
    with open(path, 'rb') as f:
        data = f.read()
    if data.startswith(b'\xef\xbb\xbf'):
        return data.decode('utf-8-sig'), 'utf-8-sig'
    for enc in ('utf-8', 'gb18030', 'big5', 'latin-1'):
        try:
            return data.decode(enc), enc
        except (UnicodeDecodeError, LookupError):
            continue
    return data.decode('utf-8', errors='replace'), 'utf-8'


# ---------------------------------------------------------------- HTTP 服务

class Handler(BaseHTTPRequestHandler):
    server_version = 'ReadMD/' + VERSION
    LAN_TOKEN = None

    def log_message(self, fmt, *args):
        pass  # 静默访问日志

    def do_GET(self):
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._route()
        except Exception as e:
            logging.exception('http error: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            except Exception:
                pass

    def do_POST(self):
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._route()
        except Exception as e:
            logging.exception('http post error: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', ('error: %s' % e).encode('utf-8'))
            except Exception:
                pass

    def _lan_authorized(self):
        """局域网模式下，除页面与静态资源外，所有 API 都要求携带 token。"""
        if not self.LAN_TOKEN:
            return True
        u = urlparse(self.path)
        if u.path in ('/', '/index.html') or u.path.startswith('/assets/'):
            return True
        qs = parse_qs(u.query)
        if qs.get('t', [''])[0] == self.LAN_TOKEN:
            return True
        return self.headers.get('X-ReadMD-Token', '') == self.LAN_TOKEN

    def _route(self):
        u = urlparse(self.path)
        path = u.path
        qs = parse_qs(u.query)
        if path in ('/', '/index.html'):
            self._send_index()
        elif path.startswith('/assets/'):
            rel = path[len('/assets/'):]
            fp = os.path.normpath(os.path.join(APP_DIR, 'assets', rel))
            base = os.path.normpath(os.path.join(APP_DIR, 'assets'))
            if not fp.startswith(base):
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return
            mime = mimetypes.guess_type(fp)[0] or 'application/octet-stream'
            if mime.startswith('text/') or mime in ('application/javascript', 'application/json'):
                mime += '; charset=utf-8'
            self._send_file(fp, mime)
        elif path == '/api/file':
            p = unquote(qs.get('p', [''])[0])
            if not p:
                self._send(400, 'text/plain; charset=utf-8', b'missing p')
                return
            self._api_file(p, qs.get('meta', ['0'])[0] == '1')
        elif path == '/api/list':
            p = unquote(qs.get('p', [''])[0])
            self._api_list(p)
        elif path == '/api/modules':
            RM.load_all()  # 幂等：任意前端轮询即触发后台加载（渲染完成后的首次轮询才会发生）
            st, err = RM.status()
            self._send_json(200, {'modules': st, 'errors': err})
        elif path == '/api/convert':
            p = unquote(qs.get('p', [''])[0])
            self._api_convert(p)
        elif path == '/api/ocr':
            p = unquote(qs.get('p', [''])[0])
            self._api_ocr(p)
        elif path == '/api/url':
            u = unquote(qs.get('u', [''])[0])
            crawl = qs.get('crawl', ['0'])[0] == '1'
            self._api_url(u, crawl)
        elif path == '/api/save':
            self._do_save()
        elif path == '/api/upload':
            self._do_upload(qs.get('ext', [''])[0])
        elif path == '/api/ai/config':
            self._api_ai_config()
        elif path == '/api/ai/chat':
            self._api_ai_chat()
        elif path == '/api/image/save':
            self._api_image_save()
        elif path == '/api/ai/prompts':
            self._api_ai_prompts()
        elif path == '/api/ai/history':
            self._api_ai_history()
        elif path == '/api/share/start':
            self._send_json(200, start_lan_server())
        elif path == '/api/share/stop':
            self._send_json(200, stop_lan_server())
        elif path == '/api/share/status':
            self._send_json(200, share_status())
        elif path == '/raw':
            p = unquote(qs.get('p', [''])[0])
            self._send_raw(p)
        else:
            self._send(404, 'text/plain; charset=utf-8', b'not found')

    def _send(self, code, ctype, body):
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-cache')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, code, obj):
        self._send(code, 'application/json; charset=utf-8',
                   json.dumps(obj, ensure_ascii=False).encode('utf-8'))

    def _send_index(self):
        """返回首页；局域网模式下注入 token 供前端 fetch 携带。"""
        fp = os.path.join(APP_DIR, 'assets', 'index.html')
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        with open(fp, 'rb') as f:
            data = f.read()
        if self.LAN_TOKEN:
            data = data.replace(b'window.LAN_TOKEN=null;',
                                ('window.LAN_TOKEN="%s";' % self.LAN_TOKEN).encode('utf-8'))
        self._send(200, 'text/html; charset=utf-8', data)

    def _sse(self, obj):
        try:
            self.wfile.write(('data: ' + json.dumps(obj, ensure_ascii=False) + '\n\n').encode('utf-8'))
            self.wfile.flush()
        except Exception:
            pass

    def _api_ai_config(self):
        if not RM.is_ready('ai'):
            RM.load_all()
            self._send_json(409, {'error': 'AI 模块加载中，请稍候再试'})
            return
        try:
            mod = RM.get('ai')
            if self.command == 'GET':
                self._send_json(200, mod.get_config())
            else:
                n = int(self.headers.get('Content-Length', 0) or 0)
                body = json.loads(self.rfile.read(n).decode('utf-8'))
                mod.save_config(body)
                self._send_json(200, {'ok': True})
        except Exception as e:
            logging.exception('ai config failed')
            self._send_json(500, {'error': 'AI 配置失败：%s' % e})

    def _api_ai_chat(self):
        """AI 对话：SSE 流式返回，兼容 OpenAI / Anthropic 双协议。"""
        if not RM.is_ready('ai'):
            RM.load_all()
            self._send_json(409, {'error': 'AI 模块加载中，请稍候再试'})
            return
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            payload = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '请求格式错误'})
            return
        try:
            mod = RM.get('ai')
            gen = mod.chat(payload)
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream; charset=utf-8')
            self.send_header('Cache-Control', 'no-cache')
            self.send_header('Connection', 'close')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            if isinstance(gen, str):
                self._sse({'d': gen})
                self._sse({'done': True})
                return
            for delta in gen:
                self._sse({'d': delta})
            self._sse({'done': True})
        except Exception as e:
            logging.exception('ai chat failed')
            try:
                self._sse({'error': str(e)})
                self._sse({'done': True})
            except Exception:
                pass

    def _api_image_save(self):
        """保存编辑后的图片到文档目录 images/ 子目录，返回相对路径。"""
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '无效请求'})
            return
        dir_path = body.get('dir') or ''
        data_b64 = body.get('data') or ''
        fmt = (body.get('format') or 'png').lower()
        name = body.get('name') or ''
        if not dir_path or not data_b64 or not os.path.isdir(dir_path):
            self._send_json(400, {'error': '缺少目录或图片数据'})
            return
        if fmt not in ('png', 'jpeg', 'jpg', 'webp'):
            fmt = 'png'
        try:
            import base64 as _b64
            raw = _b64.b64decode(data_b64)
            if not raw:
                self._send_json(400, {'error': '图片数据为空'})
                return
            img_dir = os.path.join(dir_path, 'images')
            os.makedirs(img_dir, exist_ok=True)
            if not name or not re.match(r'^[A-Za-z0-9_\-]+', name):
                name = 'img_%d_%s' % (int(time.time() * 1000), os.urandom(3).hex())
            if not name.lower().endswith('.' + fmt):
                name += '.' + fmt
            target = os.path.join(img_dir, name)
            with open(target, 'wb') as f:
                f.write(raw)
            rel = os.path.join('images', name).replace('\\', '/')
            self._send_json(200, {'ok': True, 'path': target, 'rel': rel})
        except Exception as e:
            logging.exception('image save failed')
            self._send_json(500, {'error': '图片保存失败：%s' % e})

    def _api_ai_prompts(self):
        """Prompt 模板：GET 列表，POST 保存/覆盖/删除。"""
        try:
            if self.command == 'GET':
                self._send_json(200, load_prompts())
                return
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            action = body.get('action', 'save')
            if action == 'delete':
                self._send_json(200, {'ok': delete_prompt(body.get('id') or '')})
            else:
                t = save_prompt(body.get('template') or {})
                self._send_json(200, {'ok': True, 'template': t})
        except Exception as e:
            logging.exception('ai prompts failed')
            self._send_json(500, {'error': '模板操作失败：%s' % e})

    def _api_ai_history(self):
        """AI 会话：GET 列表/详情，POST 保存/删除/清空。"""
        try:
            if self.command == 'GET':
                u = urlparse(self.path)
                qs = parse_qs(u.query)
                sid = qs.get('id', [''])[0]
                if sid:
                    for s in load_history(500):
                        if s.get('id') == sid:
                            self._send_json(200, {'session': s})
                            return
                    self._send_json(404, {'error': '会话不存在'})
                    return
                brief = [{k: s.get(k) for k in ('id', 'title', 'created', 'updated',
                                                'provider', 'model', 'doc', 'msgCount')}
                         for s in load_history()]
                self._send_json(200, {'sessions': brief})
                return
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
            action = body.get('action', 'save')
            if action == 'delete':
                self._send_json(200, {'ok': delete_session(body.get('id') or '')})
            elif action == 'clear':
                save_json(HISTORY_FILE, {'sessions': []})
                self._send_json(200, {'ok': True})
            else:
                sess = save_session(body.get('session') or {})
                self._send_json(200, {'ok': True, 'session': sess})
        except Exception as e:
            logging.exception('ai history failed')
            self._send_json(500, {'error': '会话操作失败：%s' % e})
    def _send_file(self, fp, ctype):
        if not os.path.isfile(fp):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        with open(fp, 'rb') as f:
            self._send(200, ctype, f.read())

    def _api_file(self, p, meta_only):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        try:
            st = os.stat(p)
        except OSError:
            self._send_json(404, {'error': '无法访问文件'})
            return
        name = os.path.basename(p)
        d = {
            'path': p, 'name': name, 'dir': os.path.dirname(p),
            'mtime': st.st_mtime, 'size': st.st_size,
        }
        if meta_only:
            self._send_json(200, d)
            return
        text, enc = read_text(p)
        fr = readmd_fix.fix_markdown(text)
        d.update({
            'encoding': enc,
            'content': fr.text,
            'original': text,
            'fixes': fr.fixes,
            'stats': fr.stats,
        })
        self._send_json(200, d)

    def _api_list(self, p):
        """递归列出目录下的 Markdown 文件（最多 4 层 / 500 个）。"""
        if not os.path.isdir(p):
            self._send_json(200, {'dir': p, 'files': []})
            return
        files = []
        for root, dirs, names in os.walk(p):
            dirs[:] = [x for x in dirs if not x.startswith(('.', '_'))]
            depth = root[len(p):].count(os.sep)
            if depth >= 4:
                dirs[:] = []
                continue
            for n in sorted(names):
                if n.lower().endswith(MD_EXTS):
                    files.append(os.path.join(root, n))
            if len(files) >= 500:
                break
        self._send_json(200, {'dir': p, 'files': files[:500]})

    def _api_convert(self, p):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        if not RM.is_ready('convert'):
            RM.load_all()
            self._send_json(409, {'error': '转换模块加载中，请稍候再试'})
            return
        try:
            mod = RM.get('convert')
            text = mod.convert(p) or ''
            if not text.strip():
                self._send_json(200, {'content': '', 'name': os.path.basename(p),
                                      'dir': os.path.dirname(p), 'source': 'convert',
                                      'note': '未提取到文字，可尝试“扫描转 MD”（OCR）'})
                return
            fr = readmd_fix.fix_markdown(text)
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                                  'name': os.path.basename(p),
                                  'dir': os.path.dirname(p), 'source': 'convert', 'path': p})
        except Exception as e:
            logging.exception('convert failed: %s', p)
            self._send_json(500, {'error': '转换失败：%s' % e})

    def _api_ocr(self, p):
        if not os.path.isfile(p):
            self._send_json(404, {'error': '文件不存在'})
            return
        if not RM.is_ready('ocr'):
            RM.load_all()
            self._send_json(409, {'error': 'OCR 模块加载中，请稍候再试'})
            return
        try:
            mod = RM.get('ocr')
            text = mod.ocr_any(p)
            fr = readmd_fix.fix_markdown(text or '')
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                                  'name': os.path.basename(p),
                                  'dir': os.path.dirname(p), 'source': 'ocr', 'path': p})
        except Exception as e:
            logging.exception('ocr failed: %s', p)
            self._send_json(500, {'error': 'OCR 失败：%s' % e})

    def _api_url(self, u, crawl):
        if not u:
            self._send_json(400, {'error': '缺少 URL'})
            return
        if not RM.is_ready('web'):
            RM.load_all()
            self._send_json(409, {'error': '网页模块加载中，请稍候再试'})
            return
        try:
            mod = RM.get('web')
            text = mod.crawl(u) if crawl else mod.fetch_url(u)
            if not text:
                self._send_json(200, {'content': '', 'name': u, 'dir': '',
                                      'source': 'url', 'note': '未能从该网页提取到正文'})
                return
            fr = readmd_fix.fix_markdown(text)
            self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                                  'name': u, 'dir': '', 'source': 'url', 'path': u})
        except Exception as e:
            logging.exception('url convert failed: %s', u)
            self._send_json(500, {'error': '抓取失败：%s' % e})

    def _do_upload(self, ext):
        """浏览器兜底模式：接收文件字节写入临时目录，返回可转换的路径。"""
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            data = self.rfile.read(n)
            if not data:
                self._send_json(400, {'error': '空文件'})
                return
            upload_dir = os.path.join(DATA_DIR, 'uploads')
            os.makedirs(upload_dir, exist_ok=True)
            import uuid
            name = uuid.uuid4().hex + (ext if ext and ext.startswith('.') else ('.' + ext if ext else '.bin'))
            target = os.path.join(upload_dir, name)
            with open(target, 'wb') as f:
                f.write(data)
            self._send_json(200, {'path': target})
        except Exception as e:
            logging.exception('upload failed')
            self._send_json(500, {'error': '上传失败：%s' % e})

    def _do_save(self):
        try:
            n = int(self.headers.get('Content-Length', 0) or 0)
            body = json.loads(self.rfile.read(n).decode('utf-8'))
        except Exception:
            self._send_json(400, {'error': '无效请求'})
            return
        path = body.get('path') or ''
        content = body.get('content') or ''
        enc = body.get('encoding') or 'utf-8'
        if not path:
            self._send_json(400, {'error': '缺少文件路径'})
            return
        try:
            import shutil
            bak = None
            if os.path.isfile(path) and not os.path.exists(path + '.bak'):
                shutil.copy2(path, path + '.bak')
                bak = path + '.bak'
            with open(path, 'w', encoding=enc, newline='') as f:
                f.write(content)
            self._send_json(200, {'ok': True, 'path': path, 'backup': bak})
        except Exception as e:
            logging.exception('save failed: %s', path)
            self._send_json(500, {'error': '保存失败：%s' % e})

    def _send_raw(self, p):
        if not os.path.isfile(p):
            self._send(404, 'text/plain; charset=utf-8', b'not found')
            return
        mime = mimetypes.guess_type(p)[0] or 'application/octet-stream'
        try:
            with open(p, 'rb') as f:
                body = f.read()
        except OSError:
            self._send(500, 'text/plain; charset=utf-8', b'read error')
            return
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-cache')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.end_headers()
        self.wfile.write(body)


LAN = {'server': None, 'token': None}


def _is_private(ip):
    parts = ip.split('.')
    if len(parts) != 4:
        return False
    try:
        a = int(parts[0]); b = int(parts[1])
    except ValueError:
        return False
    return (a == 10) or (a == 172 and 16 <= b <= 31) or (a == 192 and b == 168)


def get_lan_ip():
    """获取本机局域网 IP：优先 RFC1918 私网地址（避免取到 VPN/代理网段）。"""
    ips = []
    for target in ('223.5.5.5', '114.114.114.114', '1.1.1.1', '8.8.8.8'):
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            try:
                sock.connect((target, 80))
                ip = sock.getsockname()[0]
                if ip and ip not in ips:
                    ips.append(ip)
            finally:
                sock.close()
        except Exception:
            continue
    for ip in ips:
        if _is_private(ip):
            return ip
    if ips:
        return ips[0]
    try:
        return socket.gethostbyname(socket.gethostname())
    except Exception:
        return '127.0.0.1'


def share_status():
    srv = LAN.get('server')
    if srv is None:
        return {'running': False}
    return {'running': True, 'port': srv.server_port, 'token': LAN.get('token'),
            'url': 'http://%s:%d/' % (get_lan_ip(), srv.server_port)}


def start_lan_server():
    """启动局域网共享服务器（带随机 token 鉴权），供手机等设备访问。"""
    if LAN['server'] is not None:
        return share_status()
    token = secrets.token_urlsafe(12)

    class LanHandler(Handler):
        LAN_TOKEN = token

    try:
        srv = ThreadingHTTPServer(('0.0.0.0', 0), LanHandler)
    except OSError as e:
        return {'ok': False, 'error': '无法监听局域网：%s' % e}
    srv.daemon_threads = True
    threading.Thread(target=srv.serve_forever, daemon=True, name='readmd-lan').start()
    LAN['server'] = srv
    LAN['token'] = token
    d = share_status()
    d['ok'] = True
    logging.info('LAN share started: %s', d.get('url'))
    return d


def stop_lan_server():
    srv = LAN.get('server')
    if srv is None:
        return {'ok': True, 'running': False}
    try:
        srv.shutdown()
    except Exception:
        pass
    try:
        srv.server_close()
    except Exception:
        pass
    LAN['server'] = None
    LAN['token'] = None
    logging.info('LAN share stopped')
    return {'ok': True, 'running': False}


def start_server(port=0):
    server = ThreadingHTTPServer(('127.0.0.1', port), Handler)
    server.daemon_threads = True
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    return server


# ---------------------------------------------------------------- JS 桥接 API

class Api(object):
    """暴露给前端 window.pywebview.api 的方法（浏览器模式下不可用）。"""

    def __init__(self):
        self._window = None

    def choose_file(self):
        import webview
        if self._window is None:
            return None
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG,
                file_types=('Markdown 文件 (*.md;*.markdown;*.mdown;*.mkd;*.txt)',))
            return files[0] if files else None
        except Exception as e:
            logging.exception('choose_file failed')
            return None

    def choose_folder(self):
        import webview
        if self._window is None:
            return None
        try:
            dirs = self._window.create_file_dialog(webview.FOLDER_DIALOG)
            return dirs[0] if dirs else None
        except Exception:
            return None

    def choose_any_file(self):
        """任意格式文件（用于“万物转 MD”）。"""
        import webview
        if self._window is None:
            return None
        try:
            files = self._window.create_file_dialog(
                webview.OPEN_DIALOG,
                file_types=(
                    '所有文件 (*.*)',
                    '文档 (*.md;*.markdown;*.docx;*.doc;*.pptx;*.xlsx;*.pdf;*.html;*.htm;*.txt;*.csv;*.json)',
                    '图片 (*.png;*.jpg;*.jpeg;*.bmp;*.webp;*.tif;*.tiff)',
                ))
            return files[0] if files else None
        except Exception:
            return None

    def start_modules(self):
        """渲染完成后由前端触发：后台加载转换 / OCR / 网页模块。"""
        RM.load_all()
        return True

    def get_modules_status(self):
        st, err = RM.status()
        return {'modules': st, 'errors': err}

    def save_file(self, path, content, encoding):
        """编辑保存：写回文件，首次保存自动生成 .bak 备份。"""
        try:
            import shutil
            bak = None
            if os.path.isfile(path) and not os.path.exists(path + '.bak'):
                shutil.copy2(path, path + '.bak')
                bak = path + '.bak'
            with open(path, 'w', encoding=encoding or 'utf-8', newline='') as f:
                f.write(content)
            return {'ok': True, 'backup': bak}
        except Exception as e:
            logging.exception('save_file failed')
            return {'ok': False, 'error': str(e)}

    def save_as(self, content, suggested):
        """把转换 / 网页 / OCR 结果另存为 .md 文件。"""
        import webview
        if self._window is None:
            return None
        try:
            target = self._window.create_file_dialog(
                webview.SAVE_DIALOG, save_filename=suggested,
                file_types=('Markdown (*.md)',))
            if not target:
                return None
            with open(target, 'w', encoding='utf-8', newline='') as f:
                f.write(content)
            return target
        except Exception as e:
            logging.exception('save_as failed')
            return None

    def open_external(self, url):
        try:
            webbrowser.open(url)
            return True
        except Exception:
            return False

    def open_path(self, path):
        """用系统默认程序打开文件（如图片、PDF 或外部文档）。"""
        try:
            os.startfile(path)
            return True
        except Exception:
            return False

    def get_settings(self):
        return load_json(SETTINGS_FILE, {})

    def save_settings(self, settings):
        cur = load_json(SETTINGS_FILE, {})
        cur.update(settings or {})
        save_json(SETTINGS_FILE, cur)
        return True

    def get_recent(self):
        return load_json(RECENT_FILE, [])

    def add_recent(self, path):
        rec = load_json(RECENT_FILE, [])
        try:
            rec = [x for x in rec if os.path.normcase(x) != os.path.normcase(path)]
        except Exception:
            rec = [x for x in rec if x != path]
        rec.insert(0, path)
        save_json(RECENT_FILE, rec[:20])
        return True

    def clear_recent(self):
        save_json(RECENT_FILE, [])
        return True

    def save_fixed(self, path, content):
        """把修正后的文本另存为新文件。"""
        try:
            base, ext = os.path.splitext(path)
            out = base + '.readmd' + (ext or '.md')
            with open(out, 'w', encoding='utf-8', newline='\n') as f:
                f.write(content)
            return out
        except Exception as e:
            logging.exception('save_fixed failed')
            return None

    def install_association(self):
        """注册 .md 文件关联（当前用户，无需管理员）。"""
        return install_association()

    def get_app_info(self):
        return {'version': VERSION, 'python': sys.version.split()[0]}


# ---------------------------------------------------------------- 文件关联

def _quote(s):
    return '"%s"' % s


def install_association():
    """把 .md 等扩展名关联到 ReadMD（HKCU，无需管理员权限）。

    打包版（PyInstaller exe）直接关联 exe；源码版关联 pythonw + readmd.py。
    """
    try:
        import shutil
        frozen = getattr(sys, 'frozen', False)
        if frozen:
            pyw = sys.executable
            cmd = '%s "%%1"' % _quote(pyw)
            icon = '%s,0' % _quote(pyw)  # exe 自带图标
        else:
            pyw = None
            for cand in (os.path.join(APP_DIR, '.venv', 'Scripts', 'pythonw.exe'),):
                if os.path.isfile(cand):
                    pyw = cand
            if pyw is None:
                py = sys.executable
                base = os.path.basename(py).lower()
                if base == 'python.exe':
                    cand = os.path.splitext(py)[0] + 'w.exe'
                    pyw = cand if os.path.isfile(cand) else None
                if pyw is None:
                    pyw = py  # 退化为 python（可能闪一个控制台）
            script = os.path.join(APP_DIR, 'readmd.py')
            cmd = '%s %s "%%1"' % (_quote(pyw), _quote(script))
            icon = '%s,0' % _quote(os.path.join(APP_DIR, 'assets', 'readmd.ico'))
        for ext in ('.md', '.markdown', '.mdown', '.mkd'):
            subprocess.run(['reg', 'add', r'HKCU\Software\Classes\%s' % ext, '/ve',
                            '/d', 'ReadMD.markdown', '/f'],
                           capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown', '/ve',
                        '/d', 'ReadMD Markdown 阅读器', '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown\DefaultIcon',
                        '/ve', '/d', icon, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\ReadMD.markdown\shell\open\command',
                        '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        subprocess.run(['reg', 'add', r'HKCU\Software\Classes\Applications\readmd.py\shell\open\command',
                        '/ve', '/t', 'REG_EXPAND_SZ', '/d', cmd, '/f'], capture_output=True)
        try:
            subprocess.run(['ie4uinit.exe', '-show'], capture_output=True)
        except Exception:
            pass
        return True
    except Exception as e:
        logging.exception('install_association failed')
        return str(e)


# ---------------------------------------------------------------- 自测

def run_selftest():
    ok = True
    try:
        import urllib.request
        import readmd_fix_test
        readmd_fix_test.run_tests(quiet=True)
    except Exception as e:
        safe_print('fixer tests import failed:', e)
        ok = False
    try:
        server = start_server(0)
        port = server.server_port
        with urllib.request.urlopen('http://127.0.0.1:%d/' % port, timeout=5) as r:
            body = r.read().decode('utf-8', 'replace')
            assert r.status == 200 and 'ReadMD' in body
        if getattr(sys, 'frozen', False):
            with urllib.request.urlopen(
                    'http://127.0.0.1:%d/api/modules' % port, timeout=10) as r:
                d = json.loads(r.read().decode('utf-8'))
                assert 'modules' in d and 'ai' in d['modules']
        else:
            self_file = os.path.abspath(__file__)
            with urllib.request.urlopen(
                    'http://127.0.0.1:%d/api/file?p=%s' % (port, quote(self_file)),
                    timeout=5) as r:
                d = json.loads(r.read().decode('utf-8'))
                assert d['name'] == 'readmd.py'
        safe_print('http server OK (port %d)' % port)
    except Exception as e:
        safe_print('http selftest failed:', e)
        ok = False
    try:
        t = save_prompt({'name': '_selftest', 'system': 'x', 'action': 'ask'})
        assert load_prompts()['templates']
        assert delete_prompt(t['id'])
        s = save_session({'title': '_selftest', 'provider': 'DeepSeek', 'model': 'deepseek-chat',
                         'doc': 't', 'messages': [{'role': 'user', 'content': 'hi'}]})
        assert s['id'] and load_history()[0]['id'] == s['id']
        assert delete_session(s['id'])
        safe_print('prompts/history OK')
    except Exception as e:
        safe_print('prompts/history selftest failed:', e)
        ok = False
    try:
        import tempfile, base64 as _b64
        with tempfile.TemporaryDirectory() as td:
            png = _b64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==')
            img_dir = os.path.join(td, 'images')
            os.makedirs(img_dir, exist_ok=True)
            target = os.path.join(img_dir, 't.png')
            with open(target, 'wb') as f:
                f.write(png)
            assert os.path.isfile(target)
        safe_print('image save OK')
    except Exception as e:
        safe_print('image save selftest failed:', e)
        ok = False
    safe_print('selftest %s' % ('PASSED' if ok else 'FAILED'))
    return 0 if ok else 1


# ---------------------------------------------------------------- 启动

def main():
    parser = argparse.ArgumentParser(description='ReadMD - 轻量级 Markdown 阅读器')
    parser.add_argument('file', nargs='?', help='要打开的 .md 文件')
    parser.add_argument('--browser', action='store_true', help='用默认浏览器打开（兜底模式）')
    parser.add_argument('--port', type=int, default=0, help='本地服务端口（默认随机）')
    parser.add_argument('--selftest', action='store_true', help='运行自测')
    parser.add_argument('--mods', action='store_true', help='加载全部扩展模块并报告状态')
    parser.add_argument('--share', action='store_true', help='启动后自动开启局域网共享（手机扫码访问）')
    parser.add_argument('--assoc', action='store_true', help='注册 .md 默认打开方式后退出')
    args = parser.parse_args()

    if args.assoc:
        r = install_association()
        safe_print('association: %s' % ('OK' if r is True else r))
        return 0 if r is True else 1

    if args.selftest:
        sys.exit(run_selftest())

    if args.mods:
        ok = True
        for m in RM.MODULES:
            good = RM.load_forced(m)
            st, err = RM.status()
            safe_print('%s: %s%s' % (m, st.get(m), (' - ' + err.get(m, '')) if err.get(m) else ''))
            ok = ok and good
        return 0 if ok else 1

    setup_logging()
    server = start_server(args.port)
    if args.share:
        d = start_lan_server()
        if d.get('ok'):
            safe_print('局域网共享已开启：%s' % d.get('url'))
        else:
            safe_print('局域网共享失败：%s' % d.get('error'))
    initial = None
    if args.file:
        p = os.path.abspath(args.file)
        if os.path.isfile(p):
            initial = p
        else:
            safe_print('文件不存在: %s' % args.file)

    url = 'http://127.0.0.1:%d/' % server.server_port
    if initial:
        url += '?file=' + quote(initial)

    if args.browser:
        webbrowser.open(url)
        safe_print('ReadMD 服务运行于 %s（Ctrl+C 退出）' % url)
        try:
            while True:
                threading.Event().wait(3600)
        except KeyboardInterrupt:
            pass
        return 0

    try:
        import webview
    except ImportError:
        safe_print('未安装 pywebview。请先运行 install.bat，或用 --browser 模式。')
        safe_print('快速兜底：python readmd.py --browser "%s"' % (initial or ''))
        return 1

    api = Api()
    try:
        window = webview.create_window(
            'ReadMD', url, js_api=api,
            width=1160, height=820, min_size=(720, 480),
            text_select=True, zoomable=True, background_color='#f7f7f5')
    except Exception as e:
        safe_print('创建窗口失败：%s' % e)
        return 1
    api._window = window
    try:
        webview.start()
    except Exception as e:
        logging.exception('webview start failed')
        safe_print('启动失败：%s' % e)
        try:
            import ctypes
            ctypes.windll.user32.MessageBoxW(0, 'ReadMD 启动失败：%s' % e, 'ReadMD', 0x10)
        except Exception:
            pass
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())