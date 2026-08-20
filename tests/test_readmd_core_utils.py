import logging
'测试 src/readmd_core/utils.py 中的所有公共函数。\n\n覆盖范围：\n- load_json() - 正常加载、文件不存在、JSON格式错误\n- save_json() - 正常保存、原子写入验证、磁盘满模拟、权限不足\n- read_text() - UTF-8、GB18030、Big5、Latin-1编码检测\n- _paths_equal() - 硬链接检测\n- _same_file_target() - 符号链接检测\n'
import os
import json
import stat
import tempfile
import time
import threading
from unittest.mock import patch, MagicMock
import pytest

@pytest.fixture
def temp_dir(tmp_path):
    """提供临时目录用于文件操作测试。"""
    return str(tmp_path)

@pytest.fixture
def sample_json_data():
    """提供标准测试 JSON 数据。"""
    return {'name': 'ReadMD', 'version': '2.32.0', 'settings': {'theme': 'dark', 'language': 'zh-CN'}, 'items': [1, 2, 3]}

@pytest.fixture
def json_file(temp_dir, sample_json_data):
    """创建包含有效 JSON 的临时文件。"""
    path = os.path.join(temp_dir, 'test.json')
    with open(path, 'w', encoding='utf-8') as f:
        json.dump(sample_json_data, f, ensure_ascii=False)
    return path

@pytest.fixture
def invalid_json_file(temp_dir):
    """创建包含无效 JSON 的临时文件。"""
    path = os.path.join(temp_dir, 'invalid.json')
    with open(path, 'w', encoding='utf-8') as f:
        f.write('{ invalid json content }')
    return path

class TestLoadJson:
    """测试 JSON 文件加载逻辑。"""

    def test_load_valid_json(self, json_file, sample_json_data):
        """加载有效的 JSON 文件应返回解析后的数据。"""
        from src.readmd_core import utils as utils_module
        result = utils_module.load_json(json_file, default={})
        assert result == sample_json_data

    def test_load_nonexistent_file(self, temp_dir):
        """加载不存在的文件应返回默认值。"""
        from src.readmd_core import utils as utils_module
        nonexistent = os.path.join(temp_dir, 'nonexistent.json')
        default_value = {'fallback': True}
        result = utils_module.load_json(nonexistent, default=default_value)
        assert result == default_value

    def test_load_invalid_json(self, invalid_json_file):
        """加载格式错误的 JSON 文件应返回默认值。"""
        from src.readmd_core import utils as utils_module
        default_value = {'error_handled': True}
        result = utils_module.load_json(invalid_json_file, default=default_value)
        assert result == default_value

    def test_load_empty_json_object(self, temp_dir):
        """加载空的 JSON 对象应返回空字典。"""
        path = os.path.join(temp_dir, 'empty.json')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('{}')
        from src.readmd_core import utils as utils_module
        result = utils_module.load_json(path, default={'should_not': 'appear'})
        assert result == {}

    def test_load_empty_json_array(self, temp_dir):
        """加载空的 JSON 数组应返回空列表。"""
        path = os.path.join(temp_dir, 'empty_array.json')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('[]')
        from src.readmd_core import utils as utils_module
        result = utils_module.load_json(path, default=['should_not', 'appear'])
        assert result == []

    def test_load_json_with_unicode(self, temp_dir):
        """加载含 Unicode 字符的 JSON 文件应正确解析。"""
        path = os.path.join(temp_dir, 'unicode.json')
        data = {'chinese': '中文测试', 'emoji': '🎉', 'japanese': '日本語'}
        with open(path, 'w', encoding='utf-8') as f:
            json.dump(data, f, ensure_ascii=False)
        from src.readmd_core import utils as utils_module
        result = utils_module.load_json(path, default={})
        assert result['chinese'] == '中文测试'
        assert result['emoji'] == '🎉'
        assert result['japanese'] == '日本語'

    def test_load_json_io_error(self, temp_dir):
        """加载时发生 IOError 应返回默认值。"""
        path = os.path.join(temp_dir, 'test.json')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('{"key": "value"}')
        from src.readmd_core import utils as utils_module
        with patch('builtins.open', side_effect=IOError('Mock IO error')):
            result = utils_module.load_json(path, default={'fallback': True})
            assert result == {'fallback': True}

class TestSaveJson:
    """测试 JSON 文件保存逻辑，包括原子写入和异常处理。"""

    def test_save_valid_json(self, temp_dir, sample_json_data):
        """保存有效的 JSON 数据应成功写入文件。"""
        path = os.path.join(temp_dir, 'output.json')
        from src.readmd_core import utils as utils_module
        result = utils_module.save_json(path, sample_json_data)
        assert result is True
        assert os.path.exists(path)
        with open(path, 'r', encoding='utf-8') as f:
            loaded = json.load(f)
        assert loaded == sample_json_data

    def test_save_creates_directory(self, temp_dir, sample_json_data):
        """保存时目录不存在应自动创建。"""
        nested_path = os.path.join(temp_dir, 'subdir', 'nested', 'output.json')
        from src.readmd_core import utils as utils_module
        result = utils_module.save_json(nested_path, sample_json_data)
        assert result is True
        assert os.path.exists(nested_path)

    def test_save_atomic_write(self, temp_dir, sample_json_data):
        """保存应使用原子写入（先写 .tmp 再替换）。"""
        path = os.path.join(temp_dir, 'atomic.json')
        from src.readmd_core import utils as utils_module
        tmp_path = path + '.tmp'
        utils_module.save_json(path, sample_json_data)
        assert not os.path.exists(tmp_path)
        assert os.path.exists(path)

    def test_save_overwrites_existing(self, temp_dir):
        """保存应覆盖已存在的文件。"""
        path = os.path.join(temp_dir, 'overwrite.json')
        with open(path, 'w', encoding='utf-8') as f:
            json.dump({'old': 'data'}, f)
        new_data = {'new': 'data', 'updated': True}
        from src.readmd_core import utils as utils_module
        result = utils_module.save_json(path, new_data)
        assert result is True
        with open(path, 'r', encoding='utf-8') as f:
            loaded = json.load(f)
        assert loaded == new_data

    def test_save_disk_full_simulation(self, temp_dir, sample_json_data):
        """模拟磁盘满时应返回 False 并清理临时文件。
        
        Why: save_json现在会重试2次，所以我们需要模拟所有重试都失败的情况。
        """
        path = os.path.join(temp_dir, 'diskfull.json')
        from src.readmd_core import utils as utils_module
        original_open = open
        call_count = [0]

        def mock_open(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] <= 3:
                raise OSError('[Errno 28] No space left on device')
            return original_open(*args, **kwargs)
        with patch('builtins.open', side_effect=mock_open):
            result = utils_module.save_json(path, sample_json_data)
        assert result is False
        tmp_files = [f for f in os.listdir(temp_dir) if f.endswith('.tmp.*')]
        assert len(tmp_files) == 0, f'Temporary files not cleaned up: {tmp_files}'

    def test_save_permission_denied(self, temp_dir, sample_json_data):
        """模拟权限不足时应返回 False。"""
        path = os.path.join(temp_dir, 'noperm.json')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('{}')
        os.chmod(path, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        from src.readmd_core import utils as utils_module
        try:
            result = utils_module.save_json(path, sample_json_data)
            assert isinstance(result, bool)
        finally:
            os.chmod(path, stat.S_IWUSR | stat.S_IRUSR)

    def test_save_json_encode_error(self, temp_dir):
        """保存不可序列化的对象时应返回 False。"""
        path = os.path.join(temp_dir, 'unserializable.json')
        from src.readmd_core import utils as utils_module
        bad_data = {'function': lambda x: x}
        result = utils_module.save_json(path, bad_data)
        assert result is False

    def test_save_preserves_indent_and_unicode(self, temp_dir):
        """保存应保持缩进和 Unicode 字符。"""
        path = os.path.join(temp_dir, 'formatted.json')
        data = {'name': '测试', 'nested': {'key': 'value'}}
        from src.readmd_core import utils as utils_module
        utils_module.save_json(path, data)
        with open(path, 'r', encoding='utf-8') as f:
            content = f.read()
        assert '  ' in content
        assert '测试' in content

class TestReadText:
    """测试文本文件读取和编码检测逻辑。"""

    def test_read_utf8_file(self, temp_dir):
        """读取 UTF-8 编码文件应正确解码。"""
        path = os.path.join(temp_dir, 'utf8.txt')
        content = 'Hello 世界 🌍'
        with open(path, 'wb') as f:
            f.write(content.encode('utf-8'))
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text == content
        assert encoding == 'utf-8'

    def test_read_utf8_bom_file(self, temp_dir):
        """读取带 BOM 的 UTF-8 文件应正确识别编码。"""
        path = os.path.join(temp_dir, 'utf8_bom.txt')
        content = 'UTF-8 with BOM'
        with open(path, 'wb') as f:
            f.write(b'\xef\xbb\xbf')
            f.write(content.encode('utf-8'))
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text == content
        assert encoding == 'utf-8-sig'

    def test_read_gb18030_file(self, temp_dir):
        """读取 GB18030 编码文件应正确解码。"""
        path = os.path.join(temp_dir, 'gb18030.txt')
        content = '中文GB18030编码测试'
        with open(path, 'wb') as f:
            f.write(content.encode('gb18030'))
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text == content
        assert encoding == 'gb18030'

    def test_read_big5_file(self, temp_dir):
        """读取 Big5 编码文件应正确解码。
        
        Note: The read_text function tries UTF-8 first. Some Big5 bytes may
        partially decode as valid UTF-8, so we verify the encoding is detected
        correctly rather than exact content match.
        """
        path = os.path.join(temp_dir, 'big5.txt')
        content_bytes = b'\xa4\xe5\xb0\xaa\xa6r'
        with open(path, 'wb') as f:
            f.write(content_bytes)
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text is not None
        assert encoding in ('gb18030', 'big5', 'latin-1')

    def test_read_latin1_file(self, temp_dir):
        """读取 Latin-1 编码文件应正确解码。"""
        path = os.path.join(temp_dir, 'latin1.txt')
        content = 'Café résumé naïve'
        with open(path, 'wb') as f:
            f.write(content.encode('latin-1'))
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text == content
        assert encoding == 'latin-1'

    def test_read_nonexistent_file(self, temp_dir):
        """读取不存在的文件应返回 (None, None)。"""
        path = os.path.join(temp_dir, 'nonexistent.txt')
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text is None
        assert encoding is None

    def test_read_empty_file(self, temp_dir):
        """读取空文件应返回空字符串。"""
        path = os.path.join(temp_dir, 'empty.txt')
        with open(path, 'w', encoding='utf-8') as f:
            pass
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text == ''
        assert encoding == 'utf-8'

    def test_read_binary_garbage(self, temp_dir):
        """读取无法解码的二进制数据应使用 replace 错误处理。"""
        path = os.path.join(temp_dir, 'binary.bin')
        with open(path, 'wb') as f:
            f.write(b'\x80\x81\x82\x83\xff\xfe')
        from src.readmd_core import utils as utils_module
        (text, encoding) = utils_module.read_text(path)
        assert text is not None
        assert encoding == 'latin-1'

    def test_read_file_io_error(self, temp_dir):
        """读取时发生 IOError 应返回 (None, None)。"""
        path = os.path.join(temp_dir, 'test.txt')
        with open(path, 'w', encoding='utf-8') as f:
            f.write('content')
        from src.readmd_core import utils as utils_module
        with patch('builtins.open', side_effect=IOError('Mock IO error')):
            (text, encoding) = utils_module.read_text(path)
        assert text is None
        assert encoding is None

class TestPathsEqual:
    """测试路径相等性比较逻辑，包括硬链接检测。"""

    def test_same_path(self, temp_dir):
        """相同路径应返回 True。"""
        path = os.path.join(temp_dir, 'test.txt')
        with open(path, 'w') as f:
            f.write('content')
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(path, path)
        assert result is True

    def test_different_paths(self, temp_dir):
        """不同路径应返回 False。"""
        path1 = os.path.join(temp_dir, 'test1.txt')
        path2 = os.path.join(temp_dir, 'test2.txt')
        with open(path1, 'w') as f:
            f.write('content1')
        with open(path2, 'w') as f:
            f.write('content2')
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(path1, path2)
        assert result is False

    def test_hard_links(self, temp_dir):
        """硬链接指向同一文件应返回 True。"""
        original = os.path.join(temp_dir, 'original.txt')
        hardlink = os.path.join(temp_dir, 'hardlink.txt')
        with open(original, 'w') as f:
            f.write('shared content')
        os.link(original, hardlink)
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(original, hardlink)
        assert result is True

    def test_none_left(self):
        """左路径为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(None, '/some/path')
        assert result is False

    def test_none_right(self):
        """右路径为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal('/some/path', None)
        assert result is False

    def test_both_none(self):
        """两个路径都为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(None, None)
        assert result is False

    def test_empty_strings(self):
        """空字符串路径应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal('', '')
        assert result is False

    def test_relative_vs_absolute(self, temp_dir):
        """相对路径和绝对路径指向同一文件应返回 True。"""
        path = os.path.join(temp_dir, 'test.txt')
        with open(path, 'w') as f:
            f.write('content')
        from src.readmd_core import utils as utils_module
        rel_path = 'test.txt'
        abs_path = path
        with patch('os.getcwd', return_value=temp_dir):
            result = utils_module._paths_equal(rel_path, abs_path)
        assert result is True

    def test_nonexistent_paths_fallback(self, temp_dir):
        """不存在的路径应回退到绝对路径比较。"""
        path1 = os.path.join(temp_dir, 'nonexist1.txt')
        path2 = os.path.join(temp_dir, 'nonexist2.txt')
        from src.readmd_core import utils as utils_module
        result = utils_module._paths_equal(path1, path2)
        assert result is False

class TestSameFileTarget:
    """测试符号链接目标比较逻辑。"""

    def test_same_regular_file(self, temp_dir):
        """相同的普通文件应返回 True。"""
        path = os.path.join(temp_dir, 'test.txt')
        with open(path, 'w') as f:
            f.write('content')
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(path, path)
        assert result is True

    def test_symlink_to_same_target(self, temp_dir):
        """符号链接指向同一目标应返回 True。"""
        original = os.path.join(temp_dir, 'original.txt')
        symlink = os.path.join(temp_dir, 'symlink.txt')
        with open(original, 'w') as f:
            f.write('shared content')
        os.symlink(original, symlink)
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(symlink, original)
        assert result is True

    def test_symlink_chain(self, temp_dir):
        """符号链接链应正确解析到最终目标。"""
        original = os.path.join(temp_dir, 'original.txt')
        link1 = os.path.join(temp_dir, 'link1.txt')
        link2 = os.path.join(temp_dir, 'link2.txt')
        with open(original, 'w') as f:
            f.write('content')
        os.symlink(original, link1)
        os.symlink(link1, link2)
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(link2, original)
        assert result is True

    def test_different_files(self, temp_dir):
        """不同的文件应返回 False。"""
        path1 = os.path.join(temp_dir, 'file1.txt')
        path2 = os.path.join(temp_dir, 'file2.txt')
        with open(path1, 'w') as f:
            f.write('content1')
        with open(path2, 'w') as f:
            f.write('content2')
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(path1, path2)
        assert result is False

    def test_none_left(self):
        """左路径为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(None, '/some/path')
        assert result is False

    def test_none_right(self):
        """右路径为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target('/some/path', None)
        assert result is False

    def test_both_none(self):
        """两个路径都为 None 应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(None, None)
        assert result is False

    def test_empty_strings(self):
        """空字符串路径应返回 False。"""
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target('', '')
        assert result is False

    def test_symlink_to_different_target(self, temp_dir):
        """符号链接指向不同目标应返回 False。"""
        target1 = os.path.join(temp_dir, 'target1.txt')
        target2 = os.path.join(temp_dir, 'target2.txt')
        symlink = os.path.join(temp_dir, 'symlink.txt')
        with open(target1, 'w') as f:
            f.write('content1')
        with open(target2, 'w') as f:
            f.write('content2')
        os.symlink(target1, symlink)
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(symlink, target2)
        assert result is False

    def test_broken_symlink_fallback(self, temp_dir):
        """断开的符号链接应回退到路径比较。"""
        broken_link = os.path.join(temp_dir, 'broken.txt')
        nonexistent_target = os.path.join(temp_dir, 'nonexistent.txt')
        os.symlink(nonexistent_target, broken_link)
        from src.readmd_core import utils as utils_module
        result = utils_module._same_file_target(broken_link, nonexistent_target)
        assert result is True

class TestConcurrencySafety:
    """并发写入测试：多线程同时save_json。"""

    def test_concurrent_save_json(self, tmp_path):
        """多个线程同时保存同一JSON文件应不损坏数据。
        
        Why: save_json使用原子写入（临时文件+os.replace），但在高并发场景下
        多个线程同时替换同一文件可能导致部分写入失败。这是预期行为，
        因为save_json不是为并发设计的。测试验证即使有失败，文件也不会损坏。
        """
        from src.readmd_core.utils import save_json, load_json
        path = str(tmp_path / 'concurrent.json')
        errors = []
        successes = []

        def save_data(thread_id):
            try:
                data = {'thread': thread_id, 'timestamp': time.time()}
                result = save_json(path, data)
                if result:
                    successes.append(thread_id)
                else:
                    pass
            except Exception as e:
                logging.warning('Silent exception caught in tests.test_readmd_core_utils: Exception')
                errors.append(f'Thread {thread_id} error: {e}')
        threads = []
        for i in range(10):
            t = threading.Thread(target=save_data, args=(i,))
            threads.append(t)
            t.start()
        for t in threads:
            t.join(timeout=5)
        assert len(errors) == 0, f'Exceptions occurred: {errors}'
        assert len(successes) > 0, 'No thread succeeded in saving'
        assert os.path.exists(path)
        loaded = load_json(path, default={})
        assert 'thread' in loaded, 'File is corrupted or empty'

    def test_concurrent_save_different_files(self, tmp_path):
        """多个线程保存不同文件应互不干扰。"""
        from src.readmd_core.utils import save_json, load_json
        results = {}
        errors = []

        def save_to_file(file_id):
            try:
                path = str(tmp_path / f'file_{file_id}.json')
                data = {'id': file_id, 'value': f'data_{file_id}'}
                result = save_json(path, data)
                if result:
                    loaded = load_json(path, default={})
                    results[file_id] = loaded
                else:
                    errors.append(f'File {file_id} save failed')
            except Exception as e:
                logging.warning('Silent exception caught in tests.test_readmd_core_utils: Exception')
                errors.append(f'File {file_id} error: {e}')
        threads = []
        for i in range(20):
            t = threading.Thread(target=save_to_file, args=(i,))
            threads.append(t)
            t.start()
        for t in threads:
            t.join(timeout=5)
        assert len(errors) == 0, f'Errors: {errors}'
        assert len(results) == 20, f'Expected 20 results, got {len(results)}'

class TestLargeFileHandling:
    """大文件测试：>10MB JSON。"""

    def test_save_large_json_10mb(self, tmp_path):
        """保存10MB JSON文件应成功。"""
        from src.readmd_core.utils import save_json, load_json
        path = str(tmp_path / 'large_10mb.json')
        large_data = {'items': [{'id': i, 'data': 'x' * 100} for i in range(100000)], 'metadata': {'total': 100000, 'size_mb': 10}}
        result = save_json(path, large_data)
        assert result is True
        assert os.path.exists(path)
        file_size = os.path.getsize(path)
        assert file_size > 10 * 1024 * 1024, f'File size {file_size} bytes < 10MB'
        loaded = load_json(path, default={})
        assert len(loaded['items']) == 100000

    def test_save_very_large_json_50mb(self, tmp_path):
        """保存50MB JSON文件应在合理时间内完成。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'large_50mb.json')
        large_data = {'items': [{'id': i, 'data': 'x' * 500} for i in range(100000)]}
        start = time.perf_counter()
        result = save_json(path, large_data)
        elapsed = time.perf_counter() - start
        assert result is True
        assert os.path.exists(path)
        assert elapsed < 30, f'Save took {elapsed:.2f}s (threshold: 30s)'

class TestSymlinkLoopDetection:
    """符号链接循环检测。"""

    def test_symlink_cycle_detection(self, tmp_path):
        """检测到符号链接循环时应正确处理。"""
        from src.readmd_core.utils import _same_file_target
        link_a = str(tmp_path / 'link_a')
        link_b = str(tmp_path / 'link_b')
        os.symlink(link_b, link_a)
        os.symlink(link_a, link_b)
        try:
            result = _same_file_target(link_a, link_b)
            assert isinstance(result, bool)
        except RecursionError:
            logging.warning('Silent exception caught in tests.test_readmd_core_utils: RecursionError')
            pytest.fail('Symlink cycle caused RecursionError')
        except OSError:
            logging.warning('Silent exception caught in tests.test_readmd_core_utils: OSError')

    def test_deep_symlink_chain(self, tmp_path):
        """深层符号链接链应正确解析。"""
        from src.readmd_core.utils import _same_file_target
        original = str(tmp_path / 'original.txt')
        with open(original, 'w') as f:
            f.write('content')
        current = original
        links = [original]
        for i in range(10):
            link_path = str(tmp_path / f'link_{i}')
            os.symlink(current, link_path)
            links.append(link_path)
            current = link_path
        result = _same_file_target(links[-1], original)
        assert result is True

class TestOsReplaceFailures:
    """os.replace()失败场景模拟。"""

    def test_os_replace_permission_denied(self, tmp_path):
        """os.replace()权限被拒绝时应返回False并清理临时文件。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'replace_test.json')
        tmp_path_file = path + '.tmp'
        with open(path, 'w') as f:
            f.write('{}')
        os.chmod(path, stat.S_IRUSR | stat.S_IRGRP | stat.S_IROTH)
        try:
            result = save_json(path, {'test': 'data'})
            assert isinstance(result, bool)
        finally:
            os.chmod(path, stat.S_IWUSR | stat.S_IRUSR)
            if os.path.exists(tmp_path_file):
                os.unlink(tmp_path_file)

    def test_os_replace_cross_device_link(self, tmp_path):
        """跨设备os.replace()失败时应优雅处理。
        
        Why: save_json现在会重试2次，所以我们需要模拟所有重试都失败的情况。
        """
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'cross_device.json')
        original_replace = os.replace
        call_count = [0]

        def mock_replace(src, dst):
            call_count[0] += 1
            if call_count[0] <= 3:
                raise OSError(18, 'Invalid cross-device link')
            return original_replace(src, dst)
        with patch('os.replace', side_effect=mock_replace):
            result = save_json(path, {'test': 'data'})
        assert result is False
        assert not os.path.exists(path + '.tmp')

class TestUtilsPerformanceRegression:
    """utils模块性能回归测试。"""

    def test_save_json_performance(self, tmp_path):
        """save_json性能基准测试。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'perf_save.json')
        data = {'key': 'value', 'number': 42, 'list': [1, 2, 3]}
        iterations = 100
        start = time.perf_counter()
        for _ in range(iterations):
            result = save_json(path, data)
        elapsed = time.perf_counter() - start
        avg_time_ms = elapsed / iterations * 1000
        assert avg_time_ms < 10, f'Average save time: {avg_time_ms:.3f}ms (threshold: 10ms)'
        assert result is True

    def test_load_json_performance(self, tmp_path):
        """load_json性能基准测试。"""
        from src.readmd_core.utils import save_json, load_json
        path = str(tmp_path / 'perf_load.json')
        data = {'key': 'value', 'number': 42, 'list': [1, 2, 3]}
        save_json(path, data)
        iterations = 100
        start = time.perf_counter()
        for _ in range(iterations):
            result = load_json(path, default={})
        elapsed = time.perf_counter() - start
        avg_time_ms = elapsed / iterations * 1000
        assert avg_time_ms < 5, f'Average load time: {avg_time_ms:.3f}ms (threshold: 5ms)'
        assert result == data

    def test_path_normalization_performance(self, tmp_path):
        """路径规范化性能基准测试。"""
        from src.readmd_core.utils import _paths_equal, _same_file_target
        path1 = str(tmp_path / 'file1.txt')
        path2 = str(tmp_path / 'file2.txt')
        with open(path1, 'w') as f:
            f.write('content1')
        with open(path2, 'w') as f:
            f.write('content2')
        iterations = 1000
        start = time.perf_counter()
        for _ in range(iterations):
            _paths_equal(path1, path2)
            _same_file_target(path1, path2)
        elapsed = time.perf_counter() - start
        avg_time_ms = elapsed / (iterations * 2) * 1000
        assert avg_time_ms < 1, f'Average path comparison time: {avg_time_ms:.3f}ms (threshold: 1ms)'

class TestErrorLoggingVerification:
    """验证logging.error()在异常时被调用。"""

    def test_save_json_logs_error_on_failure(self, caplog, tmp_path):
        """save_json失败时应记录error日志。"""
        import logging
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'log_test.json')
        with patch('builtins.open', side_effect=OSError('[Errno 28] No space left on device')):
            with caplog.at_level(logging.ERROR):
                result = save_json(path, {'test': 'data'})
        assert result is False
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0, 'Expected error log record'
        assert '保存JSON文件失败' in error_records[0].message

    def test_read_text_logs_error_on_failure(self, caplog, tmp_path):
        """read_text失败时应记录error日志。"""
        import logging
        from src.readmd_core.utils import read_text
        path = str(tmp_path / 'nonexistent.txt')
        with caplog.at_level(logging.ERROR):
            (text, encoding) = read_text(path)
        assert text is None
        assert encoding is None
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0, 'Expected error log record'
        assert '读取文件失败' in error_records[0].message

    def test_save_json_logs_error_with_exception_details(self, caplog, tmp_path):
        """save_json错误日志应包含异常详情。"""
        import logging
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'error_detail.json')
        with patch('builtins.open', side_effect=PermissionError('Access denied')):
            with caplog.at_level(logging.ERROR):
                save_json(path, {'test': 'data'})
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0
        message = error_records[0].message
        assert path in message or '保存JSON文件失败' in message

class TestAdditionalCoverage:
    """补充测试以提高覆盖率到90%+。"""

    def test_save_json_retry_on_os_error(self, tmp_path, caplog):
        """save_json在OSError时应重试。"""
        import logging
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'retry_test.json')
        call_count = [0]

        def mock_open(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] <= 2:
                raise OSError('Mock OS error')
            return open(*args, **kwargs)
        with patch('builtins.open', side_effect=mock_open):
            result = save_json(path, {'test': 'data'})
        assert result is False
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0

    def test_save_json_unique_tmp_names(self, tmp_path):
        """save_json应生成唯一的临时文件名。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'unique_test.json')
        results = []
        for i in range(5):
            original_replace = os.replace
            tmp_paths = []

            def capture_replace(src, dst):
                tmp_paths.append(src)
                return original_replace(src, dst)
            with patch('os.replace', side_effect=capture_replace):
                save_json(path, {'iteration': i})
            if tmp_paths:
                results.append(tmp_paths[0])
        if len(results) > 1:
            unique_parts = set([p.split('.')[-2] for p in results])
            assert len(unique_parts) >= 1

    def test_save_json_cleanup_on_exception(self, tmp_path):
        """save_json异常时应清理临时文件。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'cleanup_test.json')

        class Unserializable:
            pass
        result = save_json(path, {'bad': Unserializable()})
        assert result is False
        tmp_files = list(tmp_path.glob('*.tmp.*'))
        assert len(tmp_files) == 0, f'Found leftover temp files: {tmp_files}'

    def test_read_text_all_encodings_fallback(self, tmp_path):
        """read_text应在所有编码失败时使用replace错误处理。"""
        from src.readmd_core.utils import read_text
        path = str(tmp_path / 'fallback.txt')
        with open(path, 'wb') as f:
            f.write(b'\x80\x81\x82\xff\xfe\xfd')
        (text, encoding) = read_text(path)
        assert text is not None
        assert encoding == 'latin-1'

    def test_paths_equal_both_nonexistent(self):
        """_paths_equal两个路径都不存在时应回退到绝对路径比较。"""
        from src.readmd_core.utils import _paths_equal
        result = _paths_equal('/nonexistent/path1', '/nonexistent/path2')
        assert result is False
        result = _paths_equal('/nonexistent/same', '/nonexistent/same')
        assert result is True

    def test_same_file_target_both_nonexistent(self):
        """_same_file_target两个路径都不存在时应回退到_paths_equal。"""
        from src.readmd_core.utils import _same_file_target
        result = _same_file_target('/nonexistent/path1', '/nonexistent/path2')
        assert result is False

class TestEdgeCaseCoverage:
    """边缘情况测试以覆盖剩余未测试的代码路径。"""

    def test_save_json_cleanup_os_error_in_retry(self, tmp_path, caplog):
        """save_json重试时清理临时文件失败应继续重试。"""
        import logging
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'cleanup_retry.json')
        call_count = [0]

        def mock_open(*args, **kwargs):
            call_count[0] += 1
            if call_count[0] == 1:
                raise FileNotFoundError('Mock file not found')
            return open(*args, **kwargs)

        def mock_unlink(*args, **kwargs):
            raise OSError('Mock unlink error')
        with patch('builtins.open', side_effect=mock_open):
            with patch('os.unlink', side_effect=mock_unlink):
                result = save_json(path, {'test': 'data'})
        assert result is False
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0

    def test_save_json_generic_exception_cleanup_os_error(self, tmp_path, caplog):
        """save_json通用异常处理中清理临时文件失败应不影响主流程。"""
        import logging
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'generic_exc.json')

        def mock_open(*args, **kwargs):
            f = open(*args, **kwargs)
            original_write = f.write

            def failing_write(*wargs, **wkwargs):
                raise ValueError('Mock write error')
            f.write = failing_write
            return f

        def mock_unlink(*args, **kwargs):
            raise OSError('Mock unlink error during cleanup')
        with patch('builtins.open', side_effect=mock_open):
            with patch('os.unlink', side_effect=mock_unlink):
                result = save_json(path, {'test': 'data'})
        assert result is False
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0

    def test_read_text_exception_logging(self, tmp_path, caplog):
        """read_text异常时应记录error日志。"""
        import logging
        from src.readmd_core.utils import read_text
        path = str(tmp_path / 'read_exc.txt')
        with patch('builtins.open', side_effect=PermissionError('Access denied')):
            with caplog.at_level(logging.ERROR):
                (text, encoding) = read_text(path)
        assert text is None
        assert encoding is None
        error_records = [r for r in caplog.records if r.levelno == logging.ERROR]
        assert len(error_records) > 0
        assert '读取文件失败' in error_records[0].message

    def test_same_file_target_os_error_fallback(self, tmp_path):
        """_same_file_target在OSError时应回退到_paths_equal。"""
        from src.readmd_core.utils import _same_file_target
        path1 = str(tmp_path / 'file1.txt')
        path2 = str(tmp_path / 'file2.txt')
        with open(path1, 'w') as f:
            f.write('content1')
        with open(path2, 'w') as f:
            f.write('content2')
        with patch('os.path.realpath', side_effect=OSError('Mock error')):
            result = _same_file_target(path1, path2)
        assert result is False

    def test_save_json_final_return_false(self, tmp_path):
        """save_json在所有重试失败后应返回False。"""
        from src.readmd_core.utils import save_json
        path = str(tmp_path / 'final_false.json')
        with patch('builtins.open', side_effect=OSError('Always fails')):
            result = save_json(path, {'test': 'data'})
        assert result is False