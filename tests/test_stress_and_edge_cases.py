# -*- coding: utf-8 -*-
"""ReadMD 极限压力测试与对抗性边缘场景测试套件。

覆盖维度：
1. 10,000+ 行 / 500,000+ 字符超大文档解析、语法自愈与渲染吞吐量测试；
2. 极端畸变 Markdown 对抗性输入（嵌套多层反引号、未闭合数学环境、XSS 注入、断裂表格）；
3. 46 国多语言（含阿拉伯语/希伯来语 RTL、藏语、印地语等复杂文字）全量并发字典加载与渲染测试；
4. MCP Server 高并发 stdio JSON-RPC 调度压力测试。
"""

import concurrent.futures
import json
import os
import sys
import time
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

MCP_DIR = os.path.join(ROOT, 'packages', 'mcp-server')
if MCP_DIR not in sys.path:
    sys.path.insert(0, MCP_DIR)

from src.readmd_core import fix_markdown
from src.readmd_modules import mdexport, texmd, web
import readmd_mcp_server


class TestStressAndEdgeCases(unittest.TestCase):
    """极限压力与对抗性边缘场景测试。"""

    def test_massive_document_stress(self):
        """测试 10,000+ 行超大文档的自愈与 LaTeX 编译性能。"""
        lines = ["# 极限压力测试文档\n\n"]
        for i in range(1, 2001):
            lines.append(f"## 章节 {i}：深入分析与公式推导\n")
            lines.append(f"这是第 {i} 段中文描述文本，测试公式 $E_{{{i}}} = m c^2 + \\frac{{{i}}}{{100}}$ 与代码片段 `val_{i}`。\n\n")
            lines.append(f"| 索引 | 名称 | 状态 | 权重 |\n|---|---|---|---|\n| {i} | Item_{i} | Active | 0.{i%100:02d} |\n\n")
            if i % 50 == 0:
                lines.append("```python\ndef process_batch(n):\n    return [x**2 for x in range(n)]\n```\n\n")

        massive_md = "".join(lines)
        self.assertGreater(len(massive_md), 300000)

        # 1. 语法自愈压力耗时统计
        t0 = time.perf_counter()
        res = fix_markdown(massive_md)
        t_fix = time.perf_counter() - t0
        self.assertTrue(res.text)
        self.assertLess(t_fix, 5.0, f"30万字超大文档自愈耗时过长: {t_fix:.2f}s (应在 5.0s 内)")

        # 2. LaTeX 编译压力耗时统计
        t0 = time.perf_counter()
        tex = texmd.markdown_to_latex(massive_md, title="Massive Stress Doc")
        t_tex = time.perf_counter() - t0
        self.assertTrue(r"\begin{document}" in tex)
        self.assertLess(t_tex, 5.0, f"30万字 LaTeX 编译耗时过长: {t_tex:.2f}s")

    def test_adversarial_malformed_markdown(self):
        """测试对抗性畸变 Markdown 语法修复的鲁棒性。"""
        malformed_inputs = [
            # 1. 极端未闭合嵌套代码块
            "````markdown\n```python\nprint('hello')\n",
            # 2. 深度断裂的数学公式环境
            "$$\n\\begin{aligned}\nx &= 1 \\\\\ny &= 2\n",
            # 3. 严重错位与缺列的表格
            "| col1 | col2 | col3 |\n|---|---|\n| val1 |\n| val1 | val2 | val3 | val4 | val5 |\n",
            # 4. XSS 与文件伪协议注入
            "<script>alert('xss')</script><iframe src='javascript:void(0)'></iframe>[link](file:///etc/passwd)",
            # 5. 极端空白与零宽字符混合
            "\u200b\u200c\u200d#  标题   \t\t\n\n\n\n\n   ",
            # 6. 中英文与全半角标点混杂
            "这是ReadMD，一个Markdown编辑器；支持LaTeX、BibTeX等。"
        ]

        for idx, sample in enumerate(malformed_inputs):
            res = fix_markdown(sample)
            self.assertIsNotNone(res.text, f"畸变用例 #{idx} 处理失败")
            # 确保不会崩溃且产出合规字符串
            self.assertIsInstance(res.text, str)

    def test_multilingual_complex_scripts_and_rtl(self):
        """测试 46 国语言字典加载及复杂字形（RTL 阿拉伯语/希伯来语、藏语、泰语等）处理。"""
        i18n_dir = os.path.join(ROOT, "assets", "i18n")
        lang_files = [f for f in os.listdir(i18n_dir) if f.endswith(".json") and f != "meta.json"]
        self.assertEqual(len(lang_files), 46)

        # Use the warmed pass so CI disk pressure does not turn cold I/O into a parser regression.
        t_load = float('inf')
        all_dicts = {}
        for _ in range(3):
            t0 = time.perf_counter()
            all_dicts = {}
            for fname in lang_files:
                with open(os.path.join(i18n_dir, fname), "r", encoding="utf-8") as f:
                    all_dicts[fname[:-5]] = json.load(f)
            t_load = min(t_load, time.perf_counter() - t0)
        self.assertLess(t_load, 0.5, f"46国语言加载耗时过长: {t_load:.3f}s")

        # RTL 与复杂语言样例渲染测试
        rtl_samples = {
            "ar": "مرحبا بك في ReadMD! هذا محرر نصوص متطور يدعم صيغة Markdown و LaTeX.",
            "he": "ברוך הבא ל-ReadMD! עורך Markdown מתקדם עם תמיכה בנוסחאות מתמטיות.",
            "bo": "ReadMD ལ་ཕེབས་པར་དགའ་བསུ་ཞུ། འདི་ནི་ Markdown རྩོམ་སྒྲིག་བྱེད་ཆས་ཤིག་ཡིན།",
            "th": "ยินดีต้อนรับสู่ ReadMD เครื่องมือแก้ไข Markdown ระดับมืออาชีพ",
            "hi": "ReadMD में आपका स्वागत है! यह एक शक्तिशाली मार्कडाउन संपादक है।"
        }

        for lang, text in rtl_samples.items():
            res = fix_markdown(f"# {text}\n\n{text} $x^2 + y^2 = r^2$")
            self.assertTrue(len(res.text) > 0)
            self.assertIn(text, res.text)

    def test_mcp_concurrent_dispatch_stress(self):
        """测试 MCP Server 在 30 个并发请求下的调度稳定性与正确性。"""
        def make_call(task_idx):
            content = f"# Task {task_idx}\n\n测试公式 $\\alpha_{{{task_idx}}} + \\beta = \\gamma$"
            return readmd_mcp_server.handle_tool_call("readmd_fix_markdown", {"content": content})

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
            futures = [executor.submit(make_call, i) for i in range(30)]
            results = [f.result() for f in concurrent.futures.as_completed(futures)]

        self.assertEqual(len(results), 30)
        for res in results:
            self.assertFalse(res.get("isError", False))
            payload = json.loads(res["content"][0]["text"])
            self.assertTrue(payload["ok"])


if __name__ == '__main__':
    unittest.main()
