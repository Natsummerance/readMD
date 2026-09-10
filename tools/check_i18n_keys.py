import os
import json
import re

base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
i18n_dir = os.path.join(base_dir, "assets", "i18n")
html_path = os.path.join(base_dir, "assets", "index.html")

en_dict = json.load(open(os.path.join(i18n_dir, "en.json"), "r", encoding="utf-8"))
zh_dict = json.load(open(os.path.join(i18n_dir, "zh-CN.json"), "r", encoding="utf-8"))
html_content = open(html_path, "r", encoding="utf-8").read()

pattern = re.compile(r'data-i18n(?:-title|-placeholder|-aria|-html)?="([^"]+)"')
matches = pattern.findall(html_content)

en_keys = set(en_dict.keys())
missing_in_en = [k for k in matches if k not in en_keys]
print("Found", len(matches), "data-i18n tags in HTML.")
print("Missing in en.json:", set(missing_in_en))

# Check 46 languages parity
all_json_files = [f for f in os.listdir(i18n_dir) if f.endswith(".json") and f != "meta.json"]
print("Total language files:", len(all_json_files))
missing_per_lang = {}
for fname in all_json_files:
    d = json.load(open(os.path.join(i18n_dir, fname), "r", encoding="utf-8"))
    diff = en_keys - set(d.keys())
    if diff:
        missing_per_lang[fname] = len(diff)

if missing_per_lang:
    print("Files with missing keys:", missing_per_lang)
else:
    print("All language files have equal keys to en.json!")
