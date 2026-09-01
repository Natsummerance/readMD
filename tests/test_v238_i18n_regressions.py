import json
from pathlib import Path


I18N_DIR = Path(__file__).parents[1] / "assets" / "i18n"


def test_save_as_is_localized_in_every_non_english_locale():
    english = json.loads((I18N_DIR / "en.json").read_text(encoding="utf-8"))
    for locale_path in I18N_DIR.glob("*.json"):
        if locale_path.stem in {"en", "meta"}:
            continue
        locale = json.loads(locale_path.read_text(encoding="utf-8"))
        assert locale.get("menu.saveAs"), locale_path.stem
        assert locale.get("menu.saveAsSub"), locale_path.stem
        assert locale["menu.saveAs"] != english["menu.saveAs"], locale_path.stem
        assert locale["menu.saveAsSub"] != english["menu.saveAsSub"], locale_path.stem
