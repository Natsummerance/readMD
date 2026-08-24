# -*- coding: utf-8 -*-
"""ReadMD 输入与安全校验模块：防止路径遍历、Shell 注入与 SSRF 风险。"""

import ipaddress
import os
import re
import shlex
from typing import List, Optional, Union
from urllib.parse import urlparse


class ValidationError(Exception):
    """输入验证错误异常。"""
    pass


def validate_file_path(
    path: str,
    allowed_extensions: Optional[List[str]] = None,
    allowed_dirs: Optional[List[str]] = None
) -> str:
    """验证文件路径合法性，防止 null 字节截断与越权遍历。

    支持包含正常空格、圆括号（如 Windows 的 Program Files (x86) 或 笔记 (1).md）的合法路径。
    """
    if not path or not isinstance(path, str):
        raise ValidationError('路径不能为空')
    if '\x00' in path or any(ord(c) < 32 and c not in '\t\r\n' for c in path):
        raise ValidationError('路径包含非法控制字符')
    
    # 检查 shell 注入危险字符（在直接传入子进程前拦截）
    if re.search(r'[;&|`$]', path):
        raise ValidationError('路径包含危险字符')

    abs_path = os.path.realpath(os.path.normpath(path))

    if allowed_dirs:
        abs_allowed = [os.path.realpath(d) for d in allowed_dirs]
        if not any(paths_within(abs_path, allowed) for allowed in abs_allowed):
            raise ValidationError(f'路径不在允许的目录范围内: {abs_path}')
    else:
        # 非法高危系统路径检查 (POSIX)
        if os.name != 'nt':
            system_dirs = ['/etc', '/proc', '/sys', '/dev']
            if any(paths_within(abs_path, system_dir) or abs_path == os.path.realpath(system_dir)
                   for system_dir in system_dirs):
                raise ValidationError('不允许访问系统受保护目录')

    if allowed_extensions:
        _, ext = os.path.splitext(abs_path)
        ext_lower = ext.lower()
        normalized_exts = [e.lower() if e.startswith('.') else f'.{e.lower()}' for e in allowed_extensions]
        if ext_lower not in normalized_exts:
            raise ValidationError(f'不支持的文件类型: {ext}')

    return abs_path


def paths_within(path, root):
    """Return whether path equals or descends from root without prefix tricks."""
    path = os.path.normcase(os.path.realpath(path))
    root = os.path.normcase(os.path.realpath(root))
    try:
        return os.path.commonpath((path, root)) == root
    except (OSError, ValueError):
        return False


def validate_command(cmd: Union[str, List[str]]) -> List[str]:
    """验证可执行命令与参数，防止 Shell 注入。"""
    if not cmd:
        raise ValidationError('命令不能为空')
    if isinstance(cmd, str):
        if '\x00' in cmd or any(ord(c) < 32 and c not in '\t\r\n' for c in cmd):
            raise ValidationError('命令包含非法字符')
        if re.search(r'[;&|`$]', cmd):
            raise ValidationError('命令包含危险操作符')
        try:
            cmd_parts = shlex.split(cmd, posix=(os.name != 'nt'))
        except ValueError:
            raise ValidationError('命令解析格式错误')
    else:
        cmd_parts = list(cmd)

    if not cmd_parts:
        raise ValidationError('命令不能为空')

    # 验证各参数不包含 null 字节
    for arg in cmd_parts:
        if '\x00' in str(arg):
            raise ValidationError('命令参数包含非法字符')

    return cmd_parts


def validate_url(url: str, allow_private: bool = True) -> str:
    """验证 URL 合法性与 SSRF 防护。

    Args:
        url: 待校验的 URL 字符串
        allow_private: 是否允许访问 localhost / 私有 IP 网段（本地自测与局域网模式需为 True）
    """
    if not url or not isinstance(url, str):
        raise ValidationError('URL 不能为空')
    try:
        parsed = urlparse(url.strip())
    except Exception as e:
        raise ValidationError(f'URL 解析失败: {e}')

    if parsed.scheme not in ('http', 'https'):
        raise ValidationError('只支持 HTTP / HTTPS 协议')

    hostname = parsed.hostname
    if not hostname:
        raise ValidationError('无效的 URL: 缺少主机名')

    if not allow_private:
        # SSRF 防御模式：禁止环回地址与私有网段
        host_lower = hostname.lower()
        if host_lower in ('localhost', '127.0.0.1', '::1', '0.0.0.0') or host_lower.endswith(('.local', '.internal', '.localhost')):
            raise ValidationError('不允许访问本地或内部网络地址')
        try:
            ip = ipaddress.ip_address(hostname)
            if ip.is_private or ip.is_loopback or ip.is_link_local or ip.is_reserved:
                raise ValidationError('不允许访问内部私有网络地址')
        except ValueError:
            pass

    return url.strip()
