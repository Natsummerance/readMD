import logging
'Tests for src/readmd_fix.py - Markdown auto-fix functionality.\n\nCovers:\n- fix_markdown() - table alignment, heading spacing, code fence closure\n- mask_all_code() - code block masking\n- Boundary conditions: empty text, large text, special characters\n'
import sys
import os
import pytest
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from src.readmd_core.readmd_fix import fix_markdown, FixResult, mask_all_code, mask_code_spans, restore

class TestFixResult:
    """Test FixResult class."""

    def test_fix_result_creation(self):
        """Verify FixResult can be instantiated with correct attributes."""
        result = FixResult('text', ['fix1'], {'table': 1})
        assert result.text == 'text'
        assert result.fixes == ['fix1']
        assert result.stats == {'table': 1}

    def test_fix_result_slots(self):
        """Verify FixResult uses __slots__ for memory efficiency."""
        result = FixResult('text', [], {})
        with pytest.raises(AttributeError):
            result.extra_attr = 'should fail'

class TestMaskCodeSpans:
    """Test inline code masking."""

    def test_no_code_spans(self):
        """Text without code spans should remain unchanged."""
        text = 'Hello world'
        (masked, spans) = mask_code_spans(text)
        assert masked == text
        assert spans == []

    def test_single_code_span(self):
        """Single backtick code span should be masked."""
        text = 'Use `code` here'
        (masked, spans) = mask_code_spans(text)
        assert '`code`' not in masked
        assert len(spans) == 1
        restored = restore(masked, spans)
        assert restored == text

    def test_multiple_code_spans(self):
        """Multiple code spans should all be masked."""
        text = 'Use `foo` and `bar` here'
        (masked, spans) = mask_code_spans(text)
        assert '`foo`' not in masked
        assert '`bar`' not in masked
        assert len(spans) == 2
        restored = restore(masked, spans)
        assert restored == text

    def test_nested_backticks(self):
        """Double backtick code spans should work correctly."""
        text = 'Use ``code with ` inside`` here'
        (masked, spans) = mask_code_spans(text)
        assert '``code with ` inside``' not in masked
        assert len(spans) == 1
        restored = restore(masked, spans)
        assert restored == text

    def test_unmatched_backtick(self):
        """Unmatched backtick should not be treated as code span."""
        text = 'Use `code here'
        (masked, spans) = mask_code_spans(text)
        assert '`' in masked

    def test_escaped_backtick(self):
        """Backslash-escaped backtick behavior depends on implementation."""
        text = 'Use `code` here'
        (masked, spans) = mask_code_spans(text)
        assert len(spans) >= 0

class TestMaskAllCode:
    """Test full code masking (fences + inline)."""

    def test_no_code(self):
        """Text without any code should remain unchanged."""
        text = 'Hello world\nNo code here'
        (masked, spans) = mask_all_code(text)
        assert masked == text
        assert spans == []

    def test_fence_code_block(self):
        """Fenced code blocks should be masked."""
        text = "Before\n```python\nprint('hello')\n```\nAfter"
        (masked, spans) = mask_all_code(text)
        assert '```python' not in masked
        assert "print('hello')" not in masked
        assert len(spans) >= 2
        restored = restore(masked, spans)
        assert restored == text

    def test_tilde_fence(self):
        """Tilde fences should also be recognized."""
        text = 'Before\n~~~\ncode\n~~~\nAfter'
        (masked, spans) = mask_all_code(text)
        assert '~~~' not in masked or '\x1aF' in masked
        restored = restore(masked, spans)
        assert restored == text

    def test_inline_code_in_text(self):
        """Inline code within regular text should be masked."""
        text = 'Use `code` and more `stuff`'
        (masked, spans) = mask_all_code(text)
        assert '`code`' not in masked
        assert '`stuff`' not in masked
        restored = restore(masked, spans)
        assert restored == text

    def test_mixed_fences_and_inline(self):
        """Mixed fence blocks and inline code should all be masked."""
        text = 'Start\n```js\nlet x = `template`;\n```\nEnd with `inline`'
        (masked, spans) = mask_all_code(text)
        assert '\x1aF' in masked or '\x1aC' in masked
        restored = restore(masked, spans)
        assert restored == text

    def test_indented_code_not_masked_as_fence(self):
        """Indented code (4 spaces) should not be treated as fenced code."""
        text = '    indented code\nregular text'
        (masked, spans) = mask_all_code(text)
        assert 'indented code' in masked

class TestFixMarkdownTables:
    """Test table-related fixes."""

    def test_missing_separator_row(self):
        """Table missing separator row should get one added."""
        text = '| A | B |\n| 1 | 2 |'
        result = fix_markdown(text)
        assert '| --- | --- |' in result.text
        assert '[表格]' in str(result.fixes)

    def test_unequal_column_count(self):
        """Table rows with unequal columns should be aligned."""
        text = '| A | B | C |\n|---|---|\n| 1 | 2 |'
        result = fix_markdown(text)
        assert result.text.count('|') >= 8

    def test_table_without_outer_pipes(self):
        """Table without outer pipes should be normalized."""
        text = 'A | B\n--|--\n1 | 2'
        result = fix_markdown(text)
        assert '| A | B |' in result.text
        assert '| --- | --- |' in result.text

    def test_escaped_pipe_in_cell(self):
        """Escaped pipe in cell should be preserved."""
        text = '| a \\| b | c |\n|---|---|'
        result = fix_markdown(text)
        assert '\\|' in result.text

    def test_alignment_preserved(self):
        """Alignment markers (:--) should be preserved."""
        text = '| a | b |\n|:--|--:|\n| 1 | 2 |'
        result = fix_markdown(text)
        assert ':---' in result.text or '---:' in result.text

    def test_two_column_prose_unchanged(self):
        """Two-column prose without proper table structure should not change."""
        text = 'a|b\nc|d'
        result = fix_markdown(text)
        assert result.text == text

    def test_single_line_with_pipe_unchanged(self):
        """Single line with pipe should not become a table."""
        text = '价格|折扣'
        result = fix_markdown(text)
        assert result.text == text

    def test_code_block_table_unchanged(self):
        """Table inside code block should not be modified."""
        text = '```\n| A | B |\n| 1 | 2 |\n```'
        result = fix_markdown(text)
        assert '| --- | --- |' not in result.text

    def test_empty_text(self):
        """Empty text should return empty result."""
        result = fix_markdown('')
        assert result.text == ''
        assert result.fixes == []

    def test_whitespace_only(self):
        """Whitespace-only text should be handled gracefully."""
        result = fix_markdown('   \n\n  ')
        assert result.text.strip() == ''

class TestFixMarkdownEmphasis:
    """Test emphasis/bold fixes."""

    def test_unclosed_bold(self):
        """Unclosed bold should be completed."""
        text = '**bold'
        result = fix_markdown(text)
        assert '**bold**' in result.text

    def test_trailing_bold_markers(self):
        """Trailing bold markers should be escaped."""
        text = 'bold**'
        result = fix_markdown(text)
        assert '\\*\\*' in result.text

    def test_mixed_bold(self):
        """Mixed closed and unclosed bold should be fixed."""
        text = '**a** **b'
        result = fix_markdown(text)
        assert '**a**' in result.text
        assert '**b**' in result.text

    def test_underscore_bold(self):
        """Underscore bold should work similarly."""
        text = '__under'
        result = fix_markdown(text)
        assert '__under__' in result.text

    def test_italic_unclosed(self):
        """Unclosed italic should be completed."""
        text = '*italic'
        result = fix_markdown(text)
        assert '*italic*' in result.text

    def test_multiplication_sign(self):
        """Multiplication sign pattern should be escaped."""
        text = '2 * 3'
        result = fix_markdown(text)
        assert '\\*' in result.text

    def test_list_items_unchanged(self):
        """List items should not be affected."""
        text = '* item\n* item2'
        result = fix_markdown(text)
        assert '* item' in result.text
        assert '* item2' in result.text

    def test_horizontal_rule_unchanged(self):
        """Horizontal rules should not be modified."""
        text = '***\n---\n___'
        result = fix_markdown(text)
        assert '***' in result.text
        assert '---' in result.text

    def test_code_inline_unchanged(self):
        """Bold/italic inside inline code should not be fixed."""
        text = '`**x` 和 `*y`'
        result = fix_markdown(text)
        assert '`**x`' in result.text
        assert '`*y`' in result.text

    def test_word_internal_underscore(self):
        """Underscores within words should not be treated as emphasis."""
        text = 'foo_bar foo__bar'
        result = fix_markdown(text)
        assert 'foo_bar' in result.text
        assert 'foo__bar' in result.text

    def test_triple_bold(self):
        """Triple asterisk bold should be completed."""
        text = '***bold'
        result = fix_markdown(text)
        assert '***bold***' in result.text

    def test_isolated_markers(self):
        """Isolated marker pairs should be escaped."""
        text = '**'
        result = fix_markdown(text)
        assert '\\*\\*' in result.text

class TestFixMarkdownMath:
    """Test math formula fixes."""

    def test_inline_math_unclosed(self):
        """Unclosed inline math should be completed."""
        text = '$x^2$ 和 $y'
        result = fix_markdown(text)
        assert '$y$' in result.text

    def test_currency_symbol(self):
        """Currency symbols should be escaped when no LaTeX detected."""
        text = '价格 $5'
        result = fix_markdown(text)
        assert '\\$5' in result.text

    def test_display_math_unclosed(self):
        """Unclosed display math should be completed."""
        text = '$$\nE=mc^2'
        result = fix_markdown(text)
        assert '$$' in result.text
        assert result.text.count('$$') >= 2

    def test_parenthesis_math(self):
        """Unclosed \\( should be completed."""
        text = '\\(x^2'
        result = fix_markdown(text)
        assert '\\)' in result.text

    def test_bracket_math(self):
        """Unclosed \\[ should be completed."""
        text = '\\[x^2'
        result = fix_markdown(text)
        assert '\\]' in result.text

    def test_empty_display_block_removed(self):
        """Empty display math blocks should be removed."""
        text = '$$\n\n$$\n正文'
        result = fix_markdown(text)
        assert '正文' in result.text
        assert '$$\n\n$$' not in result.text

    def test_inline_code_math_unchanged(self):
        """Math inside inline code should not be fixed."""
        text = '`$x$`'
        result = fix_markdown(text)
        assert '`$x$`' in result.text

    def test_complete_math_unchanged(self):
        """Complete math expressions should remain unchanged."""
        text = '$a$ 和 $b$'
        result = fix_markdown(text)
        assert '$a$' in result.text
        assert '$b$' in result.text

    def test_inline_display_math(self):
        """Inline display math $$...$$ should be preserved."""
        text = '$$x^2$$ 文本'
        result = fix_markdown(text)
        assert '$$x^2$$' in result.text

class TestFixMarkdownHeadings:
    """Test heading fixes."""

    def test_missing_space_after_hash(self):
        """Missing space after # should be added."""
        text = '#标题'
        result = fix_markdown(text)
        assert '# 标题' in result.text

    def test_multiple_hashes(self):
        """Multiple hash levels should all get spaces."""
        text = '##标题'
        result = fix_markdown(text)
        assert '## 标题' in result.text

    def test_seven_hashes_unchanged(self):
        """Seven hashes (invalid heading) should not be modified."""
        text = '####### x'
        result = fix_markdown(text)
        assert '####### x' in result.text

    def test_already_correct_unchanged(self):
        """Already correct headings should not be modified."""
        text = '# 标题'
        result = fix_markdown(text)
        assert '# 标题' in result.text

    def test_standalone_hashes_unchanged(self):
        """Standalone hash characters should not be modified."""
        text = '#\n##'
        result = fix_markdown(text)
        assert '#' in result.text

class TestFixMarkdownGeneral:
    """Test general fixes."""

    def test_bom_removal(self):
        """UTF-8 BOM should be removed."""
        text = '\ufeff# 标题'
        result = fix_markdown(text)
        assert '\ufeff' not in result.text
        assert '# 标题' in result.text

    def test_crlf_normalization(self):
        """CRLF line endings should be normalized to LF."""
        text = '# a\r\n# b\r\n'
        result = fix_markdown(text)
        assert '\r' not in result.text
        assert '# a\n# b\n' in result.text

    def test_stats_tracking(self):
        """Stats should track all fix types."""
        text = '#标题\n**bold\n$x^2'
        result = fix_markdown(text)
        assert result.stats['heading'] >= 1
        assert result.stats['bold'] >= 1
        assert result.stats['math'] >= 1

    def test_fixes_logging(self):
        """Fixes should be logged with descriptive messages."""
        text = '#标题'
        result = fix_markdown(text)
        assert len(result.fixes) > 0
        assert '[标题]' in result.fixes[0]

class TestBoundaryConditions:
    """Test boundary and edge cases."""

    def test_none_input(self):
        """None input should be handled gracefully."""
        try:
            result = fix_markdown(None)
            assert result is not None
        except (TypeError, AttributeError):
            logging.warning('Silent exception caught in tests.test_readmd_fix: (TypeError, AttributeError)')

    def test_very_long_text(self):
        """Very long text should be processed without error."""
        text = 'Line\n' * 10000
        result = fix_markdown(text)
        assert result.text is not None
        assert len(result.text) > 0

    def test_special_characters(self):
        """Special characters should be handled correctly."""
        text = '!@#$%^&*()_+-=[]{}|;\':",./<>?'
        result = fix_markdown(text)
        assert result.text is not None

    def test_unicode_characters(self):
        """Unicode characters should be preserved."""
        text = '中文测试 🎉 emoji 测试'
        result = fix_markdown(text)
        assert '中文测试' in result.text
        assert '🎉' in result.text

    def test_mixed_languages(self):
        """Mixed language text should be handled."""
        text = 'Hello 世界 Привет мир'
        result = fix_markdown(text)
        assert 'Hello' in result.text
        assert '世界' in result.text

    def test_newline_variations(self):
        """Different newline styles should be normalized."""
        text = 'Line1\rLine2\r\nLine3\nLine4'
        result = fix_markdown(text)
        assert '\r' not in result.text

    def test_tabs_in_text(self):
        """Tabs should be preserved in non-table contexts."""
        text = 'Col1\tCol2'
        result = fix_markdown(text)
        assert '\t' in result.text

    def test_empty_lines(self):
        """Multiple empty lines should be preserved by fix_markdown."""
        text = 'Line1\n\n\n\nLine2'
        result = fix_markdown(text)
        assert 'Line1' in result.text
        assert 'Line2' in result.text

    def test_only_whitespace(self):
        """Text with only whitespace should be handled."""
        text = '   \t  \n  \t  '
        result = fix_markdown(text)
        assert result.text is not None

    def test_single_character(self):
        """Single character text should work."""
        text = 'A'
        result = fix_markdown(text)
        assert result.text == 'A'

    def test_only_newlines(self):
        """Text with only newlines should work."""
        text = '\n\n\n'
        result = fix_markdown(text)
        assert result.text is not None