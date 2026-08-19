import os
import sys
import unittest
import json

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

import readmd


class TestV232Features(unittest.TestCase):

    def test_version_bumped_to_v232(self):
        self.assertEqual(readmd.VERSION, '2.3.2')

    def test_close_confirm_modal_assets_and_keys(self):
        root = os.path.dirname(os.path.dirname(__file__))
        index_path = os.path.join(root, 'assets', 'index.html')
        with open(index_path, 'r', encoding='utf-8') as f:
            html = f.read()
        self.assertIn('id="close-confirm-modal"', html)
        self.assertIn('id="close-confirm-box"', html)
        self.assertIn('id="close-confirm-title"', html)
        self.assertIn('id="close-confirm-desc"', html)
        self.assertIn('id="close-confirm-save"', html)
        self.assertIn('id="close-confirm-discard"', html)
        self.assertIn('id="close-confirm-cancel"', html)

        css_path = os.path.join(root, 'assets', 'style.css')
        with open(css_path, 'r', encoding='utf-8') as f:
            css = f.read()
        self.assertIn('#close-confirm-modal', css)
        self.assertIn('#close-confirm-box', css)
        self.assertIn('.close-confirm-icon-wrap', css)
        self.assertIn('.close-confirm-actions', css)

    def test_homepage_keys_in_all_languages(self):
        root = os.path.dirname(os.path.dirname(__file__))
        i18n_dir = os.path.join(root, 'assets', 'i18n')
        homepage_keys = [
            'app.slogan', 'app.open', 'app.folder', 'app.folderSub',
            'app.ai', 'app.convert', 'app.web', 'app.ocr',
            'app.ocrSub', 'app.recent', 'app.clearRecent', 'app.welcomeHint'
        ]
        for fname in os.listdir(i18n_dir):
            if not fname.endswith('.json') or fname in ['meta.json', '_meta.json']:
                continue
            fpath = os.path.join(i18n_dir, fname)
            with open(fpath, 'r', encoding='utf-8') as f:
                d = json.load(f)
            for k in homepage_keys:
                self.assertIn(k, d, f"Missing '{k}' in {fname}")
                self.assertTrue(len(d[k].strip()) > 0, f"Empty '{k}' in {fname}")


if __name__ == '__main__':
    unittest.main()
