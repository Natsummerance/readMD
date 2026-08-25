# -*- coding: utf-8 -*-
"""Presentation slides must not become an active-content bypass."""

import unittest

from src.readmd_modules.mdexport import presentation_render


class PresentationSanitizationTest(unittest.TestCase):
    def test_sanitizer_removes_active_content_and_preserves_safe_markdown(self):
        source = (
            '<script>window.__presentationXss = true;</script>\n\n'
            '<iframe src="https://example.invalid"></iframe>\n'
            '<img src="x" onerror="window.__presentationXss = true">\n'
            '<a href="javascript:alert(1)">bad link</a>\n\n'
            '**Safe bold**\n\n'
        )

        clean = presentation_render._sanitize_slide_html(source)

        self.assertNotIn('<script', clean)
        self.assertNotIn('onerror=', clean)
        self.assertNotIn('javascript:', clean)
        self.assertNotIn('<iframe', clean)
        self.assertIn('**Safe bold**', clean)

    def test_sanitizer_preserves_scripts_inside_inline_code(self):
        clean = presentation_render._sanitize_slide_html(
            'Use `<script>sample</script>` carefully.'
        )

        # Inline code is protected from the HTML parser and remains a sample.
        self.assertIn('`<script>sample</script>`', clean)

    def test_generated_presentation_removes_active_slide_content(self):
        markdown = (
            '# Safe slide\n\n'
            '<script>window.__presentationXss = true;</script>\n\n'
            '<img src="x" onerror="window.__presentationXss = true">\n\n'
            '<a href="javascript:alert(1)">bad</a>'
        )
        html = presentation_render.render_presentation_html(markdown)

        self.assertIn('# Safe slide', html)
        self.assertNotIn('<script>window.__presentationXss', html)
        self.assertNotIn('onerror=', html)
        self.assertNotIn('javascript:', html)


if __name__ == '__main__':
    unittest.main()
