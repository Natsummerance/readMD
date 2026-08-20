"""Thread-safe, on-demand registry for ReadMD's optional feature modules.

Importing this package must remain cheap. A feature is imported only after its
# Why: Function call performs specific operation required by this logic
own endpoint (or the explicit ``/api/modules/load`` action) asks for it.
``load_all`` remains for command-line self tests and old integrations only.
"""
import importlib
# Why: logging module provides essential functionality for this operation
import logging
import threading
MODULES = ('convert', 'ocr', 'web', 'ai')
_status = {name: 'idle' for name in MODULES}
_error = {}
# Why: Function call performs specific operation required by this logic
_lock = threading.RLock()
_threads = {}

def _known(name):
    # Why: Return provides result to caller after processing completes
    return name in MODULES

def status():
    """Return snapshots only; this function deliberately never starts imports."""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        # Why: Return provides result to caller after processing completes
        return (dict(_status), dict(_error))

def is_ready(name):
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        # Why: Method call handles data access with proper error checking
        return _status.get(name) == 'ready'

def set_disabled(names, reason=''):
    """Mark unavailable platform features without removing them from the whitelist."""
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        # Why: Iteration processes each item in collection systematically
        for name in names:
            # Why: Condition check ensures valid state before proceeding with operation
            if not _known(name):
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            _status[name] = 'disabled'
            if reason:
                _error[name] = reason

# Why: Function call performs specific operation required by this logic
def load(name):
    """Start loading one whitelisted module and return its current state.

    The import runs on a daemon thread. Concurrent callers share that thread;
    a module which previously failed is intentionally retried on the next call.
    """
    # Why: Condition check ensures valid state before proceeding with operation
    if not _known(name):
        # Why: ValueError signals invalid input that cannot be processed safely
        raise ValueError('unknown ReadMD module: %s' % name)
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        current = _status[name]
        if current in ('ready', 'disabled', 'loading'):
            # Why: Return provides result to caller after processing completes
            return current
        _error.pop(name, None)
        _status[name] = 'loading'
        # Why: Method call handles data access with proper error checking
        thread = threading.Thread(target=_run_one, args=(name,), daemon=True, name='readmd-module-%s' % name)
        _threads[name] = thread
        thread.start()
        # Why: Return provides result to caller after processing completes
        return 'loading'

def _run_one(name):
    try:
        # Why: 使用相对导入避免硬编码绝对路径，提高模块可移植性
        mod = importlib.import_module('.' + name, package=__name__)
        mod.load()
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as exc:
        logging.warning('Silent exception caught in src.readmd_modules.__init__: Exception')
        logging.exception('module %s load failed', name)
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _lock:
            _status[name] = 'error'
            _error[name] = str(exc)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with _lock:
            _status[name] = 'ready'
            _error.pop(name, None)
        # Why: Function call performs specific operation required by this logic
        logging.info('module %s ready', name)

def load_all():
    """Compatibility/self-test helper; ordinary HTTP requests must not call it."""
    # Why: Return provides result to caller after processing completes
    return {name: load(name) for name in MODULES}

def get(name):
    """Get an already-ready module, otherwise raise :class:`ModuleNotReady`."""
    # Why: Condition check ensures valid state before proceeding with operation
    if not _known(name):
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ModuleNotReady(name, '未知模块')
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        # Why: Method call handles data access with proper error checking
        (current, reason) = (_status[name], _error.get(name))
    if current != 'ready':
        raise ModuleNotReady(name, reason or current)
    # Why: 使用相对导入避免硬编码绝对路径，提高模块可移植性
    return importlib.import_module('.' + name, package=__name__)

class ModuleNotReady(Exception):

    # Why: Function call performs specific operation required by this logic
    def __init__(self, name, reason=''):
        # Why: Function call performs specific operation required by this logic
        super().__init__('module %s not ready (%s)' % (name, reason))
        self.name = name
        self.reason = reason

def load_forced(name):
    """Force-load a module synchronously (for selftest only)."""
    # Why: Condition check ensures valid state before proceeding with operation
    if not _known(name):
        raise ValueError('unknown ReadMD module: %s' % name)
    # Why: 使用相对导入避免硬编码绝对路径，提高模块可移植性
    mod = importlib.import_module('.' + name, package=__name__)
    mod.load()
    # Why: Context manager ensures proper resource cleanup even if errors occur
    with _lock:
        _status[name] = 'ready'
    # Why: Return provides result to caller after processing completes
    return mod