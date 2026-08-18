# -*- coding: utf-8 -*-
"""Thread-safe, on-demand registry for ReadMD's optional feature modules.

Importing this package must remain cheap. A feature is imported only after its
own endpoint (or the explicit ``/api/modules/load`` action) asks for it.
``load_all`` remains for command-line self tests and old integrations only.
"""

import importlib
import logging
import threading


MODULES = ('convert', 'ocr', 'web', 'ai')

_status = {name: 'idle' for name in MODULES}
_error = {}
_lock = threading.RLock()
_threads = {}


def _known(name):
    return name in MODULES


def status():
    """Return snapshots only; this function deliberately never starts imports."""
    with _lock:
        return dict(_status), dict(_error)


def is_ready(name):
    with _lock:
        return _status.get(name) == 'ready'


def set_disabled(names, reason=''):
    """Mark unavailable platform features without removing them from the whitelist."""
    with _lock:
        for name in names:
            if not _known(name):
                continue
            _status[name] = 'disabled'
            if reason:
                _error[name] = reason


def load(name):
    """Start loading one whitelisted module and return its current state.

    The import runs on a daemon thread. Concurrent callers share that thread;
    a module which previously failed is intentionally retried on the next call.
    """
    if not _known(name):
        raise ValueError('unknown ReadMD module: %s' % name)
    with _lock:
        current = _status[name]
        if current in ('ready', 'disabled', 'loading'):
            return current
        # ``error`` is retryable. Clear its old diagnostic before retrying.
        _error.pop(name, None)
        _status[name] = 'loading'
        thread = threading.Thread(target=_run_one, args=(name,), daemon=True,
                                  name='readmd-module-%s' % name)
        _threads[name] = thread
        thread.start()
        return 'loading'


def _run_one(name):
    try:
        mod = importlib.import_module('src.readmd_modules.' + name)
        mod.load()
    except Exception as exc:  # noqa: BLE001
        logging.exception('module %s load failed', name)
        with _lock:
            _status[name] = 'error'
            _error[name] = str(exc)
    else:
        with _lock:
            _status[name] = 'ready'
            _error.pop(name, None)
        logging.info('module %s ready', name)


def load_all():
    """Compatibility/self-test helper; ordinary HTTP requests must not call it."""
    return {name: load(name) for name in MODULES}


def get(name):
    """Get an already-ready module, otherwise raise :class:`ModuleNotReady`."""
    if not _known(name):
        raise ModuleNotReady(name, '未知模块')
    with _lock:
        current, reason = _status[name], _error.get(name)
    if current != 'ready':
        raise ModuleNotReady(name, reason or current)
    return importlib.import_module('src.readmd_modules.' + name)


class ModuleNotReady(Exception):
    def __init__(self, name, reason=''):
        super().__init__('module %s not ready (%s)' % (name, reason))
        self.name = name
        self.reason = reason


def load_forced(name):
    """Force-load a module synchronously (for selftest only)."""
    if not _known(name):
        raise ValueError('unknown ReadMD module: %s' % name)
    mod = importlib.import_module('src.readmd_modules.' + name)
    mod.load()
    with _lock:
        _status[name] = 'ready'
    return mod
