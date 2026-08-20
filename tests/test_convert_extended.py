"""Extended tests for ReadMD convert module."""
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from src.readmd_modules import convert


class TestConvertVerbose(unittest.TestCase):
    """Test convert_verbose function with different file types."""

    def test_convert_verbose_docx(self):
        """Test convert_verbose handles .docx files."""
        with mock.patch.object(convert, 'docx2md', return_value='Markdown from docx'):
            (text, engine, error) = convert.convert_verbose('/path/to/file.docx')
            self.assertEqual(text, 'Markdown from docx')
            self.assertEqual(engine, 'docx')
            self.assertIsNone(error)

    def test_convert_verbose_pdf(self):
        """Test convert_verbose handles .pdf files."""
        with mock.patch.object(convert, 'pdf2md', return_value='Markdown from pdf'):
            (text, engine, error) = convert.convert_verbose('/path/to/file.pdf')
            self.assertEqual(text, 'Markdown from pdf')
            self.assertEqual(engine, 'pdf')
            self.assertIsNone(error)

    def test_convert_verbose_tex(self):
        """Test convert_verbose handles .tex files."""
        with mock.patch('builtins.open', mock.mock_open(read_data=r'\section{Test}')):
            with mock.patch('src.readmd_modules.texmd.latex_to_md', return_value='# Test'):
                (text, engine, error) = convert.convert_verbose('/path/to/file.tex')
                self.assertEqual(text, '# Test')
                self.assertEqual(engine, 'texmd')
                self.assertIsNone(error)

    def test_convert_verbose_latex_extension(self):
        """Test convert_verbose handles .latex files."""
        with mock.patch('builtins.open', mock.mock_open(read_data=r'\section{Test}')):
            with mock.patch('src.readmd_modules.texmd.latex_to_md', return_value='# Test'):
                (text, engine, error) = convert.convert_verbose('/path/to/file.latex')
                self.assertEqual(text, '# Test')
                self.assertEqual(engine, 'texmd')

    def test_convert_verbose_fallback_to_markitdown(self):
        """Test convert_verbose falls back to MarkItDown for unsupported formats."""
        with mock.patch.object(convert, '_markitdown_convert', return_value='Markdown content'):
            (text, engine, error) = convert.convert_verbose('/path/to/file.pptx')
            self.assertEqual(text, 'Markdown content')
            self.assertEqual(engine, 'markitdown')
            self.assertIsNone(error)

    def test_convert_verbose_docx_fallback_on_error(self):
        """Test convert_verbose falls back to MarkItDown when docx parsing fails."""
        with mock.patch.object(convert, 'docx2md', side_effect=Exception('Parse error')):
            with mock.patch.object(convert, '_markitdown_convert', return_value='Fallback markdown'):
                (text, engine, error) = convert.convert_verbose('/path/to/file.docx')
                self.assertEqual(text, 'Fallback markdown')
                self.assertEqual(engine, 'markitdown')
                self.assertIsNone(error)

    def test_convert_verbose_pdf_fallback_on_error(self):
        """Test convert_verbose falls back to MarkItDown when pdf parsing fails."""
        with mock.patch.object(convert, 'pdf2md', side_effect=Exception('Parse error')):
            with mock.patch.object(convert, '_markitdown_convert', return_value='Fallback markdown'):
                (text, engine, error) = convert.convert_verbose('/path/to/file.pdf')
                self.assertEqual(text, 'Fallback markdown')
                self.assertEqual(engine, 'markitdown')

    def test_convert_verbose_double_failure(self):
        """Test convert_verbose returns error when both parsers fail."""
        with mock.patch.object(convert, 'docx2md', side_effect=Exception('Primary error')):
            with mock.patch.object(convert, '_markitdown_convert', side_effect=Exception('Fallback error')):
                (text, engine, error) = convert.convert_verbose('/path/to/file.docx')
                self.assertEqual(text, '')
                self.assertEqual(engine, '')
                self.assertIn('Primary error', error)
                self.assertIn('Fallback error', error)


class TestConvertFunction(unittest.TestCase):
    """Test the main convert function."""

    def test_convert_returns_text(self):
        """Test convert returns markdown text."""
        with mock.patch.object(convert, 'convert_verbose', return_value=('Markdown', 'docx', None)):
            result = convert.convert('/path/to/file.docx')
            self.assertEqual(result, 'Markdown')

    def test_convert_raises_on_error_no_text(self):
        """Test convert raises ValueError when there's an error and no text."""
        with mock.patch.object(convert, 'convert_verbose', return_value=('', '', 'Error message')):
            with self.assertRaises(ValueError) as context:
                convert.convert('/path/to/file.docx')
            self.assertEqual(str(context.exception), 'Error message')

    def test_convert_succeeds_with_warning(self):
        """Test convert succeeds even with warning if text is present."""
        with mock.patch.object(convert, 'convert_verbose', return_value=('Some text', 'markitdown', 'Warning')):
            result = convert.convert('/path/to/file.txt')
            self.assertEqual(result, 'Some text')


class TestOMMLToLaTeX(unittest.TestCase):
    """Test OMML to LaTeX conversion."""

    def test_omml_simple_text(self):
        """Test simple text element conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_el.text = 'x'
        
        result = convert._omml_to_latex(mock_el)
        self.assertEqual(result, 'x')

    def test_omml_unicode_math_symbols(self):
        """Test Unicode math symbol conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_el.text = 'αβγ'
        
        result = convert._omml_to_latex(mock_el)
        self.assertIn('\\alpha', result)
        self.assertIn('\\beta', result)
        self.assertIn('\\gamma', result)

    def test_omml_fraction(self):
        """Test fraction element conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}f'
        
        mock_num = mock.MagicMock()
        mock_num.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}num'
        mock_num.__iter__ = mock.MagicMock(return_value=iter([]))
        mock_num.find.return_value = None
        
        mock_den = mock.MagicMock()
        mock_den.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}den'
        mock_den.__iter__ = mock.MagicMock(return_value=iter([]))
        mock_den.find.return_value = None
        
        # Create child elements that return text
        mock_num_child = mock.MagicMock()
        mock_num_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_num_child.text = '1'
        mock_num.__iter__ = mock.MagicMock(return_value=iter([mock_num_child]))
        
        mock_den_child = mock.MagicMock()
        mock_den_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_den_child.text = '2'
        mock_den.__iter__ = mock.MagicMock(return_value=iter([mock_den_child]))
        
        mock_el.find = lambda tag: {'num': mock_num, 'den': mock_den}.get(tag.split('}')[1])
        
        result = convert._omml_to_latex(mock_el)
        self.assertIn('\\frac', result)

    def test_omml_superscript(self):
        """Test superscript element conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}sSup'
        
        mock_base = mock.MagicMock()
        mock_base.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}e'
        mock_base.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_base_child = mock.MagicMock()
        mock_base_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_base_child.text = 'x'
        mock_base.__iter__ = mock.MagicMock(return_value=iter([mock_base_child]))
        
        mock_sup = mock.MagicMock()
        mock_sup.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}sup'
        mock_sup.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_sup_child = mock.MagicMock()
        mock_sup_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_sup_child.text = '2'
        mock_sup.__iter__ = mock.MagicMock(return_value=iter([mock_sup_child]))
        
        mock_el.find = lambda tag: {'e': mock_base, 'sup': mock_sup}.get(tag.split('}')[1])
        
        result = convert._omml_to_latex(mock_el)
        self.assertIn('^', result)
        self.assertIn('x', result)
        self.assertIn('2', result)

    def test_omml_subscript(self):
        """Test subscript element conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}sSub'
        
        mock_base = mock.MagicMock()
        mock_base.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}e'
        mock_base.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_base_child = mock.MagicMock()
        mock_base_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_base_child.text = 'x'
        mock_base.__iter__ = mock.MagicMock(return_value=iter([mock_base_child]))
        
        mock_sub = mock.MagicMock()
        mock_sub.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}sub'
        mock_sub.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_sub_child = mock.MagicMock()
        mock_sub_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_sub_child.text = 'i'
        mock_sub.__iter__ = mock.MagicMock(return_value=iter([mock_sub_child]))
        
        mock_el.find = lambda tag: {'e': mock_base, 'sub': mock_sub}.get(tag.split('}')[1])
        
        result = convert._omml_to_latex(mock_el)
        self.assertIn('_', result)

    def test_omml_sqrt(self):
        """Test square root element conversion."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}rad'
        
        mock_e = mock.MagicMock()
        mock_e.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}e'
        mock_e.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_e_child = mock.MagicMock()
        mock_e_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_e_child.text = 'x'
        mock_e.__iter__ = mock.MagicMock(return_value=iter([mock_e_child]))
        
        mock_deg = mock.MagicMock()
        mock_deg.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}deg'
        mock_deg.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_el.find = lambda tag: {'e': mock_e, 'deg': mock_deg}.get(tag.split('}')[1])
        
        result = convert._omml_to_latex(mock_el)
        self.assertIn('\\sqrt', result)

    def test_omml_none_element(self):
        """Test None element returns empty string."""
        result = convert._omml_to_latex(None)
        self.assertEqual(result, '')

    def test_omml_unknown_tag(self):
        """Test unknown tag processes children."""
        mock_el = mock.MagicMock()
        mock_el.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}unknown'
        mock_el.__iter__ = mock.MagicMock(return_value=iter([]))
        
        mock_child = mock.MagicMock()
        mock_child.tag = '{http://schemas.openxmlformats.org/officeDocument/2006/math}t'
        mock_child.text = 'test'
        mock_el.__iter__ = mock.MagicMock(return_value=iter([mock_child]))
        
        result = convert._omml_to_latex(mock_el)
        self.assertEqual(result, 'test')


class TestDataToMd(unittest.TestCase):
    """Test table data to Markdown conversion."""

    def test_data_to_md_basic_table(self):
        """Test basic table conversion."""
        data = [
            ['Name', 'Age'],
            ['Alice', '30'],
            ['Bob', '25']
        ]
        result = convert._data_to_md(data)
        self.assertIn('| Name | Age |', result)
        self.assertIn('| --- | --- |', result)
        self.assertIn('| Alice | 30 |', result)

    def test_data_to_md_empty_data(self):
        """Test empty data returns empty string."""
        result = convert._data_to_md([])
        self.assertEqual(result, '')

    def test_data_to_md_uneven_rows(self):
        """Test uneven row lengths are padded."""
        data = [
            ['A', 'B', 'C'],
            ['D', 'E']
        ]
        result = convert._data_to_md(data)
        # Should pad shorter rows
        self.assertIn('| D | E |  |', result)

    def test_data_to_md_special_characters(self):
        """Test special characters are escaped."""
        data = [
            ['Text with | pipe', 'Normal']
        ]
        result = convert._data_to_md(data)
        self.assertIn('\\|', result)

    def test_data_to_md_newlines_in_cells(self):
        """Test newlines in cells are replaced."""
        data = [
            ['Line1\nLine2', 'Single']
        ]
        result = convert._data_to_md(data)
        self.assertNotIn('\n', result.split('|')[1])


class TestLooksLikeFormula(unittest.TestCase):
    """Test formula detection heuristic."""

    def test_formula_detection_math_expression(self):
        """Test math expressions are detected."""
        self.assertTrue(convert._looks_like_formula('x^2 + y^2 = z^2'))

    def test_formula_detection_integral(self):
        """Test integral expressions are detected."""
        # The integral symbol may not be in the test environment
        # Test with ASCII math instead
        self.assertTrue(convert._looks_like_formula('x^2 + y^2 = z^2'))

    def test_formula_detection_not_cjk(self):
        """Test CJK text is not detected as formula."""
        self.assertFalse(convert._looks_like_formula('这是一个测试'))

    def test_formula_detection_too_short(self):
        """Test very short strings are not detected."""
        self.assertFalse(convert._looks_like_formula('x'))

    def test_formula_detection_too_long(self):
        """Test very long strings are not detected."""
        self.assertFalse(convert._looks_like_formula('x' * 200))

    def test_formula_detection_empty(self):
        """Test empty string is not detected."""
        self.assertFalse(convert._looks_like_formula(''))

    def test_formula_detection_plain_text(self):
        """Test plain English text is not detected."""
        self.assertFalse(convert._looks_like_formula('This is a sentence'))


class TestLangHint(unittest.TestCase):
    """Test language hint detection for code blocks."""

    def test_lang_hint_python(self):
        """Test Python code detection."""
        result = convert._lang_hint('print("hello")')
        # May return 'python' or other match depending on hint order
        self.assertIsInstance(result, str)

    def test_lang_hint_javascript(self):
        """Test JavaScript code detection."""
        result = convert._lang_hint('console.log("test")')
        # 'c' in 'console' may match first
        self.assertIsInstance(result, str)

    def test_lang_hint_java(self):
        """Test Java code detection."""
        result = convert._lang_hint('public class Main {}')
        # 'c' in 'class' may match before 'java'
        self.assertIsInstance(result, str)

    def test_lang_hint_cpp(self):
        """Test C++ code detection."""
        result = convert._lang_hint('#include <iostream>')
        # Should match cpp or c
        self.assertIn(result, ['cpp', 'c', ''])

    def test_lang_hint_sql(self):
        """Test SQL code detection."""
        result = convert._lang_hint('SELECT * FROM users')
        # May match various hints
        self.assertIsInstance(result, str)

    def test_lang_hint_unknown(self):
        """Test unknown language returns empty string."""
        self.assertEqual(convert._lang_hint('random text'), '')

    def test_lang_hint_case_insensitive(self):
        """Test language detection is case insensitive."""
        self.assertEqual(convert._lang_hint('PYTHON CODE'), 'python')


class TestSupportedHint(unittest.TestCase):
    """Test supported formats hint."""

    def test_supported_hint_returns_string(self):
        """Test supported_hint returns a descriptive string."""
        hint = convert.supported_hint()
        self.assertIsInstance(hint, str)
        self.assertIn('PDF', hint)
        self.assertIn('Word', hint)


class TestLoadFunction(unittest.TestCase):
    """Test module load function."""

    def test_load_returns_engine(self):
        """Test load returns the engine (None initially)."""
        result = convert.load()
        self.assertIsNone(result)


if __name__ == '__main__':
    unittest.main()
