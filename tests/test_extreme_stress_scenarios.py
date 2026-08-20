# -*- coding: utf-8 -*-
"""ReadMD v2.3.4 极限多场景全维度压力测试套件 (Extreme Multi-Scenario Stress Suite)。

测试场景矩阵：
1. [场景 1] 1,000 复合章节技术巨著处理吞吐与内存稳定性；
2. [场景 2] 10,000 行大规模 CSV 数据表瞬时动态转 Markdown 表格；
3. [场景 3] 30 层深层嵌套导入与交叉循环引用攻击防御与优雅截断；
4. [场景 4] 高频并发多语言 Code Chunk 沙箱调度与结果捕获；
5. [场景 5] 400 个复杂 LaTeX 公式与 TikZ / WaveDrom / PlantUML 图表密集交织解析；
6. [场景 6] 46 国多语言 i18n 字典完整性与 0 缺失全覆盖核验。
"""

import json
import os
import sys
import tempfile
import time
import unittest
from concurrent.futures import ThreadPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_core.readmd_fix import fix_markdown
from src.readmd_core.source_map import annotate_markdown_source_lines, inject_source_line_attributes_to_html
from src.readmd_core.toc_engine import extract_headings, generate_toc_markdown, process_toc_markers
from src.readmd_modules.code_chunk_runner import execute_code_chunk, execute_python_chunk
from src.readmd_modules.diagrams import identify_diagram_blocks
from src.readmd_modules.import_processor import (
    csv_to_markdown_table,
    process_markdown_imports,
)
from src.readmd_modules.mdexport.epub_render import export_epub


class TestExtremeStressScenarios(unittest.TestCase):
    """极限多场景压力测试用例。"""

    def test_scenario_1_million_characters_technical_monolith(self):
        """[场景 1] 1,000 章节技术巨著处理吞吐。"""
        chapter_template = """
# 章节 {idx}: 系统模块化与数学定理
在第 {idx} 章节中，我们推导微积分基本定理 $\\int_{{a}}^{{b}} f(x)dx = F(b) - F(a)$。
表格数据如下：
| 指标 A | 指标 B | 指标 C |
| --- | --- | --- |
| 值 1 | 值 2 | 值 3 |

```python
def chapter_{idx}_func(val):
    return val * {idx}
```
"""
        # 构建 1,000 个章节
        sections = [chapter_template.format(idx=i) for i in range(1, 1001)]
        huge_book = "\n".join(sections)
        total_len = len(huge_book)
        self.assertGreater(total_len, 200000)

        t0 = time.perf_counter()
        fixed = fix_markdown(huge_book)
        t_fix = (time.perf_counter() - t0) * 1000

        t1 = time.perf_counter()
        headings = extract_headings(fixed.text)
        toc = generate_toc_markdown(headings, depth_from=1, depth_to=3)
        t_toc = (time.perf_counter() - t1) * 1000

        t2 = time.perf_counter()
        source_mapped = annotate_markdown_source_lines(fixed.text)
        t_sm = (time.perf_counter() - t2) * 1000

        print(f"\n[压力场景 1] 1000章节巨著: 大小={total_len/1024:.1f} KB | 语法自愈={t_fix:.2f} ms | 1000标题TOC={t_toc:.2f} ms | SourceMap={t_sm:.2f} ms")
        self.assertEqual(len(headings), 1000)
        self.assertLess(t_toc, 50)  # 1000 个标题生成应在 50ms 内完成

    def test_scenario_2_massive_csv_table_conversion(self):
        """[场景 2] 10,000 行大规模 CSV 数据表瞬时动态转换。"""
        header = "RowID,UUID,MetricName,Score,P99Latency,Status,Timestamp\n"
        rows = [f"{i},uuid-{i},Metric_{i%50},{85+(i%15)},{1.2+(i%10)*0.1},PASS,2026-08-20T21:00:00Z\n" for i in range(10000)]
        massive_csv = header + "".join(rows)

        t0 = time.perf_counter()
        table_md = csv_to_markdown_table(massive_csv)
        t_csv = (time.perf_counter() - t0) * 1000

        print(f"[压力场景 2] 10,000 行 CSV 转 Markdown 表格耗时: {t_csv:.2f} ms")
        self.assertIn("| 9999 |", table_md)
        self.assertLess(t_csv, 500)  # 10,000 行转换应在 500ms 内完成

    def test_scenario_3_deep_nested_and_circular_imports_defense(self):
        """[场景 3] 30 层深层嵌套与环形交叉导入攻击防御。"""
        with tempfile.TemporaryDirectory() as tmpdir:
            # 创建 30 层的嵌套链
            for i in range(30):
                file_path = os.path.join(tmpdir, f"level_{i}.md")
                next_file = f"level_{i+1}.md" if i < 29 else "level_0.md"  # 最后一层指向第 0 层制造大环
                with open(file_path, "w", encoding="utf-8") as f:
                    f.write(f"## Level {i} Content\n\n@import \"{next_file}\"\n")

            main_path = os.path.join(tmpdir, "level_0.md")
            with open(main_path, "r", encoding="utf-8") as f:
                content = f.read()

            t0 = time.perf_counter()
            res = process_markdown_imports(content, base_dir=tmpdir, current_file=main_path)
            t_import = (time.perf_counter() - t0) * 1000

            print(f"[压力场景 3] 30层嵌套与循环引用防御耗时: {t_import:.2f} ms")
            self.assertTrue("最大 @import 嵌套深度限制" in res or "循环引用" in res)
            self.assertLess(t_import, 500)

    def test_scenario_4_burst_code_chunk_execution(self):
        """[场景 4] 连续 20 次多语言代码块并发安全执行。"""
        snippets = [
            (f"print('Py-{i}')", "python") if i % 2 == 0 else (f"echo Shell-{i}", "cmd" if sys.platform == "win32" else "bash")
            for i in range(20)
        ]

        def run_one(pair):
            code, lang = pair
            return execute_code_chunk(code, lang=lang, capture_plot=False, timeout=5)

        t0 = time.perf_counter()
        with ThreadPoolExecutor(max_workers=4) as executor:
            results = list(executor.map(run_one, snippets))
        t_burst = (time.perf_counter() - t0) * 1000

        print(f"[压力场景 4] 20 次多语言代码块执行耗时: {t_burst:.2f} ms")
        success_count = sum(1 for r in results if r["ok"])
        for idx, r in enumerate(results):
            if not r["ok"]:
                print(f"FAILED snippet {idx}: {snippets[idx]} -> {r}")
        self.assertEqual(success_count, 20)

    def test_scenario_5_dense_latex_and_diagrams_matrix(self):
        """[场景 5] 400 个复杂数学公式与专业图表交织扫描。"""
        blocks = []
        for i in range(100):
            blocks.append(f"### 图表与公式组 {i}\n公式：$\\sum_{{k=1}}^{{n}} k^2 = \\frac{{n(n+1)(2n+1)}}{{6}}$\n")
            blocks.append(f"```wavedrom\n{{ 'signal': [{{ 'name': 'sig_{i}', 'wave': '0101' }}] }}\n```\n")
            blocks.append(f"```dot\ndigraph G_{i} {{ A_{i} -> B_{i} }}\n```\n")
            blocks.append(f"```tikz\n\\draw (0,0) -- ({i},1);\n```\n")
            blocks.append(f"```puml\n@startuml\nA_{i} -> B_{i}\n@enduml\n```\n")

        dense_doc = "\n".join(blocks)
        t0 = time.perf_counter()
        diags = identify_diagram_blocks(dense_doc)
        t_diag = (time.perf_counter() - t0) * 1000

        print(f"[压力场景 5] 400 个多格式专业图表识别耗时: {t_diag:.2f} ms")
        self.assertEqual(len(diags), 400)
        self.assertLess(t_diag, 50)

    def test_scenario_6_i18n_46_languages_100_percent_parity(self):
        """[场景 6] 46 国多语言 i18n 词条全覆盖与 0 缺失校验。"""
        i18n_dir = os.path.join(ROOT, 'assets', 'i18n')
        self.assertTrue(os.path.isdir(i18n_dir))

        zh_path = os.path.join(i18n_dir, 'zh-CN.json')
        with open(zh_path, 'r', encoding='utf-8') as f:
            zh_data = json.load(f)

        base_keys = set(zh_data.keys())
        total_keys = len(base_keys)
        self.assertGreaterEqual(total_keys, 900)

        all_files = [f for f in os.listdir(i18n_dir) if f.endswith('.json') and f != 'meta.json']
        self.assertEqual(len(all_files), 46)

        missing_report = {}
        for fname in all_files:
            fpath = os.path.join(i18n_dir, fname)
            with open(fpath, 'r', encoding='utf-8') as f:
                lang_data = json.load(f)
            missing = base_keys - set(lang_data.keys())
            if missing:
                missing_report[fname] = len(missing)

        print(f"[i18n 校验] 46 国语言审计: 基准词条={total_keys} | 0缺失语言数={46 - len(missing_report)}/46")
        self.assertEqual(len(missing_report), 0, f"发现多语言词条缺失: {missing_report}")


if __name__ == '__main__':
    unittest.main()
