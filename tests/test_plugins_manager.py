# -*- coding: utf-8 -*-
"""ReadMD 插件沙箱管理器与 API 端点自动化测试套件。"""

import importlib
import os
import shutil
import sys
import tempfile
import threading
import types
import unittest
from unittest.mock import patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import plugin_manager as pm


def _write_sandbox_install(site_packages, import_name, package, version):
    """伪造一次"pip 真的装完了"的沙箱状态：包目录 + 发行版元数据。"""
    os.makedirs(site_packages, exist_ok=True)
    pkg_dir = os.path.join(site_packages, import_name)
    os.makedirs(pkg_dir, exist_ok=True)
    with open(os.path.join(pkg_dir, '__init__.py'), 'w', encoding='utf-8') as handle:
        handle.write('__version__ = %r\n' % version)
    info_dir = os.path.join(site_packages, '%s-%s.dist-info' % (package, version))
    os.makedirs(info_dir, exist_ok=True)
    with open(os.path.join(info_dir, 'METADATA'), 'w', encoding='utf-8') as handle:
        handle.write('Metadata-Version: 2.1\nName: %s\nVersion: %s\n' % (package, version))
    return pkg_dir, info_dir


class _ImmediateThread:
    """把后台安装线程改成同步执行，让断言不必依赖 sleep 轮询。"""

    def __init__(self, target=None, name='', args=(), kwargs=None, daemon=None):
        self._target = target
        self._args = args
        self._kwargs = kwargs or {}

    def start(self):
        if self._target:
            self._target(*self._args, **self._kwargs)

    def join(self, timeout=None):
        return None


class TestPluginManager(unittest.TestCase):
    """插件管理器单元测试。"""

    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix='readmd_test_plugins_')
        self._orig_dir = pm.PLUGINS_DIR
        self._orig_manifest = pm.PLUGINS_MANIFEST
        self._orig_site = pm.PLUGINS_SITE_PACKAGES
        self._orig_bin = pm.PLUGINS_BIN
        self._orig_tasks = dict(pm._install_tasks)
        self._orig_path_env = os.environ.get('PATH', '')

        pm.PLUGINS_DIR = self.test_dir
        pm.PLUGINS_MANIFEST = os.path.join(self.test_dir, 'plugins.json')
        pm.PLUGINS_SITE_PACKAGES = os.path.join(self.test_dir, 'site-packages')
        pm.PLUGINS_BIN = os.path.join(self.test_dir, 'bin')
        pm._install_tasks.clear()
        for mod in ('pylatexenc', 'easyocr', 'rapidocr_onnxruntime', 'rapid_table', 'whisper', 'jieba', 'pygments', 'pypandoc'):
            sys.modules.pop(mod, None)
        importlib.invalidate_caches()

    def tearDown(self):
        pm.PLUGINS_DIR = self._orig_dir
        pm.PLUGINS_MANIFEST = self._orig_manifest
        pm.PLUGINS_SITE_PACKAGES = self._orig_site
        pm.PLUGINS_BIN = self._orig_bin
        pm._install_tasks.clear()
        pm._install_tasks.update(self._orig_tasks)
        os.environ['PATH'] = self._orig_path_env
        for leaked in (self.test_dir + os.sep, self.test_dir):
            sys.path[:] = [entry for entry in sys.path if not entry.startswith(leaked)]
        for mod in ('pylatexenc', 'easyocr', 'rapidocr_onnxruntime', 'rapid_table', 'whisper', 'jieba', 'pygments', 'pypandoc'):
            sys.modules.pop(mod, None)
        importlib.invalidate_caches()
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def test_load_manifest_defaults(self):
        """测试初始状态下加载默认清单。"""
        manifest = pm.load_manifest()
        self.assertIn('easyocr', manifest)
        self.assertIn('pylatexenc', manifest)
        self.assertIn('rapidocr', manifest)
        self.assertIn('rapid_table', manifest)
        self.assertIn('whisper', manifest)

        rapidocr = manifest['rapidocr']
        self.assertEqual(rapidocr['category'], 'ocr')
        self.assertEqual(rapidocr['weight'], 'light')
        self.assertIn('cached', rapidocr)

    def test_mount_sandbox(self):
        """测试挂载沙箱环境。"""
        pm.mount_sandbox()
        self.assertIn(pm.PLUGINS_SITE_PACKAGES, sys.path)
        self.assertIn(pm.PLUGINS_BIN, os.environ.get('PATH', ''))

    def test_set_plugin_enabled_and_save(self):
        """测试设置启用状态并持久化到 plugins.json。"""
        _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'whisper', 'openai-whisper', '20231117')

        self.assertTrue(pm.is_plugin_installed('whisper'))

        # 启用
        pm.set_plugin_enabled('whisper', True)
        self.assertTrue(pm.is_plugin_enabled('whisper'))

        # 重新加载清单验证持久化
        manifest = pm.load_manifest()
        self.assertTrue(manifest['whisper']['installed'])
        self.assertTrue(manifest['whisper']['enabled'])

        # 禁用
        pm.set_plugin_enabled('whisper', False)
        self.assertFalse(pm.is_plugin_enabled('whisper'))
        manifest = pm.load_manifest()
        self.assertFalse(manifest['whisper']['enabled'])

    def test_uninstall_plugin(self):
        """测试插件卸载并清理目录，同时清空残留错误态。"""
        pm._set_task(
            'easyocr',
            status='error',
            progress=0,
            last_log='stale',
            error_code='pip_network',
            error_detail=['stale'],
        )
        pkg_dir, info_dir = _write_sandbox_install(
            pm.PLUGINS_SITE_PACKAGES, 'easyocr', 'easyocr', '1.7.0'
        )

        self.assertTrue(pm.is_plugin_installed('easyocr'))
        ok = pm.uninstall_plugin('easyocr')
        self.assertTrue(ok)
        self.assertFalse(os.path.exists(pkg_dir))
        self.assertFalse(os.path.exists(info_dir))
        self.assertFalse(pm.is_plugin_installed('easyocr'))
        self.assertNotIn('easyocr', pm._install_tasks)
        manifest = pm.load_manifest()
        self.assertEqual(manifest['easyocr']['install_error_code'], '')
        self.assertEqual(manifest['easyocr']['install_error'], '')

    def test_uninstall_locked_reports_error_without_flagging_uninstalled(self):
        """删不掉的工件必须回传 uninstall_locked，不能谎报"已卸载"。"""
        _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'easyocr', 'easyocr', '1.7.0')

        with patch.object(pm.shutil, 'rmtree', side_effect=OSError('WinError 5 拒绝访问')):
            self.assertFalse(pm.uninstall_plugin('easyocr'))

        task = pm._install_tasks['easyocr']
        self.assertEqual(task['error_code'], 'uninstall_locked')
        self.assertEqual(task['status'], 'error')
        manifest = pm.load_manifest()
        self.assertFalse(manifest['easyocr']['uninstalled'])
        self.assertTrue(manifest['easyocr']['installed'])

    def test_declarative_cache_check(self):
        """测试声明式元数据驱动的模型缓存检测机制。"""
        # 测试 cache_type = installed
        with patch.object(pm, 'is_plugin_installed', side_effect=lambda pid: pid == 'installed_mock'):
            self.assertFalse(pm._check_model_cached('rapid_table'))
        with patch.object(pm, 'is_plugin_installed', side_effect=lambda pid: pid == 'rapid_table'):
            self.assertTrue(pm._check_model_cached('rapid_table'))

        # 测试 cache_type = dir_has_files (如 easyocr, whisper)
        fake_home = os.path.join(self.test_dir, 'fake_home')
        easyocr_dir = os.path.join(fake_home, '.EasyOCR', 'model')
        os.makedirs(easyocr_dir, exist_ok=True)
        with patch('os.path.expanduser', side_effect=lambda p: p.replace('~', fake_home)):
            # 目录为空时返回 False
            self.assertFalse(pm._check_model_cached('easyocr'))
            # 目录下写入权重文件后返回 True
            with open(os.path.join(easyocr_dir, 'craft.pth'), 'w') as f:
                f.write('dummy')
            self.assertTrue(pm._check_model_cached('easyocr'))

    def test_manifest_includes_rapid_table_and_progress(self):
        """测试清单中包含 rapid_table 插件及下载进度字段。"""
        manifest = pm.load_manifest()
        self.assertIn('rapid_table', manifest)
        rt = manifest['rapid_table']
        self.assertEqual(rt['category'], 'document')
        self.assertEqual(rt['weight'], 'light')
        self.assertIn('progress', rt)
        self.assertEqual(rt['progress'], 0)

    def test_frozen_pip_dispatch_never_relaunches_host_executable(self):
        """冻结态必须进程内跑 pip：[sys.executable, '-m', 'pip'] 会把参数喂回主程序。"""
        in_process = []

        def fake_inprocess(args, on_line):
            in_process.append(list(args))
            return 0

        with patch.object(sys, 'frozen', True, create=True), patch.object(
            pm, '_run_pip_inprocess', side_effect=fake_inprocess
        ), patch.object(pm, '_run_pip_subprocess') as subprocess_run:
            return_code = pm._run_pip(['install', 'pylatexenc'], lambda line: None)

        self.assertEqual(return_code, 0)
        subprocess_run.assert_not_called()
        self.assertEqual(in_process, [['install', 'pylatexenc']])

    def test_development_pip_dispatch_uses_subprocess(self):
        """开发态保留子进程，且 argv 只用于解释器自身，不会命中主程序 argparse。"""
        recorded = {}

        class FakeProcess:
            stdout = iter(['Collecting pylatexenc\n', 'Successfully installed pylatexenc-2.9\n'])

            def wait(self):
                return 0

        def fake_popen(argv, **kwargs):
            recorded['argv'] = list(argv)
            return FakeProcess()

        with patch.object(sys, 'frozen', False, create=True), patch.object(
            pm.subprocess, 'Popen', side_effect=fake_popen
        ), patch.object(pm, '_run_pip_inprocess') as in_process:
            lines = []
            return_code = pm._run_pip(['install', 'pylatexenc'], lines.append)

        self.assertEqual(return_code, 0)
        in_process.assert_not_called()
        self.assertEqual(recorded['argv'][0], sys.executable)
        self.assertEqual(recorded['argv'][1:3], ['-m', 'pip'])
        self.assertEqual(lines, ['Collecting pylatexenc\n', 'Successfully installed pylatexenc-2.9\n'])

    def test_run_pip_inprocess_feeds_argv_and_restores_state(self):
        """runpy 不自己造 argv：必须注入 pip 参数，并在返回前恢复被劫持的流与环境。"""
        seen = {}

        def fake_run_module(module_name, run_name=None, alter_sys=False):
            seen['module'] = module_name
            seen['run_name'] = run_name
            seen['alter_sys'] = alter_sys
            seen['argv'] = list(sys.argv)
            sys.stdout.write('Collecting pylatexenc\n')
            raise SystemExit(2)

        stdout_before, stderr_before, argv_before = sys.stdout, sys.stderr, list(sys.argv)
        progress_before = os.environ.get('PIP_PROGRESS_BAR')
        color_before = os.environ.get('NO_COLOR')
        lines = []
        with patch.object(pm.runpy, 'run_module', side_effect=fake_run_module):
            return_code = pm._run_pip_inprocess(['install', 'pylatexenc'], lines.append)

        self.assertEqual(return_code, 2)
        self.assertEqual(seen['module'], 'pip')
        self.assertEqual(seen['run_name'], '__main__')
        self.assertTrue(seen['alter_sys'])
        self.assertEqual(seen['argv'], ['pip', 'install', 'pylatexenc'])
        self.assertEqual(lines, ['Collecting pylatexenc'])
        self.assertIs(sys.stdout, stdout_before)
        self.assertIs(sys.stderr, stderr_before)
        self.assertEqual(sys.argv, argv_before)
        self.assertEqual(os.environ.get('PIP_PROGRESS_BAR'), progress_before)
        self.assertEqual(os.environ.get('NO_COLOR'), color_before)

    def test_pip_failure_classification_is_order_independent(self):
        """断网时 pip 也打印 No matching distribution found，网络特征必须先命中。"""
        offline = [
            'WARNING: Retrying (Retry(total=4, connect=None)) after connection broken by '
            '"NewConnectionError(HTTPSConnectionPool(host=\'pypi.org\', port=443))"',
            'ERROR: No matching distribution found for pylatexenc',
        ]
        self.assertEqual(pm._classify_pip_failure(offline), 'pip_network')
        self.assertEqual(pm._classify_pip_failure(['ERROR: Access is denied']), 'pip_permission')
        self.assertEqual(pm._classify_pip_failure(['WARNING: Read timed out']), 'pip_timeout')
        self.assertEqual(
            pm._classify_pip_failure(['ERROR: No matching distribution found for nope']),
            'pip_no_distribution',
        )
        self.assertEqual(pm._classify_pip_failure(['something else entirely']), 'pip_unknown')

    def test_progress_from_line(self):
        """进度既认 pip 阶段标记也认百分比。"""
        self.assertEqual(pm._progress_from_line('Collecting pylatexenc'), 10)
        self.assertEqual(pm._progress_from_line('Installing collected packages: pylatexenc'), 85)
        self.assertEqual(pm._progress_from_line('Successfully installed pylatexenc-2.9'), 100)
        self.assertEqual(pm._progress_from_line('| 45%|'), 45)
        self.assertIsNone(pm._progress_from_line('nothing useful here'))

    def test_environment_ready_requires_matching_version(self):
        """find_spec 命中不等于装好了：版本不满足请求就必须继续走 pip。"""
        host_spec = types.SimpleNamespace(origin=os.path.join('C:', 'Python311', 'site-packages', 'x.py'))
        with patch.object(pm, '_installed_distribution_version', return_value='1.0'), patch.object(
            pm.importlib.util, 'find_spec', return_value=host_spec
        ):
            satisfied = {'import_name': 'jieba', 'package': 'jieba', 'pip_args': ['jieba>=1.0']}
            self.assertEqual(pm._environment_plugin_ready(satisfied), (True, '1.0'))

            too_old = {'import_name': 'jieba', 'package': 'jieba', 'pip_args': ['jieba>=9.9']}
            self.assertEqual(pm._environment_plugin_ready(too_old), (False, '1.0'))

    def test_environment_ready_rejects_half_installed_sandbox(self):
        """沙箱里缺 dist-info 的残骸不能被当成"已安装"，否则出现装了却没装。"""
        os.makedirs(pm.PLUGINS_SITE_PACKAGES, exist_ok=True)
        pkg_dir = os.path.join(pm.PLUGINS_SITE_PACKAGES, 'pylatexenc')
        os.makedirs(pkg_dir, exist_ok=True)
        origin = os.path.join(pkg_dir, '__init__.py')
        sandbox_spec = types.SimpleNamespace(origin=origin)

        with patch.object(pm.importlib.util, 'find_spec', return_value=sandbox_spec):
            self.assertFalse(pm.is_plugin_installed('pylatexenc'))
            self.assertEqual(
                pm._environment_plugin_ready(pm.PLUGIN_SPECS['pylatexenc']), (False, '')
            )

        _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'pylatexenc', 'pylatexenc', '2.9')
        with patch.object(pm.importlib.util, 'find_spec', return_value=sandbox_spec):
            self.assertTrue(pm.is_plugin_installed('pylatexenc'))
            self.assertEqual(
                pm._environment_plugin_ready(pm.PLUGIN_SPECS['pylatexenc']), (True, '2.9')
            )

    def test_install_runs_pip_and_persists_resolved_version(self):
        """成功路径必须真的调用 pip、落盘版本，并把错误态清空。"""
        pm._set_task(
            'pylatexenc',
            status='error',
            progress=0,
            last_log='stale',
            error_code='pip_network',
            error_detail=['stale'],
        )
        pip_calls = []

        def fake_pip(args, on_line):
            pip_calls.append(list(args))
            on_line('Collecting pylatexenc\n')
            _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'pylatexenc', 'pylatexenc', '2.9')
            on_line('Successfully installed pylatexenc-2.9\n')
            return 0

        with patch.object(pm.threading, 'Thread', _ImmediateThread), patch.object(
            pm, '_environment_plugin_ready', side_effect=[(False, ''), (True, '2.9')]
        ), patch.object(pm, '_run_pip', side_effect=fake_pip):
            self.assertTrue(pm.install_plugin_async('pylatexenc'))

        self.assertEqual(len(pip_calls), 1)
        self.assertEqual(pip_calls[0][0], 'install')
        self.assertIn(pm.PLUGINS_SITE_PACKAGES, pip_calls[0])
        self.assertIn('pylatexenc', pip_calls[0])
        self.assertIn('--upgrade', pip_calls[0])
        self.assertNotIn(sys.executable, pip_calls[0])

        task = pm._install_tasks['pylatexenc']
        self.assertEqual(task['status'], 'success')
        self.assertEqual(task['error_code'], '')
        self.assertEqual(task['progress'], 100)
        manifest = pm.load_manifest()
        self.assertTrue(manifest['pylatexenc']['installed'])
        self.assertEqual(manifest['pylatexenc']['version'], '2.9')
        self.assertEqual(manifest['pylatexenc']['install_error_code'], '')
        self.assertFalse(manifest['pylatexenc']['uninstalled'])

    def test_install_failure_exposes_only_localizable_error_code(self):
        """失败只回传 error_code + 原始 detail，绝不把 pip 散文当用户文案。"""
        def fake_pip(args, on_line):
            on_line('ERROR: Could not find a version that satisfies the requirement nope\n')
            on_line("WARNING: Retrying after connection broken by 'Name or service not known'\n")
            return 1

        with patch.object(pm.threading, 'Thread', _ImmediateThread), patch.object(
            pm, '_environment_plugin_ready', return_value=(False, '')
        ), patch.object(pm, '_run_pip', side_effect=fake_pip):
            self.assertTrue(pm.install_plugin_async('pylatexenc'))

        manifest = pm.load_manifest()
        entry = manifest['pylatexenc']
        self.assertEqual(entry['install_error_code'], 'pip_network')
        self.assertEqual(entry['install_error'], 'pip_network')
        self.assertIn('Name or service not known', entry['install_error_detail'])
        self.assertFalse(entry['installed'])
        self.assertFalse(entry['installing'])

    def test_install_without_pip_reports_pip_unavailable(self):
        """包里没带 pip 时给出可本地化的 pip_unavailable，而不是抛栈。"""
        with patch.object(pm.threading, 'Thread', _ImmediateThread), patch.object(
            pm, '_environment_plugin_ready', return_value=(False, '')
        ), patch.object(pm, '_run_pip', side_effect=ImportError('no module named pip')):
            self.assertTrue(pm.install_plugin_async('pylatexenc'))

        entry = pm.load_manifest()['pylatexenc']
        self.assertEqual(entry['install_error_code'], 'pip_unavailable')
        self.assertFalse(entry['installed'])

    def test_retry_after_failure_clears_stale_error(self):
        """重试立刻覆盖上一轮错误码，前端不再停在旧失败文案。"""
        pm._set_task(
            'pylatexenc',
            status='error',
            progress=0,
            last_log='stale',
            error_code='pip_permission',
            error_detail=['stale'],
        )
        # 环境已满足 spec 的真实形态：工件确实在沙箱里，installed 才会是 True。
        _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'pylatexenc', 'pylatexenc', '2.9')
        with patch.object(pm.threading, 'Thread', _ImmediateThread), patch.object(
            pm, '_environment_plugin_ready', return_value=(True, '2.9')
        ), patch.object(pm, '_run_pip') as pip_run:
            self.assertTrue(pm.install_plugin_async('pylatexenc'))

        pip_run.assert_not_called()
        entry = pm.load_manifest()['pylatexenc']
        self.assertEqual(entry['install_error_code'], '')
        self.assertEqual(pm._install_tasks['pylatexenc']['status'], 'success')
        self.assertTrue(entry['installed'])
        self.assertEqual(entry['version'], '2.9')

    def test_unknown_plugin_never_installs(self):
        with patch.object(pm.threading, 'Thread', _ImmediateThread):
            self.assertFalse(pm.install_plugin_async('definitely-not-a-plugin'))
            self.assertFalse(pm.uninstall_plugin('definitely-not-a-plugin'))

    def test_pip_success_without_package_is_an_error(self):
        with patch.object(pm.threading, 'Thread', _ImmediateThread), patch.object(
            pm, '_environment_plugin_ready', return_value=(False, '')
        ), patch.object(pm, '_run_pip', return_value=0):
            self.assertTrue(pm.install_plugin_async('pylatexenc'))
        self.assertEqual(pm._install_tasks['pylatexenc']['status'], 'error')
        self.assertNotIn('pylatexenc', pm._read_manifest_data())

    def test_corrupt_manifest_entry_does_not_break_plugin_center(self):
        pm._ensure_dirs()
        with open(pm.PLUGINS_MANIFEST, 'w', encoding='utf-8') as handle:
            handle.write('{"pylatexenc": null, "easyocr": false}')
        self.assertIn('pylatexenc', pm.load_manifest())

    def test_uninstall_during_install_is_rejected(self):
        pkg_dir, _ = _write_sandbox_install(pm.PLUGINS_SITE_PACKAGES, 'pylatexenc', 'pylatexenc', '2.9')
        pm._set_task('pylatexenc', status='installing', progress=40, last_log='installing')
        self.assertFalse(pm.uninstall_plugin('pylatexenc'))
        self.assertTrue(os.path.isdir(pkg_dir))
        self.assertEqual(pm._install_tasks['pylatexenc']['status'], 'installing')

    def test_installers_serialize_without_blocking_manifest_reads(self):
        entered, release = threading.Event(), threading.Event()
        second_entered = threading.Event()
        threads = []
        real_thread = threading.Thread

        def tracked_thread(**kwargs):
            thread = real_thread(**kwargs)
            threads.append(thread)
            return thread

        def fake_pip(args, on_line):
            if 'pylatexenc' in args:
                entered.set()
                if not release.wait(5):
                    raise RuntimeError('test installer timed out')
            else:
                second_entered.set()
            return 1

        with patch.object(pm.threading, 'Thread', side_effect=tracked_thread), patch.object(
            pm, '_environment_plugin_ready', return_value=(False, '')
        ), patch.object(pm, '_run_pip', side_effect=fake_pip):
            try:
                pm.install_plugin_async('pylatexenc')
                self.assertTrue(entered.wait(3))
                pm.install_plugin_async('easyocr')
                self.assertTrue(pm.load_manifest()['easyocr']['installing'])
                self.assertFalse(second_entered.is_set())
            finally:
                release.set()
                for thread in threads:
                    thread.join(5)
        self.assertTrue(second_entered.is_set())


if __name__ == '__main__':
    unittest.main()
