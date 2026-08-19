# -*- coding: utf-8 -*-
"""ReadMD v2.3.0 新特性全量单元测试与回归套件。

涵盖：
1. 软件内更新器排查：临时文件清理机制、镜像源降级、进程退出调度；
2. i18n 45+ 语种字典完整性与系统语言智能侦测；
3. LaTeX PRO 与 BibTeX 参考文献解析；
4. MCP Server 工具集功能验证。
"""

import os
import sys
import unittest
import tempfile
import json
import time
import importlib.util

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

from src.readmd_modules import updater, bibtex, texmd
import readmd
from tools import i18n_sync

mcp_path = os.path.join(ROOT_DIR, 'packages', 'mcp-server', 'readmd_mcp_server.py')
spec = importlib.util.spec_from_file_location("readmd_mcp_server", mcp_path)
mcp_srv = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mcp_srv)



class TestV230Features(unittest.TestCase):

    def test_clean_old_update_artifacts(self):
        """测试历史更新安装包扫描与自动清理。"""
        td = tempfile.gettempdir()
        dummy_installer = os.path.join(td, 'ReadMDSetup-v9.9.9.exe')
        dummy_bat = os.path.join(td, 'readmd_update.bat')
        with open(dummy_installer, 'w') as f:
            f.write('dummy')
        with open(dummy_bat, 'w') as f:
            f.write('dummy')

        # 修改为 15 分钟前
        old_time = time.time() - 900
        os.utime(dummy_installer, (old_time, old_time))
        os.utime(dummy_bat, (old_time, old_time))

        updater.clean_old_update_artifacts()
        self.assertFalse(os.path.exists(dummy_installer))
        self.assertFalse(os.path.exists(dummy_bat))

    def test_system_language_detection(self):
        """测试系统语言侦测函数。"""
        lang = readmd.get_system_language()
        self.assertIsInstance(lang, str)
        self.assertTrue(len(lang) >= 2)

    def test_i18n_locales_validation(self):
        """测试 45+ 语种词条完整性（100% 覆盖率）。"""
        ok = i18n_sync.validate_all_locales()
        self.assertTrue(ok)

    def test_bibtex_parser(self):
        """测试 BibTeX 纯 Python 学术文献解析。"""
        bib_content = """
@article{vaswani2017attention,
  title={Attention is all you need},
  author={Vaswani, Ashish and Shazeer, Noam and Parmar, Niki and Uszkoreit, Jakob},
  journal={Advances in Neural Information Processing Systems},
  volume={30},
  year={2017},
  doi={10.48550/arXiv.1706.03762}
}

@book{knuth1984texbook,
  title={The TeXbook},
  author={Knuth, Donald Ervin},
  year={1984},
  publisher={Addison-Wesley}
}
"""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.bib', delete=False, encoding='utf-8') as f:
            f.write(bib_content)
            temp_bib = f.name

        try:
            entries = bibtex.parse_bibtex_file(temp_bib)
            self.assertIn('vaswani2017attention', entries)
            self.assertIn('knuth1984texbook', entries)

            vaswani = entries['vaswani2017attention']
            self.assertEqual(vaswani['title'], 'Attention is all you need')
            self.assertEqual(vaswani['year'], '2017')
            self.assertIn('Vaswani et al.', vaswani['short_cite'])
            self.assertIn('10.48550/arXiv.1706.03762', vaswani['full_reference'])

            knuth = entries['knuth1984texbook']
            self.assertEqual(knuth['title'], 'The TeXbook')
            self.assertIn('Knuth', knuth['short_cite'])
        finally:
            if os.path.exists(temp_bib):
                os.unlink(temp_bib)

    def test_mcp_server_tools(self):
        """测试 MCP Server 工具调用处理。"""
        res = mcp_srv.handle_tool_call("readmd_fix_markdown", {"content": "# Hello\n|a|b\n|---|---\n|1|2\n$x=1$"})
        self.assertFalse(res.get("isError", False))
        self.assertTrue(len(res.get("content", [])) > 0)

        # LaTeX to MD
        tex_res = mcp_srv.handle_tool_call("readmd_latex_to_md", {"latex_content": "\\section{Intro}\nHello world."})
        self.assertIn("# Intro", tex_res["content"][0]["text"])


if __name__ == '__main__':
    unittest.main()

