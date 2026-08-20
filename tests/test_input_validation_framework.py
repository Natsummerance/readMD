#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""安全测试：输入验证框架"""

import os
import sys
import pytest
import tempfile
import json

# 添加项目根目录到路径
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


class TestInputValidationFramework:
    """测试输入验证框架"""
    
    def test_validate_request_params(self):
        """测试请求参数验证"""
        from src.readmd_modules.validators import validate_request_params
        
        # 定义验证规则
        rules = {
            'filename': {'type': 'str', 'required': True, 'max_length': 255},
            'page': {'type': 'int', 'required': False, 'min': 1, 'max': 1000},
            'limit': {'type': 'int', 'required': False, 'default': 10, 'min': 1, 'max': 100},
        }
        
        # 有效参数
        params = {'filename': 'test.md', 'page': 1, 'limit': 20}
        validated = validate_request_params(params, rules)
        
        assert validated['filename'] == 'test.md'
        assert validated['page'] == 1
        assert validated['limit'] == 20
    
    def test_validate_missing_required_param(self):
        """测试缺少必需参数"""
        from src.readmd_modules.validators import validate_request_params
        
        rules = {
            'filename': {'type': 'str', 'required': True},
        }
        
        params = {}
        
        with pytest.raises(Exception, match="缺少必需参数"):
            validate_request_params(params, rules)
    
    def test_validate_invalid_type(self):
        """测试无效类型"""
        from src.readmd_modules.validators import validate_request_params
        
        rules = {
            'page': {'type': 'int', 'required': True},
        }
        
        params = {'page': 'not_a_number'}
        
        with pytest.raises(Exception, match="类型错误"):
            validate_request_params(params, rules)
    
    def test_validate_out_of_range(self):
        """测试超出范围"""
        from src.readmd_modules.validators import validate_request_params
        
        rules = {
            'page': {'type': 'int', 'required': True, 'min': 1, 'max': 100},
        }
        
        params = {'page': 999}
        
        with pytest.raises(Exception, match="超出范围"):
            validate_request_params(params, rules)
    
    def test_validate_default_value(self):
        """测试默认值"""
        from src.readmd_modules.validators import validate_request_params
        
        rules = {
            'limit': {'type': 'int', 'required': False, 'default': 10},
        }
        
        params = {}
        validated = validate_request_params(params, rules)
        
        assert validated['limit'] == 10
    
    def test_validate_string_max_length(self):
        """测试字符串最大长度"""
        from src.readmd_modules.validators import validate_request_params
        
        rules = {
            'name': {'type': 'str', 'required': True, 'max_length': 10},
        }
        
        params = {'name': 'a' * 100}
        
        with pytest.raises(Exception, match="超过最大长度"):
            validate_request_params(params, rules)


class TestAPIEndpointValidation:
    """测试API端点验证装饰器"""
    
    def test_validate_decorator_blocks_invalid_input(self):
        """测试验证装饰器阻止无效输入"""
        from src.readmd_modules.validators import validate_api_endpoint
        
        @validate_api_endpoint({
            'filename': {'type': 'str', 'required': True, 'max_length': 255},
        })
        def mock_api(filename):
            return f"Processed: {filename}"
        
        # 有效输入
        result = mock_api('test.md')
        assert result == "Processed: test.md"
        
        # 无效输入（超长）
        with pytest.raises(Exception):
            mock_api('a' * 1000)
    
    def test_validate_decorator_sanitizes_input(self):
        """测试验证装饰器清理输入"""
        from src.readmd_modules.validators import validate_api_endpoint
        
        @validate_api_endpoint({
            'query': {'type': 'str', 'required': True, 'sanitize': True},
        })
        def mock_search(query):
            return query
        
        # 包含危险字符的输入应该被清理
        result = mock_search('<script>alert("xss")</script>')
        assert '<script>' not in result


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
