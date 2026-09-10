# X2Knowledge 强于 ReadMD 的能力 & 集成方案

> 生成时间：2026-09-04
> 目标：把 X2Knowledge 的优势能力集成到 ReadMD，使 ReadMD 的解析能力全面超越 X2Knowledge

---

## 一、能力差距总览

| # | X2Knowledge 强项 | ReadMD 现状 | 集成难度 | 价值 |
|---|----------------|------------|:-------:|:----:|
| 1 | Docling 引擎（学术 PDF 解析） | 仅 MarkItDown | 中 | 高 |
| 2 | EasyOCR（复杂场景 OCR） | 仅 WinRT OCR | 中 | 高 |
| 3 | LaTeX 源码解析 | 不支持 | 低 | 中 |
| 4 | 音视频转文字 | 不支持 | 中 | 中 |
| 5 | 结构化预处理 API | 无 | 低 | 低（ReadMD 是桌面应用，不需要） |

---

## 二、逐项详解 & 集成方案

### 1. Docling 引擎 — 学术 PDF 解析

**X2Knowledge 怎么用的：**
X2Knowledge 集成了 IBM 开源的 Docling（`/api/convert-to-md-docling` 路由），对学术论文 PDF 中的复杂表格、数学公式、引用结构、多栏版面的提取效果显著优于 MarkItDown。

**ReadMD 现状：**
`convert.py` 只用 MarkItDown 一个引擎。对普通办公 PDF 够用，但遇到学术论文（多栏、嵌套表格、LaTeX 公式）效果差。

**集成方案：**

```bash
# 安装 Docling
pip install docling
```

在 `readmd_modules/convert.py` 中增加 Docling 作为 PDF 的备选引擎：

```python
# readmd_modules/convert.py

_engine = None
_docling = None


def load():
    global _engine
    if _engine is None:
        from markitdown import MarkItDown
        _engine = MarkItDown()
    return _engine


def _load_docling():
    """懒加载 Docling（首次调用时才初始化，避免拖慢启动）"""
    global _docling
    if _docling is None:
        try:
            from docling.document_converter import DocumentConverter
            _docling = DocumentConverter()
        except ImportError:
            return None  # 未安装则跳过
    return _docling


def convert(path):
    """把任意支持的文件转换为 Markdown 文本。"""
    import os
    ext = os.path.splitext(path)[1].lower()

    # PDF 文件优先尝试 Docling（学术效果更好），失败回退 MarkItDown
    if ext == '.pdf':
        docling = _load_docling()
        if docling is not None:
            try:
                result = docling.convert(path)
                text = result.document.export_to_markdown()
                if text and text.strip():
                    return text.strip()
            except Exception:
                pass  # Docling 失败，回退 MarkItDown

    # 默认走 MarkItDown
    eng = load()
    result = eng.convert(path)
    text = (result.text_content or '').strip()
    return text
```

**注意事项：**
- Docling 依赖较重（约 500MB+，含 PyTorch），建议做成可选依赖
- 未安装 Docling 时自动回退 MarkItDown，不影响现有功能
- 可以在 UI 上加一个「深度解析」按钮，让用户手动选择用 Docling

---

### 2. EasyOCR — 复杂场景 OCR

**X2Knowledge 怎么用的：**
安装了 EasyOCR（`pip install easyocr`），基于深度学习，支持 80+ 语言混合识别，对手写体、低质量图片、倾斜文字等复杂场景识别率远高于传统 OCR。

**ReadMD 现状：**
`ocr.py` 使用 Windows WinRT OCR（系统内置），速度快但对复杂场景（手写、模糊、多语言混合）识别率有限。且仅限 Windows。

**集成方案：**

```bash
# 安装 EasyOCR（需要 PyTorch）
pip install easyocr
```

在 `readmd_modules/ocr.py` 中增加 EasyOCR 作为备选引擎：

```python
# readmd_modules/ocr.py

import asyncio
import logging

_engine_cache = {}
_easyocr_reader = None


def load():
    _pick_language()
    return True


def _load_easyocr():
    """懒加载 EasyOCR（首次 OCR 时初始化）"""
    global _easyocr_reader
    if _easyocr_reader is None:
        try:
            import easyocr
            # 初始化时指定语言，GPU 可用会自动使用
            _easyocr_reader = easyocr.Reader(['ch_sim', 'en'], gpu=True)
        except ImportError:
            return None
    return _easyocr_reader


def _ocr_easyocr(image_path):
    """用 EasyOCR 识别图片（支持复杂场景）"""
    reader = _load_easyocr()
    if reader is None:
        return None
    try:
        results = reader.readtext(image_path)
        # results 格式: [(bbox, text, confidence), ...]
        lines = [text for _, text, conf in results if conf > 0.3]
        return '\n'.join(lines)
    except Exception as e:
        logging.warning('EasyOCR failed: %s', e)
        return None


# ===== 现有 WinRT OCR 函数保持不变 =====

def _ocr_bytes(data, lang_tag):
    # ... 原有代码 ...
    pass

def ocr_image(path, dpi=None):
    """识别图片：优先 EasyOCR（复杂场景更强），回退 WinRT OCR"""
    # 先尝试 EasyOCR
    text = _ocr_easyocr(path)
    if text and text.strip():
        return text

    # 回退 WinRT OCR
    with open(path, 'rb') as f:
        data = f.read()
    text = _ocr_bytes(data, _lang_tag()).strip()
    return text
```

**注意事项：**
- EasyOCR 首次运行会下载模型文件（约 100MB）
- 有 GPU 时自动用 GPU 加速，没有则用 CPU（会慢一些）
- 建议做成可选：UI 上加一个「深度 OCR」选项，普通 OCR 失败时提示用户尝试
- WinRT OCR 速度更快，对清晰文档优先用 WinRT；EasyOCR 作为兜底

---

### 3. LaTeX 源码解析

**X2Knowledge 怎么用的：**
安装了 `pylatexenc`，可以把 LaTeX 源码（`.tex` 文件）转换为纯文本或 Markdown。

**ReadMD 现状：**
不支持 `.tex` 文件。

**集成方案：**

```bash
pip install pylatexenc
```

在 `convert.py` 中增加 `.tex` 文件支持：

```python
def convert(path):
    import os
    ext = os.path.splitext(path)[1].lower()

    # LaTeX 文件
    if ext == '.tex':
        return _convert_latex(path)

    # ... 其余代码不变 ...


def _convert_latex(path):
    """LaTeX 源码转 Markdown"""
    try:
        from pylatexenc.latex2text import LatexNodes2Text
        with open(path, 'r', encoding='utf-8', errors='replace') as f:
            latex = f.read()
        text = LatexNodes2Text().latex_to_text(latex)
        return text.strip()
    except ImportError:
        # 未安装 pylatexenc，直接返回原始文本
        with open(path, 'r', encoding='utf-8', errors='replace') as f:
            return f.read()
    except Exception:
        with open(path, 'r', encoding='utf-8', errors='replace') as f:
            return f.read()
```

---

### 4. 音视频转文字

**X2Knowledge 怎么用的：**
安装了 `ffmpeg` + `pydub`，可以提取音视频文件中的音轨，配合 Whisper 等模型做语音转文字。

**ReadMD 现状：**
不支持音视频文件。

**集成方案：**

```bash
# 安装依赖
pip install openai-whisper   # 或 faster-whisper（更快）
# 系统需要安装 ffmpeg：https://ffmpeg.org/download.html
```

新建 `readmd_modules/transcribe.py`：

```python
# readmd_modules/transcribe.py
"""音视频转文字（Whisper）"""

import logging

_whisper_model = None


def load():
    """预加载 Whisper 模型（懒加载）"""
    global _whisper_model
    if _whisper_model is None:
        try:
            import whisper
            _whisper_model = whisper.load_model("base")  # base 模型，平衡速度和质量
        except ImportError:
            logging.warning('whisper 未安装，音视频转文字不可用')
            return False
    return True


def transcribe(path, language=None):
    """音视频文件转文字"""
    if not load():
        return None
    try:
        result = _whisper_model.transcribe(path, language=language)
        return result.get('text', '').strip()
    except Exception as e:
        logging.exception('transcribe failed: %s', path)
        return None


def transcribe_to_md(path, language=None):
    """音视频转 Markdown（带时间戳分段）"""
    if not load():
        return None
    try:
        result = _whisper_model.transcribe(path, language=language)
        segments = result.get('segments', [])
        if not segments:
            return result.get('text', '').strip()

        parts = []
        for seg in segments:
            start = seg.get('start', 0)
            text = seg.get('text', '').strip()
            if text:
                minutes = int(start // 60)
                seconds = int(start % 60)
                parts.append(f'**[{minutes:02d}:{seconds:02d}]** {text}')
        return '\n\n'.join(parts)
    except Exception as e:
        logging.exception('transcribe_to_md failed: %s', path)
        return None
```

在 `readmd_modules/__init__.py` 注册新模块：

```python
MODULES = ('convert', 'ocr', 'web', 'ai', 'transcribe')
```

在 `readmd.py` 的 `_route` 中增加 API：

```python
elif path == '/api/transcribe':
    p = unquote(qs.get('p', [''])[0])
    self._api_transcribe(p)

def _api_transcribe(self, p):
    if not os.path.isfile(p):
        self._send_json(404, {'error': '文件不存在'})
        return
    if not RM.is_ready('transcribe'):
        RM.load_all()
        self._send_json(409, {'error': '转写模块加载中，请稍候'})
        return
    try:
        mod = RM.get('transcribe')
        text = mod.transcribe_to_md(p)
        fr = readmd_fix.fix_markdown(text or '')
        self._send_json(200, {'content': fr.text, 'fixes': fr.fixes,
                              'name': os.path.basename(p),
                              'source': 'transcribe', 'path': p})
    except Exception as e:
        self._send_json(500, {'error': '转写失败：%s' % e})
```

**注意事项：**
- Whisper 模型分多档：`tiny`（最快）→ `base` → `small` → `medium` → `large`（最准）
- `base` 模型约 150MB，中文效果够用；需要更高精度可换 `small`（约 500MB）
- 也可以用 `faster-whisper` 替代，速度快 4 倍，内存占用更低
- 支持格式：mp3, wav, mp4, m4a, flac, ogg 等（ffmpeg 支持的都行）

---

## 三、集成优先级建议

| 优先级 | 能力 | 理由 |
|:------:|------|------|
| **P0** | Docling 引擎 | 学术 PDF 是最常见的「MarkItDown 搞不定」的场景，集成后提升最明显 |
| **P1** | EasyOCR | WinRT OCR 对清晰文档够用，但手写/模糊图片是痛点，EasyOCR 作为兜底很有价值 |
| **P2** | LaTeX 解析 | 依赖面窄（只有学术用户需要），但实现简单，5 分钟搞定 |
| **P3** | 音视频转文字 | 有用但依赖重（Whisper 模型大），且不是 ReadMD 的核心场景 |

---

## 四、一键安装脚本（建议加到 install.bat）

```bat
REM ===== 可选增强依赖（按需安装）=====

REM Docling（学术 PDF 深度解析）
REM pip install docling

REM EasyOCR（复杂场景 OCR 兜底）
REM pip install easyocr

REM LaTeX 解析
REM pip install pylatexenc

REM 音视频转文字（需要系统先装 ffmpeg）
REM pip install openai-whisper
```

建议保持可选，不强制安装——ReadMD 的核心优势是轻量秒开，依赖太重就违背初衷了。
可以在 UI 的「设置」里加一个「增强功能」面板，检测到未安装时提示用户按需安装。

---

## 五、集成后的能力矩阵

| 文件格式 | 集成前 | 集成后 |
|---------|:------:|:------:|
| PDF（文字版） | ✅ | ✅（+Docling 深度解析） |
| PDF（扫描版） | ✅ WinRT OCR | ✅ WinRT OCR + EasyOCR 兜底 |
| Word/PPT/Excel | ✅ | ✅ 不变 |
| 图片 OCR | ✅ WinRT | ✅ WinRT + EasyOCR |
| LaTeX (.tex) | ❌ | ✅ |
| 音视频转文字 | ❌ | ✅ Whisper |
| 网页抓取 | ✅ | ✅ 不变（X2K 没有） |
| 自动修正 | ✅ | ✅ 不变（X2K 没有） |

集成完成后，ReadMD 的解析能力将 **全面覆盖并超越 X2Knowledge**。
