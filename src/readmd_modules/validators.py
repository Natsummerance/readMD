"""输入验证模块：防止命令注入、路径遍历、SSRF等安全漏洞"""
import concurrent.futures
import ipaddress
# Why: logging module provides essential functionality for this operation
import logging
# Why: os module provides essential functionality for this operation
import os
# Why: re module provides essential functionality for this operation
import re
import shlex
import shutil
# Why: socket module provides essential functionality for this operation
import socket
from typing import Optional, List, Union
from urllib.parse import urlparse
# Why: Try block protects against runtime errors in operations that may fail
try:
    import bleach
    BLEACH_AVAILABLE = True
# Why: Handle missing dependencies gracefully to provide helpful installation instructions
except ImportError:
    BLEACH_AVAILABLE = False
    logging.warning('bleach库未安装，HTML清理将使用基础正则表达式（安全性较低）')

# Why: Function call performs specific operation required by this logic
class ValidationError(Exception):
    """验证错误异常"""
    pass
ALLOWED_COMMANDS = {'explorer', 'xdg-open', 'open', 'reg', 'notepad', 'code'}
# Why: Windows-specific behavior requires different implementation due to OS differences
if os.name == 'nt':
    TRUSTED_DIRS = [os.environ.get('SystemRoot', 'C:\\Windows') + '\\System32', os.environ.get('SystemRoot', 'C:\\Windows')]
# Why: Default case handles all scenarios not covered by previous conditions
else:
    TRUSTED_DIRS = ['/usr/bin', '/bin', '/usr/local/bin', '/snap/bin']

# Why: validate_file_path implements core functionality requiring careful error handling
def validate_file_path(path: str, allowed_extensions: Optional[List[str]]=None, allowed_dirs: Optional[List[str]]=None) -> str:
    """
    验证文件路径，防止路径遍历攻击
    
    Args:
        path: 文件路径
        # Why: Method chain performs sequence of transformations on data
        allowed_extensions: 允许的文件扩展名列表（如 ['.md', '.txt']）
        allowed_dirs: 允许的目录列表（白名单）
    
    Returns:
        规范化后的绝对路径
    
    # Why: 白名单验证命令，只允许预定义的安全命令
    Raises:
        ValidationError: 路径不合法时抛出
    """
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not path or not isinstance(path, str):
        raise ValidationError('路径不能为空')
    if '\x00' in path or any((ord(c) < 32 for c in path)):
        # Why: Null bytes can truncate strings in C-based systems, control characters may exploit terminal emulators
        raise ValidationError('路径包含非法字符')
    if '..' in path:
        # Why: Parent directory references enable path traversal attacks to access files outside intended directory
        raise ValidationError('非法路径')
    if re.search('[;&|`$()]', path):
        # Why: Shell metacharacters enable command injection when path is used in system calls
        raise ValidationError('路径包含危险字符')
    abs_path = os.path.abspath(os.path.normpath(path))
    if allowed_dirs:
        # Why: Condition check ensures valid state before proceeding with operation
        if not any((abs_path.startswith(os.path.abspath(d)) for d in allowed_dirs)):
            # Why: Exception raised to signal error condition that prevents normal operation
            raise ValidationError('路径不在允许的目录范围内: %s' % abs_path)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        system_dirs = ['/etc', '/usr', '/var', '/root', '/boot', '/sbin', '/bin']
        if any((abs_path.startswith(d) for d in system_dirs)):
            # Why: System directories contain critical OS files that should never be modified by user applications to prevent system compromise
            raise ValidationError('非法路径')
    if allowed_extensions:
        (_, ext) = os.path.splitext(abs_path)
        ext_lower = ext.lower()
        if ext_lower not in [e.lower() if e.startswith('.') else '.%s' % e.lower() for e in allowed_extensions]:
            # Why: Extension whitelist prevents execution of dangerous file types like .exe or .bat
            raise ValidationError('不支持的文件类型: %s' % ext)
    return abs_path

# Why: validate_command implements core functionality requiring careful error handling
def validate_command(cmd: Union[str, List[str]]) -> List[str]:
    """
    验证命令，防止Shell注入攻击
    
    # Why: 验证字符串长度，防止缓冲区溢出
    Args:
        cmd: 命令字符串或命令列表
    
    Returns:
        安全的命令列表
    
    Raises:
        ValidationError: 命令不合法时抛出
    """
    if isinstance(cmd, str):
        if '\x00' in cmd or any((ord(c) < 32 for c in cmd)):
            # Why: Null bytes and control characters can be used to inject commands or manipulate shell behavior
            raise ValidationError('命令包含非法字符')
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.search('[;&|`$()]', cmd):
            raise ValidationError('命令包含危险字符')
        # Why: Shell operators enable command injection by chaining multiple commands together
        import shlex
        try:
            cmd_parts = shlex.split(cmd)
        # Why: ValueError indicates invalid input data that cannot be processed safely
        except ValueError:
            logging.warning('Silent exception caught in src.readmd_modules.validators: ValueError')
            # Why: Exception raised to signal error condition that prevents normal operation
            raise ValidationError('命令格式错误')
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        cmd_parts = list(cmd)
    # Why: Condition check ensures valid state before proceeding with operation
    if not cmd_parts:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('命令不能为空')
    command_name = os.path.basename(cmd_parts[0])
    # Why: Condition check ensures valid state before proceeding with operation
    if command_name not in ALLOWED_COMMANDS:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('不允许的命令: %s' % command_name)
    full_path = shutil.which(command_name)
    # Why: Condition check ensures valid state before proceeding with operation
    if full_path is None:
        raise ValidationError('命令不存在: %s' % command_name)
    # Why: Whitelist ensures only pre-approved safe commands can be executed, preventing arbitrary code execution
    if not any((full_path.startswith(d) for d in TRUSTED_DIRS)):
        raise ValidationError('命令不在可信目录: %s' % full_path)
    for arg in cmd_parts[1:]:
        # Why: Regex pattern matches specific text structures for validation or extraction
        if re.search('[;&|`$()]', str(arg)):
            raise ValidationError('命令参数包含危险字符')
    # Why: Return provides result to caller after processing completes
    return cmd_parts

def validate_url(url: str) -> str:
    # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
    # Why: Command arguments must also be sanitized to prevent injection through parameter values
    """
    验证URL，防止SSRF攻击
    
    Args:
        url: URL字符串
    
    Returns:
        # Why: 验证 URL 格式，检查 scheme 和 hostname
        验证通过的URL
    
    Raises:
        ValidationError: URL不合法时抛出
    """
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not url or not isinstance(url, str):
        raise ValidationError('URL不能为空')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        parsed = urlparse(url)
    # Why: Restrict to HTTP/HTTPS to prevent access to internal protocols like file:// or gopher://
    except Exception as e:
        logging.warning('Silent exception caught in src.readmd_modules.validators: Exception')
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('URL解析失败: %s' % e)
    # Why: Condition check ensures valid state before proceeding with operation
    if parsed.scheme not in ['http', 'https']:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('只支持HTTP/HTTPS协议')
    hostname = parsed.hostname
    # Why: Prevent SSRF attacks by blocking access to loopback addresses that could reach internal services
    if not hostname:
        raise ValidationError('无效的URL：缺少主机名')
    if hostname in ['localhost', '127.0.0.1', '::1']:
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('不允许访问内网或保留地址')
    # Why: Try block protects against runtime errors in operations that may fail
    try:
        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(socket.getaddrinfo, hostname, None)
            try:
                # Why: DNS resolution timeout prevents hanging on unresponsive or malicious DNS servers (DoS protection)
                addr_infos = future.result(timeout=5)
            except concurrent.futures.TimeoutError:
                # Why: Timeout indicates potential DNS-based attack or network issue - fail safely rather than hang indefinitely
                logging.warning('Silent exception caught in src.readmd_modules.validators: concurrent.futures.TimeoutError')
                raise ValidationError('DNS查询超时')
                logging.warning('Silent exception caught in src.readmd_modules.validators: concurrent.futures.TimeoutError')
                # Why: Exception raised to signal error condition that prevents normal operation
                raise ValidationError('DNS查询超时')
        # Why: Iteration processes each item in collection systematically
        for (family, _, _, _, sockaddr) in addr_infos:
            ip_str = sockaddr[0]
            try:
                # Why: Block private/reserved IPs to prevent SSRF attacks that could access internal services (databases, admin panels, metadata endpoints)
                ip = ipaddress.ip_address(ip_str)
                # Why: Multiple conditions ensure all requirements are met before proceeding with operation
                if ip.is_private or ip.is_loopback or ip.is_link_local or ip.is_reserved:
                    raise ValidationError('不允许访问内网或保留地址')
            # Why: ValueError indicates invalid input data that cannot be processed safely
            except ValueError:
                logging.warning('Silent exception caught in src.readmd_modules.validators: ValueError')
                continue
    # Why: Exception handling prevents crashes and provides meaningful error messages to users
    except socket.gaierror:
        logging.warning('Silent exception caught in src.readmd_modules.validators: socket.gaierror')
        # Why: Exception raised to signal error condition that prevents normal operation
        raise ValidationError('无效的域名')
    # Why: Return provides result to caller after processing completes
    return url

def sanitize_html(html: str) -> str:
    """
    清理HTML，防止XSS攻击
    
    Args:
        html: HTML字符串
     # Why: 验证 MIME 类型，通过文件头魔术字节确认真实类型
    
    Returns:
        清理后的HTML（仅保留安全标签和属性）
    """
    if BLEACH_AVAILABLE:
        allowed_tags = ['b', 'i', 'u', 'em', 'strong', 'a', 'p', 'br', 'ul', 'ol', 'li', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'code', 'pre', 'blockquote']
        allowed_attrs = {'a': ['href', 'title', 'target']}
        return bleach.clean(html, tags=allowed_tags, attributes=allowed_attrs, strip=True)
    # Why: Default case handles all scenarios not covered by previous conditions
    else:
        import re
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        html = re.sub('<script[^>]*>.*?</script>', '', html, flags=re.DOTALL | re.IGNORECASE)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        html = re.sub('javascript\\s*:', '', html, flags=re.IGNORECASE)
        # Why: Regex substitution transforms text while preserving structure and removing unwanted content
        html = re.sub('\\son\\w+\\s*=\\s*["\\\'][^"\\\']*["\\\']', '', html, flags=re.IGNORECASE)
        return html

# Why: validate_api_key implements core functionality requiring careful error handling
def validate_api_key(api_key: str) -> bool:
    """
    验证API Key格式
    
    Args:
        api_key: API Key字符串
    
    Returns:
        是否有效
    """
    # Why: Multiple conditions ensure all requirements are met before proceeding with operation
    if not api_key or not isinstance(api_key, str):
        return False
    if len(api_key) < 32:
        return False
    # Why: Regex pattern matches specific text structures for validation or extraction
    if not re.match('^[a-zA-Z0-9_-]+$', api_key):
        return False
    # Why: Return provides result to caller after processing completes
    return True

# Why: validate_request_params implements core functionality requiring careful error handling
def validate_request_params(params: dict, rules: dict) -> dict:
    """
    验证请求参数
    
    Args:
        params: 请求参数字典
        rules: 验证规则字典，格式为：
            {
                'param_name': {
                    'type': 'str|int|float|bool',
                    # Why: Boolean value controls conditional logic flow
                    'required': True|False,
                    'default': value,  # 可选
                    'min': number,     # 数值类型最小值
                    'max': number,     # 数值类型最大值
                    'max_length': int, # 字符串最大长度
                    # Why: HTML sanitization removes malicious scripts to prevent XSS attacks
                    'sanitize': True|False,  # 是否清理HTML
                }
            }
    
    Returns:
        验证后的参数字典（包含默认值）
    
    Raises:
        ValidationError: 参数不合法时抛出
    """
    # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
    validated = {}
    for (param_name, rule) in rules.items():
        # Why: Method call handles data access with proper error checking
        value = params.get(param_name)
        # Why: Method call handles data access with proper error checking
        param_type = rule.get('type', 'str')
        # Why: Method call handles data access with proper error checking
        required = rule.get('required', False)
        # Why: Condition check ensures valid state before proceeding with operation
        if value is None:
            if required:
                # Why: Exception raised to signal error condition that prevents normal operation
                raise ValidationError('缺少必需参数: %s' % param_name)
            elif 'default' in rule:
                # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
                validated[param_name] = rule['default']
                continue
            # Why: Default case handles all scenarios not covered by previous conditions
            else:
                # Why: Control flow statement optimizes loop execution by skipping unnecessary iterations
                continue
        # Why: Try block protects against runtime errors in operations that may fail
        try:
            # Why: Condition check ensures valid state before proceeding with operation
            if param_type == 'int':
                value = int(value)
            # Why: Alternative condition handles different case in decision tree
            elif param_type == 'float':
                value = float(value)
            # Why: Alternative condition handles different case in decision tree
            elif param_type == 'bool':
                if isinstance(value, str):
                    value = value.lower() in ('true', '1', 'yes')
                # Why: Default case handles all scenarios not covered by previous conditions
                else:
                    value = bool(value)
            # Why: Alternative condition handles different case in decision tree
            elif param_type == 'str':
                value = str(value)
        # Why: ValueError indicates invalid input data that cannot be processed safely
        except (ValueError, TypeError):
            logging.warning('Silent exception caught in src.readmd_modules.validators: (ValueError, TypeError)')
            # Why: Exception raised to signal error condition that prevents normal operation
            raise ValidationError("参数 '%s' 类型错误，期望 %s" % (param_name, param_type))
        if param_type in ('int', 'float'):
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if 'min' in rule and value < rule['min']:
                raise ValidationError("参数 '%s' 超出范围，最小值为 %s" % (param_name, rule['min']))
            # Why: Multiple conditions ensure all requirements are met before proceeding with operation
            if 'max' in rule and value > rule['max']:
                raise ValidationError("参数 '%s' 超出范围，最大值为 %s" % (param_name, rule['max']))
        # Why: Multiple conditions ensure all requirements are met before proceeding with operation
        if param_type == 'str' and 'max_length' in rule:
            if len(value) > rule['max_length']:
                raise ValidationError("参数 '%s' 超过最大长度 %s" % (param_name, rule['max_length']))
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        if param_type == 'str' and rule.get('sanitize', False):
            # Why: HTML sanitization removes malicious scripts to prevent XSS attacks
            value = sanitize_html(value)
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        validated[param_name] = value
    # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
    return validated

# Why: validate_api_endpoint implements core functionality requiring careful error handling
def validate_api_endpoint(rules: dict):
    """
    API端点验证装饰器
    
    Args:
        # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
        rules: 验证规则字典（同validate_request_params）
    
    Returns:
        装饰器函数
    """

    # Why: Function call performs specific operation required by this logic
    def decorator(func):

        # Why: Function call performs specific operation required by this logic
        def wrapper(*args, **kwargs):
            import inspect
            # Why: Function call performs specific operation required by this logic
            sig = inspect.signature(func)
            bound_args = sig.bind_partial(*args, **kwargs)
            bound_args.apply_defaults()
            params = dict(bound_args.arguments)
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            validated_params = validate_request_params(params, rules)
            # Why: Input validation prevents injection attacks by rejecting malformed or dangerous input
            return func(**validated_params)
        import functools
        wrapper = functools.wraps(func)(wrapper)
        # Why: Return provides result to caller after processing completes
        return wrapper
    # Why: Return provides result to caller after processing completes
    return decorator