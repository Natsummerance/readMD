from src.readmd_modules.import_processor import csv_to_markdown_table

def test_reproduce_bug_008():
    out = csv_to_markdown_table('name\nAlice,IMPORTANT_VALUE\n')
    assert 'Alice' in out
    assert 'IMPORTANT_VALUE' in out
