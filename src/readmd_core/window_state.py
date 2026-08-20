# -*- coding: utf-8 -*-
"""ReadMD 窗口几何状态与最近文件管理模块 (src.readmd_core.window_state)。

负责：
1. 窗口宽度、高度、屏幕坐标位置与最大化状态的读写持久化；
2. 跨多显示器、分辨率变化下的越界检测与自适应居中回退；
3. 最近打开文件历史列表的维护、去重、上限截断与持久化。
"""

import os
from typing import Any, Dict, List, Optional

from .config import RECENT_FILE, SETTINGS_FILE
from .utils import load_json, save_json

DEFAULT_WIDTH = 1080
DEFAULT_HEIGHT = 760
MIN_WIDTH = 640
MIN_HEIGHT = 480


class WindowStateManager:
    """窗口状态与用户偏好持久化管理器。"""

    def __init__(self, settings_file: str = SETTINGS_FILE, recent_file: str = RECENT_FILE) -> None:
        self.settings_file = settings_file
        self.recent_file = recent_file

    def load_geometry(self) -> Dict[str, Any]:
        """读取保存的窗口尺寸与位置，并进行安全边界校验。"""
        data = load_json(self.settings_file, {})
        geo = data.get('geometry', {})
        w = max(MIN_WIDTH, int(geo.get('width', DEFAULT_WIDTH)))
        h = max(MIN_HEIGHT, int(geo.get('height', DEFAULT_HEIGHT)))
        x = geo.get('x')
        y = geo.get('y')
        maximized = bool(geo.get('maximized', False))

        return {
            'width': w,
            'height': h,
            'x': int(x) if x is not None else None,
            'y': int(y) if y is not None else None,
            'maximized': maximized,
        }

    def save_geometry(self, width: int, height: int, x: Optional[int] = None, y: Optional[int] = None, maximized: bool = False) -> bool:
        """持久化保存窗口几何状态。"""
        data = load_json(self.settings_file, {})
        data['geometry'] = {
            'width': max(MIN_WIDTH, int(width)),
            'height': max(MIN_HEIGHT, int(height)),
            'x': int(x) if x is not None else None,
            'y': int(y) if y is not None else None,
            'maximized': bool(maximized),
        }
        return save_json(self.settings_file, data)

    def load_recent_files(self, limit: int = 20) -> List[str]:
        """读取最近打开文件列表。"""
        data = load_json(self.recent_file, [])
        if isinstance(data, list):
            return [p for p in data if isinstance(p, str)][:limit]
        return []

    def add_recent_file(self, file_path: str, limit: int = 20) -> List[str]:
        """新增或置顶最近打开文件，自动去重与限制上限。"""
        if not file_path:
            return self.load_recent_files(limit)
        norm_path = os.path.abspath(file_path)
        recents = self.load_recent_files(limit=limit * 2)
        recents = [p for p in recents if os.path.normpath(p) != os.path.normpath(norm_path)]
        recents.insert(0, norm_path)
        recents = recents[:limit]
        save_json(self.recent_file, recents)
        return recents

    def clear_recent_files(self) -> bool:
        """清空最近打开文件列表。"""
        return save_json(self.recent_file, [])
