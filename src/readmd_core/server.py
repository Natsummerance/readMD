# -*- coding: utf-8 -*-
"""ReadMD 本地 HTTP 服务模块 (src.readmd_core.server)。

负责：
1. 本地 127.0.0.1 HTTP 服务启动、端口自增探测与关闭；
2. 静态资源（HTML / CSS / JS / Fonts / i18n）安全挂载与缓存头控制；
3. 本地 API 与局域网共享 Token 鉴权路由调度；
4. 优雅的跨平台线程池 HTTP 服务器。
"""

import json
import logging
import mimetypes
import os
import socket
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Callable, Dict, Optional, Tuple
from urllib.parse import parse_qs, urlparse


def is_port_in_use(port: int, host: str = '127.0.0.1') -> bool:
    """探测指定 TCP 端口是否被占用。"""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.2)
        return s.connect_ex((host, port)) == 0


def find_available_port(start_port: int = 26891, max_tries: int = 50, host: str = '127.0.0.1') -> int:
    """从 start_port 起向下探测可用端口；均被占用时返回 0（由操作系统自动分配）。"""
    for p in range(start_port, start_port + max_tries):
        if not is_port_in_use(p, host):
            return p
    return 0


class ReadMDHTTPHandler(BaseHTTPRequestHandler):
    """ReadMD 本地与局域网 HTTP 请求调度器。"""

    protocol_version = 'HTTP/1.1'
    server_version = 'ReadMD-Server/2.3'
    LAN_TOKEN: Optional[str] = None
    APP_DIR: str = ''
    API_ROUTER: Optional[Callable[[str, str, Dict[str, Any], 'ReadMDHTTPHandler'], bool]] = None

    def log_message(self, format: str, *args: Any) -> None:
        """静默常规 HTTP 访问日志以避免刷屏，调试级别记录。"""
        logging.debug("%s - - [%s] %s", self.address_string(), self.log_date_time_string(), format % args)

    def _lan_authorized(self) -> bool:
        """局域网模式鉴权：除静态资源与首页外，所有 API 均需携带 Token。"""
        if not self.LAN_TOKEN:
            return True
        u = urlparse(self.path)
        if u.path in ('/', '/index.html') or u.path.startswith('/assets/') or u.path.startswith('/i18n/'):
            return True
        qs = parse_qs(u.query)
        if qs.get('t', [''])[0] == self.LAN_TOKEN:
            return True
        return self.headers.get('X-ReadMD-Token', '') == self.LAN_TOKEN

    def _send(self, code: int, ctype: str, body: bytes, immutable: bool = False) -> None:
        """发送基础 HTTP 响应头与内容。"""
        self.send_response(code)
        self.send_header('Content-Type', ctype)
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, X-ReadMD-Token')
        if immutable:
            self.send_header('Cache-Control', 'public, max-age=31536000, immutable')
        else:
            self.send_header('Cache-Control', 'no-cache, no-store, must-revalidate')
        self.end_headers()
        self.wfile.write(body)

    def _send_json(self, code: int, data: Any) -> None:
        """格式化发送 JSON 响应。"""
        body = json.dumps(data, ensure_ascii=False).encode('utf-8')
        self._send(code, 'application/json; charset=utf-8', body)

    def _send_file(self, file_path: str, mime: str, immutable: bool = False) -> None:
        """安全读取并返回静态文件内容。"""
        try:
            with open(file_path, 'rb') as f:
                content = f.read()
            self._send(200, mime, content, immutable=immutable)
        except Exception as e:
            logging.debug("send_file failed for %s: %s", file_path, e)
            self._send(404, 'text/plain; charset=utf-8', b'not found')

    def do_OPTIONS(self) -> None:
        """CORS 预检响应。"""
        self.send_response(204)
        self.send_header('Content-Length', '0')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, X-ReadMD-Token')
        self.end_headers()

    def do_GET(self) -> None:
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._dispatch_route('GET')
        except Exception as e:
            logging.exception('HTTP GET route failed: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', f'Internal Server Error: {e}'.encode('utf-8'))
            except Exception:
                pass

    def do_POST(self) -> None:
        if not self._lan_authorized():
            self._send(403, 'text/plain; charset=utf-8', b'forbidden')
            return
        try:
            self._dispatch_route('POST')
        except Exception as e:
            logging.exception('HTTP POST route failed: %s', self.path)
            try:
                self._send(500, 'text/plain; charset=utf-8', f'Internal Server Error: {e}'.encode('utf-8'))
            except Exception:
                pass

    def _dispatch_route(self, method: str) -> None:
        u = urlparse(self.path)
        path = u.path
        qs = parse_qs(u.query)
        app_dir = self.APP_DIR or os.getcwd()

        # 1. 首页路由
        if path in ('/', '/index.html'):
            idx_path = os.path.join(app_dir, 'assets', 'index.html')
            self._send_file(idx_path, 'text/html; charset=utf-8')
            return

        # 2. 静态资源路由 (assets/ & i18n/)
        if path.startswith('/assets/') or path.startswith('/i18n/'):
            if path.startswith('/assets/'):
                rel = path[len('/assets/'):]
            else:
                rel = path.lstrip('/')
            fp = os.path.normpath(os.path.join(app_dir, 'assets', rel))
            base = os.path.normpath(os.path.join(app_dir, 'assets'))
            # 严格防止路径遍历
            if not fp.startswith(base):
                self._send(403, 'text/plain; charset=utf-8', b'forbidden')
                return

            mime = mimetypes.guess_type(fp)[0] or 'application/octet-stream'
            if mime.startswith('text/') or mime in ('application/javascript', 'application/json'):
                mime += '; charset=utf-8'
            is_cached = rel.startswith('vendor/') or rel.startswith('i18n/')
            immutable = bool(is_cached or qs.get('v') or qs.get('version') or qs.get('hash'))
            self._send_file(fp, mime, immutable=immutable)
            return

        # 3. 转发至外部注册的 API Router 调度器
        if self.API_ROUTER:
            handled = self.API_ROUTER(method, path, qs, self)
            if handled:
                return

        # 4. 404 兜底
        self._send(404, 'text/plain; charset=utf-8', b'not found')


class ThreadedReadMDServer(ThreadingHTTPServer):
    """多线程 HTTP 服务端，保证大文件传输与多并发请求不阻塞。"""
    daemon_threads = True
    allow_reuse_address = True


def start_server(
    port: int = 26891,
    app_dir: str = '',
    api_router: Optional[Callable[[str, str, Dict[str, Any], ReadMDHTTPHandler], bool]] = None,
    lan_token: Optional[str] = None
) -> Tuple[ThreadedReadMDServer, int]:
    """启动本地 HTTP 服务器并在后台线程运行。返回 (server_instance, bound_port)。"""
    ReadMDHTTPHandler.APP_DIR = app_dir
    ReadMDHTTPHandler.API_ROUTER = api_router
    ReadMDHTTPHandler.LAN_TOKEN = lan_token

    bind_port = find_available_port(port) if port != 0 else 0
    server = ThreadedReadMDServer(('127.0.0.1', bind_port), ReadMDHTTPHandler)
    actual_port = server.server_port

    th = threading.Thread(target=server.serve_forever, daemon=True, name='readmd-http-server')
    th.start()
    return server, actual_port
