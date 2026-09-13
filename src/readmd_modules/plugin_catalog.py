"""Curated providers; a capability is the unit of exclusive activation."""
CAPABILITIES = {
    'ocr': ('rapidocr', 'easyocr'),
    'table': ('rapid_table',),
    'pdf': ('pymupdf4llm', 'docling'),
    'audio': ('whisper', 'faster_whisper'),
    'web': ('markdownify', 'trafilatura'),
    'document': ('pandoc_bridge',),
    'latex': ('pylatexenc',),
    'keywords': ('jieba',),
    'highlight': ('pygments',),
    'encoding': ('charset_normalizer',),
}


def extend_catalog(specs):
    additions = [
        ('pymupdf4llm', 'PyMuPDF4LLM', 'pymupdf4llm', 'pymupdf4llm', 'document', 'light', '~30MB'),
        ('docling', 'Docling', 'docling', 'docling', 'document', 'heavy', '~500MB+'),
        ('faster_whisper', 'Faster Whisper', 'faster-whisper', 'faster_whisper', 'audio', 'heavy', '~150MB+'),
        ('markdownify', 'Markdownify', 'markdownify', 'markdownify', 'document', 'light', '~2MB'),
        ('trafilatura', 'Trafilatura', 'trafilatura', 'trafilatura', 'document', 'light', '~10MB'),
        ('charset_normalizer', 'Charset Normalizer', 'charset-normalizer', 'charset_normalizer', 'text', 'light', '~1MB'),
    ]
    for pid, name, package, module, category, weight, size in additions:
        specs[pid] = dict(id=pid, name=name, name_key=f'plugin.{pid}.name',
                          desc_key=f'plugin.{pid}.desc', package=package,
                          pip_args=[package], import_name=module, category=category,
                          weight=weight, approx_size=size, cache_type='installed', cache_paths=[])
    # Includes the executable: installing only pypandoc leaves the bridge unusable.
    specs['pandoc_bridge'].update(package='pypandoc-binary', pip_args=['pypandoc-binary'], approx_size='~35MB')
    for capability, providers in CAPABILITIES.items():
        for pid in providers:
            specs[pid]['capability'] = capability
            specs[pid]['requires_model'] = pid in {'docling', 'easyocr', 'whisper', 'faster_whisper'}
            specs[pid]['name'] = specs[pid].get('name', pid.replace('_', ' ').title())
