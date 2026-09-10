from src.readmd_modules.import_processor import ImportProcessor

def test_reproduce_bug_007(tmp_path):
    (tmp_path / "a.puml").write_text("@startuml\n```\n# not a heading\n@enduml", encoding="utf-8")
    result = ImportProcessor(str(tmp_path)).process('@import "a.puml"')
    assert result.startswith("````puml\n")
    assert result.endswith("\n````")
