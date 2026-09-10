from src.readmd_modules.import_processor import ImportProcessor
from src.readmd_modules.import_processor import MAX_IMPORT_DEPTH

def test_reproduce_bug_005(tmp_path):
    processor = ImportProcessor(str(tmp_path))
    assert processor.process("# 保留正文", depth=MAX_IMPORT_DEPTH) == "# 保留正文"
