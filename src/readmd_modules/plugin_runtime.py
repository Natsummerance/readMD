"""Lazy adapters used by ReadMD's actual conversion and export pipelines."""
import os
from . import plugin_manager as pm


def convert_file(path):
    extension = os.path.splitext(path)[1].lower()
    capability = ('pdf' if extension == '.pdf' else 'web' if extension in {'.html', '.htm', '.xhtml'}
                  else 'document' if extension in {'.docx', '.epub', '.odt', '.rtf', '.rst', '.org', '.fb2'} else None)
    pid = pm.active_provider(capability) if capability else None
    if not pid:
        return None

    def execute():
        if pid == 'docling':
            from docling.document_converter import DocumentConverter
            return DocumentConverter().convert(path).document.export_to_markdown()
        if pid == 'pymupdf4llm':
            import pymupdf4llm
            return pymupdf4llm.to_markdown(path, embed_images=True, show_progress=False)
        if pid == 'pandoc_bridge':
            import pypandoc
            return pypandoc.convert_file(path, 'gfm', extra_args=['--wrap=none'])
        from .txtmd import read_text
        source, _ = read_text(path)
        if pid == 'markdownify':
            from markdownify import markdownify
            return markdownify(source, heading_style='ATX')
        import trafilatura
        return trafilatura.extract(source, output_format='markdown', include_tables=True, include_links=True, include_images=True)

    text = pm.run_plugin(pid, execute)
    if isinstance(text, str) and text.strip():
        return text.strip() + '\n', pid, None
    return None


def decode_text(data):
    def execute():
        from charset_normalizer import from_bytes
        match = from_bytes(data).best()
        return (str(match), match.encoding) if match and match.percent_chaos < 20 else None
    return pm.run_plugin('charset_normalizer', execute)


def latex_label(text):
    def execute():
        from pylatexenc.latex2text import LatexNodes2Text
        return LatexNodes2Text().latex_to_text(text)
    return pm.run_plugin('pylatexenc', execute, default=text)


def keywords(text):
    def execute():
        import jieba.analyse
        return ', '.join(jieba.analyse.extract_tags(text[:100000], topK=8))
    return pm.run_plugin('jieba', execute, default='')


def highlighted_blocks(text):
    def execute():
        from pygments import highlight
        from pygments.lexers import get_lexer_by_name, TextLexer
        from pygments.formatters import HtmlFormatter
        from .mdexport.parser import parse
        result = {}
        for block in parse(text):
            if block['type'] != 'code': continue
            try: lexer = get_lexer_by_name(block.get('lang', '') or 'text')
            except Exception: lexer = TextLexer()
            result[block['content'].rstrip('\n')] = highlight(block['content'], lexer, HtmlFormatter(noclasses=True, nowrap=True))
        return result
    return pm.run_plugin('pygments', execute, default={})


def faster_transcribe(path, model_name, language=None):
    def execute():
        from faster_whisper import WhisperModel
        model = WhisperModel(model_name, device='cpu', compute_type='int8')
        segments, info = model.transcribe(path, language=language or None)
        return {'segments': [{'start': s.start, 'end': s.end, 'text': s.text} for s in segments],
                'language': info.language, 'duration': info.duration}
    return pm.run_plugin('faster_whisper', execute)
