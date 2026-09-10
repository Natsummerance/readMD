# -*- coding: utf-8 -*-
"""前端 i18n 调用点门禁测试。

存在的意义：assets/js/core/i18n.js 查不到 key 时返回 key 本身，所以任何缺失都会
把 `plugin.installFailed` 这样的点号 key 打到 46 种语言的界面上。tests/test_i18n_coverage_test.py
只覆盖 index.html 的 data-i18n*，看不到 JS 调用点 —— 上一次泄漏正是从这条缝里出去的。
"""
import json
import os
import subprocess
import sys
import unittest

BASE_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TOOL = os.path.join(BASE_DIR, 'tools', 'check_js_i18n_keys.py')
I18N_DIR = os.path.join(BASE_DIR, 'assets', 'i18n')


def run_tool(*flags):
    env = dict(os.environ, PYTHONIOENCODING='utf-8')
    proc = subprocess.run(
        [sys.executable, TOOL, *flags],
        cwd=BASE_DIR, capture_output=True, text=True, encoding='utf-8', env=env)
    return proc


class TestJsI18nGate(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.proc = run_tool('--json')
        try:
            cls.report = json.loads(cls.proc.stdout)
        except Exception:
            cls.report = None

    def test_tool_runs_and_reports(self):
        self.assertIsNotNone(self.report,
                             f'门禁脚本输出非 JSON:\n{self.proc.stdout}\n{self.proc.stderr}')

    def test_no_missing_literal_keys(self):
        missing = self.report.get('missing') or {}
        self.assertEqual(missing, {},
                         'JS 里存在 en.json 没有的 i18n key，界面会直接显示点号 key:\n'
                         + json.dumps(missing, ensure_ascii=False, indent=2))

    def test_no_empty_concat_prefixes(self):
        empty = self.report.get('empty_prefixes') or {}
        self.assertEqual(empty, {},
                         '拼接式 i18n key 的前缀在 en.json 中一个词条都没有:\n'
                         + json.dumps(empty, ensure_ascii=False, indent=2))

    def test_scan_coverage_is_meaningful(self):
        # 扫描范围一旦漂空，前几条断言会「全绿但什么都没检查」
        self.assertGreaterEqual(self.report.get('scanned_files', 0), 20,
                                'JS 门禁扫描到的文件数过少，检查 SCAN_DIRS/SCAN_FILES 是否漂移')
        self.assertGreaterEqual(self.report.get('literal_keys', 0), 400,
                                'JS 门禁抽取到的字面量 key 过少，检查 LITERAL_RE 是否失效')

    def test_concat_prefixes_still_present_in_en(self):
        """拼接式 key 只能校验前缀；这里保证评测口径本身没有静默退化。"""
        prefixes = self.report.get('concat_call_sites') or {}
        with open(os.path.join(I18N_DIR, 'en.json'), 'r', encoding='utf-8') as handle:
            en_keys = set(json.load(handle).keys())
        for prefix in prefixes:
            self.assertTrue(any(k.startswith(prefix) for k in en_keys),
                            f'前缀 {prefix} 在 en.json 中无任何词条')

    def test_exit_code_zero(self):
        self.assertEqual(self.proc.returncode, 0,
                         f'门禁退出码 {self.proc.returncode}:\n{self.proc.stdout}')


if __name__ == '__main__':
    unittest.main()
