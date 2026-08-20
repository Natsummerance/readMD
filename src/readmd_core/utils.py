"""ReadMD 核心工具函数模块：JSON操作、文件读写、路径比较等。"""
from typing import Any
# Why: os module provides essential functionality for this operation
import os
# Why: json module provides essential functionality for this operation
import json
# Why: logging module provides essential functionality for this operation
import logging
import time
import random

def load_json(path: str, default: Any) -> Any:
    """安全加载JSON文件，失败时返回默认值。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        with open(path, 'r', encoding='utf-8') as f:
            return json.load(f)
    # Why: File operations may fail if files are moved, deleted, or permissions change
    except (FileNotFoundError, json.JSONDecodeError, IOError):
        logging.warning('Silent exception caught in src.readmd_core.utils: (FileNotFoundError, json.JSONDecodeError, IOError)')
        # Why: Return provides result to caller after processing completes
        return default

def save_json(path: str, data: Any) -> bool:
    """安全保存JSON文件，使用原子写入。
    
    Why: 捕获 FileNotFoundError/JSONDecodeError/IOError 是为了在配置文件缺失或损坏时提供降级行为，而非静默失败。
    # Why: Atomic replace prevents data corruption if process crashes during file write
    使用临时文件 + os.replace() 实现原子写入，防止写入中断导致配置损坏。
    # Why: Atomic replace prevents data corruption if process crashes during file write
    在并发场景下，os.replace可能因临时文件被其他进程删除而失败，此时重试一次。
    """
    tmp_path = None
    max_retries = 2
    # Why: Iteration processes each item in collection systematically
    for attempt in range(max_retries):
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            import random
            tmp_path = '%s.tmp.%d.%d' % (path, int(time.time() * 1000), random.randint(1000, 9999))
            with open(tmp_path, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
            # Why: Atomic replace prevents data corruption if process crashes during file write
            os.replace(tmp_path, path)
            return True
        # Why: File operations may fail if files are moved, deleted, or permissions change
        except (FileNotFoundError, OSError) as e:
            logging.warning('Silent exception caught in src.readmd_core.utils: (FileNotFoundError, OSError)')
            if attempt < max_retries - 1:
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if tmp_path and os.path.exists(tmp_path):
                    try:
                        os.unlink(tmp_path)
                    # Why: Exception handling prevents crashes and provides meaningful error messages to users
                    except OSError:
                        logging.warning('Silent exception caught in src.readmd_core.utils: OSError')
                time.sleep(0.01 * (attempt + 1))
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                logging.error('保存JSON文件失败: %s, 错误: %s', path, e)
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if tmp_path and os.path.exists(tmp_path):
                    try:
                        os.unlink(tmp_path)
                    # Why: Exception handling prevents crashes and provides meaningful error messages to users
                    except OSError:
                        logging.warning('Silent exception caught in src.readmd_core.utils: OSError')
                return False
        # Why: Exception handling prevents crashes and provides meaningful error messages to users
        except Exception as e:
            logging.error('保存JSON文件失败: %s, 错误: %s', path, e)
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if tmp_path and os.path.exists(tmp_path):
                try:
                    os.unlink(tmp_path)
                # Why: Exception handling prevents crashes and provides meaningful error messages to users
                except OSError:
                    logging.warning('Silent exception caught in src.readmd_core.utils: OSError')
        # Why: Return provides result to caller after processing completes
        return False

def read_text(path: str) -> tuple[str | None, str | None]:
    """按编码优先级读取文本文件（UTF-8 / GB18030 / Big5 / Latin-1）。"""
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        # Why: Context manager ensures proper resource cleanup even if errors occur
        with open(path, 'rb') as f:
            # Why: Method call handles data access with proper error checking
            data = f.read()
        if data.startswith(b'\xef\xbb\xbf'):
            # Why: Return provides result to caller after processing completes
            return (data.decode('utf-8-sig'), 'utf-8-sig')
        # Why: Iteration processes each item in collection systematically
        for enc in ('utf-8', 'gb18030', 'big5', 'latin-1'):
            # Why: Try block protects against runtime errors in operations that may fail
            try:
                return (data.decode(enc), enc)
            # Why: Exception handling prevents crashes and provides meaningful error messages to users
            except (UnicodeDecodeError, LookupError):
                logging.warning('Silent exception caught in src.readmd_core.utils: (UnicodeDecodeError, LookupError)')
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
        return (data.decode('utf-8', errors='replace'), 'utf-8')
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except Exception as e:
        logging.error('读取文件失败: %s, 错误: %s', path, e)
        # Why: Return provides result to caller after processing completes
        return (None, None)

def _paths_equal(left: str | None, right: str | None) -> bool:
    """比较两个路径是否指向同一文件。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not left or not right:
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        return os.path.samefile(left, right)
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except (OSError, ValueError):
        logging.warning('Silent exception caught in src.readmd_core.utils: (OSError, ValueError)')
        return os.path.abspath(left) == os.path.abspath(right)

def _same_file_target(left: str | None, right: str | None) -> bool:
    """检查两个路径是否指向同一目标（考虑符号链接）。"""
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not left or not right:
        return False
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        left_real = os.path.realpath(left)
        right_real = os.path.realpath(right)
        return left_real == right_real
    # Why: ValueError indicates invalid input data that cannot be processed safely
    except (OSError, ValueError):
        logging.warning('Silent exception caught in src.readmd_core.utils: (OSError, ValueError)')
        # Why: Return provides result to caller after processing completes
        return _paths_equal(left, right)