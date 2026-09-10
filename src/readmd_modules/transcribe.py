# -*- coding: utf-8 -*-
"""ReadMD 音视频转文字模块（Whisper）。

支持多种音视频格式（MP3, WAV, M4A, MP4, FLAC, OGG, WEBM, AAC 等）提取音频并转写为带时间戳的 Markdown。
设计原则：
1. 懒加载：仅在收到转写请求时动态挂载插件沙箱并加载模型。
2. 双重检测：自动检测沙箱或系统中的 ffmpeg 及 whisper 库，缺失时给出准确诊断。
3. 容错友好：转写失败返回清晰错误提示，支持指定转写语言或自动检测。
"""

import logging
import os
from typing import Optional, Tuple
from . import plugin_manager as pm

SUPPORTED_AUDIO_VIDEO_EXTS = (
    '.mp3', '.wav', '.m4a', '.mp4', '.flac', '.ogg',
    '.webm', '.aac', '.wma', '.mkv', '.mov', '.avi'
)

_whisper_model_cache = {}


def load():
    """验证转写模块就绪。保持轻量秒开，不在此处加载重型模型。"""
    return True


def is_supported_media(path: str) -> bool:
    """判断文件是否为支持的音视频格式。"""
    ext = os.path.splitext(path)[1].lower()
    return ext in SUPPORTED_AUDIO_VIDEO_EXTS


def check_transcribe_prerequisites() -> Tuple[bool, bool]:
    """检查转写前置条件（whisper 库与 ffmpeg 二进制）。返回 (has_whisper, has_ffmpeg)。"""
    has_whisper = pm.is_plugin_enabled('whisper')
    ffmpeg_bin = pm.get_ffmpeg_path()
    has_ffmpeg = bool(ffmpeg_bin)
    return has_whisper, has_ffmpeg


def format_timestamp(seconds: float, brackets: bool = True) -> str:
    """将秒数格式化为 MM:SS 或 HH:MM:SS 时间戳。"""
    total_seconds = int(seconds)
    hours = total_seconds // 3600
    minutes = (total_seconds % 3600) // 60
    secs = total_seconds % 60
    if hours > 0:
        ts = f'{hours:02d}:{minutes:02d}:{secs:02d}'
    else:
        ts = f'{minutes:02d}:{secs:02d}'
    return f'[{ts}]' if brackets else ts


def format_segments(
    segments: list,
    title: Optional[str] = None,
    language: Optional[str] = None,
    duration: Optional[float] = None,
    file_format: Optional[str] = None,
    model_name: Optional[str] = None,
) -> str:
    """将 Whisper 识别段落格式化为 Markdown 格式（含 YAML 元数据与时间戳分段）。"""
    lines = []

    # 构造 YAML Frontmatter
    frontmatter = []
    if title:
        frontmatter.append(f'title: "{title}"')
    if file_format:
        frontmatter.append(f'format: "{file_format}"')
    if duration is not None and duration >= 0:
        duration_ts = format_timestamp(duration, brackets=False)
        frontmatter.append(f'duration: "{duration_ts}"')
    if model_name:
        frontmatter.append(f'model: "{model_name}"')
    if language:
        frontmatter.append(f'language: "{language}"')

    if frontmatter:
        lines.append('---')
        lines.extend(frontmatter)
        lines.append('---')

    if title:
        lines.append(f'# 音频/视频转写：{title}')
    if language:
        lines.append(f'> 识别语言：`{language}`')

    for seg in segments:
        start = seg.get('start', 0.0)
        text = (seg.get('text') or '').strip()
        if text:
            ts = format_timestamp(start, brackets=True)
            lines.append(f'**{ts}** {text}')

    has_valid_text = any(bool((seg.get('text') or '').strip()) for seg in segments)
    if not has_valid_text and title:
        lines.append('> （未识别到有效语音内容）')

    return '\n\n'.join(lines).strip() + '\n'


def _load_model(model_name: str = 'base'):
    """按需加载 Whisper 模型。"""
    if model_name in _whisper_model_cache:
        return _whisper_model_cache[model_name]

    pm.mount_sandbox()
    import whisper

    logging.info('Loading whisper model: %s', model_name)
    model = whisper.load_model(model_name)
    _whisper_model_cache[model_name] = model
    return model


_get_whisper_model = _load_model


def _make_whisper_notice(path: str, details: str = '未检测到语音转写模型或 FFmpeg 工具') -> str:
    """生成友好的插件安装指引 Markdown 提示文本（包含插件中心与手动 CLI 指引）。"""
    title = os.path.basename(path)
    ext = os.path.splitext(path)[1].lstrip('.').lower()
    return (
        f'---\n'
        f'title: "{title}"\n'
        f'format: "{ext}"\n'
        f'status: "unprocessed"\n'
        f'---\n\n'
        f'# 音频/视频转写：{title}\n\n'
        f'> **{details}**\n>\n'
        f'> **快速安装指引**：\n'
        f'> 1. **方式一（推荐）**：在 ReadMD 右上角打开「插件中心」，启用或一键安装 `whisper` 插件。\n'
        f'> 2. **方式二（手动 CLI 命令）**：\n'
        f'>    ```bash\n'
        f'>    pip install openai-whisper\n'
        f'>    ```\n'
        f'>    若系统缺少 FFmpeg，请运行对应命令安装并加入环境变量 PATH：\n'
        f'>    - **Windows**: `winget install Gyan.FFmpeg` 或从官网解压\n'
        f'>    - **macOS**: `brew install ffmpeg`\n'
        f'>    - **Linux**: `sudo apt install ffmpeg`\n'
    )


def transcribe_to_md(path: str, language: Optional[str] = None, model_name: str = 'base') -> Tuple[Optional[str], Optional[str]]:
    """将音视频文件转写为 Markdown 格式（带时间戳分段与元数据）。

    返回: (markdown_text, error_key_or_message)
    """
    if not os.path.isfile(path):
        return None, 'file_not_found'

    if not is_supported_media(path):
        return None, 'unsupported_media_format'

    has_whisper, has_ffmpeg = check_transcribe_prerequisites()
    if not has_whisper or not has_ffmpeg:
        return _make_whisper_notice(path, '未检测到语音转写模型或 FFmpeg 工具'), 'Whisper plugin or FFmpeg not available.'

    try:
        model = _get_whisper_model(model_name)
        if model is None:
            return _make_whisper_notice(path, '未检测到语音转写模型'), 'whisper_not_available'

        kwargs = {}
        if language and language.strip():
            kwargs['language'] = language.strip()

        result = model.transcribe(path, **kwargs)
        segments = result.get('segments', [])

        title = os.path.basename(path)
        ext = os.path.splitext(path)[1].lstrip('.').lower()
        lang = result.get('language') or language
        duration = result.get('duration')
        if duration is None and segments:
            duration = segments[-1].get('end')

        if not segments:
            text = (result.get('text') or '').strip()
            content = text if text else '> （未识别到有效语音内容）'
            fm_lines = ['---', f'title: "{title}"', f'format: "{ext}"']
            if model_name:
                fm_lines.append(f'model: "{model_name}"')
            if lang:
                fm_lines.append(f'language: "{lang}"')
            fm_lines.append('---')
            return f"{chr(10).join(fm_lines)}\n\n# 音频/视频转写：{title}\n\n{content}\n", None

        md_text = format_segments(
            segments,
            title=title,
            language=lang,
            duration=duration,
            file_format=ext,
            model_name=model_name,
        )
        return md_text, None
    except Exception as exc:
        logging.exception('transcribe_to_md failed for %s', path)
        return None, str(exc)
