# -*- coding: utf-8 -*-
"""ReadMD 扩展模块注册表：转换 / OCR / 网页提取，全部懒加载。

设计目标：打开 .md 文件时先渲染正文（秒开），渲染完成后再由前端
触发 load_all()，在后台线程中逐个导入重依赖模块。前端轮询 status()
获取加载进度，未就绪的功能按钮保持禁用。
"""

import importlib
import logging
import threading

MODULES = ['convert', 'ocr', 'web', 'ai']

_status = {m: 'idle' for m in MODULES}
_error = {}
_lock = threading.Lock()
_loader_thread = None


def status():
    with _lock:
        return dict(_status), dict(_error)


def is_ready(name):
    with _lock:
        return _status.get(name) == 'ready'


def load_all():
    """后台加载全部模块（幂等，重复调用无副作用）。"""
    global _loader_thread
    with _lock:
        if _loader_thread is not None and _loader_thread.is_alive():
            return
        for m in MODULES:
            if _status[m] in ('ready', 'error'):
                continue
            _status[m] = 'loading'
        _loader_thread = threading.Thread(target=_run, daemon=True, name='readmd-modules')
        _loader_thread.start()


def _run():
    for m in MODULES:
        try:
            mod = importlib.import_module('readmd_modules.' + m)
            mod.load()
            with _lock:
                _status[m] = 'ready'
            logging.info('module %s ready', m)
        except Exception as e:  # noqa: BLE001
            logging.exception('module %s load failed', m)
            with _lock:
                _status[m] = 'error'
                _error[m] = str(e)


def get(name):
    """获取已加载模块；未就绪抛 ModuleNotReady。"""
    if not is_ready(name):
        raise ModuleNotReady(name, _error.get(name))
    return importlib.import_module('readmd_modules.' + name)


class ModuleNotReady(Exception):
    def __init__(self, name, reason=None):
        self.name = name
        self.reason = reason
        super().__init__('模块 %s 尚未就绪%s' % (name, ('：' + reason) if reason else ''))


def load_forced(name):
    """同步强制加载单个模块（用于 --mods 自检）。"""
    try:
        mod = importlib.import_module('readmd_modules.' + name)
        mod.load()
        with _lock:
            _status[name] = 'ready'
            _error.pop(name, None)
        return True
    except Exception as e:  # noqa: BLE001
        logging.exception('module %s forced load failed', name)
        with _lock:
            _status[name] = 'error'
            _error[name] = str(e)
        return False