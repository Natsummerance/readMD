# -*- coding: utf-8 -*-
"""ReadMD 核心服务端与窗口状态管理 (src.readmd_core.server / window_state / dialogs) 单元测试。"""

import json
import os
import sys
import tempfile
import unittest
import urllib.request
from unittest.mock import MagicMock, patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.server import (
    start_server,
    is_port_in_use,
    find_available_port,
    ReadMDHTTPHandler,
)
from src.readmd_core.window_state import WindowStateManager
from src.readmd_core.dialogs import normalize_dialog_path, format_save_filename


class TestReadmdCoreServerAndWindowState(unittest.TestCase):
    """测试 HTTP 服务启动、静态文件服务、CORS 响应、窗口状态持久化与对话框规整。"""

    def test_window_state_manager_geometry(self):
        """测试窗口几何状态存取与最小尺寸限制。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_file = os.path.join(tmpdir, 'settings.json')
            rec_file = os.path.join(tmpdir, 'recents.json')
            mgr = WindowStateManager(settings_file=set_file, recent_file=rec_file)

            # 保存窗口状态
            ok = mgr.save_geometry(width=1200, height=800, x=100, y=50, maximized=False)
            self.assertTrue(ok)

            geo = mgr.load_geometry()
            self.assertEqual(geo['width'], 1200)
            self.assertEqual(geo['height'], 800)
            self.assertEqual(geo['x'], 100)
            self.assertEqual(geo['y'], 50)
            self.assertFalse(geo['maximized'])

            # 测试非法超小尺寸被自动钳位到 MIN_WIDTH / MIN_HEIGHT
            mgr.save_geometry(width=100, height=100)
            geo_clamped = mgr.load_geometry()
            self.assertEqual(geo_clamped['width'], 640)
            self.assertEqual(geo_clamped['height'], 480)

    def test_window_state_recent_files(self):
        """测试最近文件列表维护、去重与顺序置顶。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            set_file = os.path.join(tmpdir, 'settings.json')
            rec_file = os.path.join(tmpdir, 'recents.json')
            mgr = WindowStateManager(settings_file=set_file, recent_file=rec_file)

            f1 = os.path.join(tmpdir, 'doc1.md')
            f2 = os.path.join(tmpdir, 'doc2.md')

            mgr.add_recent_file(f1)
            mgr.add_recent_file(f2)
            recents = mgr.load_recent_files()
            self.assertEqual(len(recents), 2)
            self.assertEqual(recents[0], os.path.abspath(f2))

            # 再次打开 f1，应置顶
            mgr.add_recent_file(f1)
            recents_updated = mgr.load_recent_files()
            self.assertEqual(recents_updated[0], os.path.abspath(f1))
            self.assertEqual(len(recents_updated), 2)

            # 清空
            mgr.clear_recent_files()
            self.assertEqual(mgr.load_recent_files(), [])

    def test_dialogs_format_save_filename(self):
        """测试保存文件名格式化。"""
        self.assertEqual(format_save_filename('my_doc', '.md'), 'my_doc.md')
        self.assertEqual(format_save_filename('my_doc.md', '.md'), 'my_doc.md')
        self.assertEqual(format_save_filename('', '.pdf'), 'untitled.pdf')

    def test_server_startup_and_get_request(self):
        """测试 HTTP 服务器动态分配端口并正确响应 GET 请求。"""
        server, port = start_server(port=0, app_dir=ROOT)
        self.assertTrue(port > 0)

        try:
            # 访问首页
            url = f'http://127.0.0.1:{port}/'
            req = urllib.request.Request(url)
            with urllib.request.urlopen(req, timeout=5) as resp:
                self.assertEqual(resp.status, 200)
                body = resp.read().decode('utf-8', errors='replace')
                self.assertIn('ReadMD', body)
        finally:
            server.shutdown()
            server.server_close()

    def test_server_path_traversal_protection(self):
        """测试 HTTP 服务器路径穿越防护（返回 403）。"""
        server, port = start_server(port=0, app_dir=ROOT)
        try:
            url = f'http://127.0.0.1:{port}/assets/../../../../etc/passwd'
            req = urllib.request.Request(url)
            try:
                urllib.request.urlopen(req, timeout=5)
                self.fail("应当拦截路径穿越并返回 403/404")
            except urllib.error.HTTPError as e:
                self.assertIn(e.code, (403, 404))
        finally:
            server.shutdown()
            server.server_close()

    def test_server_options_cors_preflight(self):
        """测试 OPTIONS 请求返回 CORS 头。"""
        server, port = start_server(port=0, app_dir=ROOT)
        try:
            url = f'http://127.0.0.1:{port}/api/test'
            req = urllib.request.Request(url, method='OPTIONS')
            with urllib.request.urlopen(req, timeout=5) as resp:
                self.assertEqual(resp.status, 204)
                self.assertEqual(resp.headers.get('Access-Control-Allow-Origin'), '*')
        finally:
            server.shutdown()
            server.server_close()


if __name__ == '__main__':
    unittest.main()
