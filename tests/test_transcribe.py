# -*- coding: utf-8 -*-
"""Tests for audio/video transcription module in ReadMD."""

import os
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if ROOT not in sys.path:
    sys.path.insert(0, ROOT)

from src.readmd_modules import transcribe


class TestTranscribe(unittest.TestCase):
    """Test audio/video transcription and timestamp formatting."""

    def test_format_timestamp(self):
        self.assertEqual(transcribe.format_timestamp(5), "[00:05]")
        self.assertEqual(transcribe.format_timestamp(75), "[01:15]")
        self.assertEqual(transcribe.format_timestamp(3665), "[01:01:05]")

    def test_format_segments_to_markdown(self):
        segments = [
            {'start': 0.5, 'end': 3.2, 'text': ' Welcome to ReadMD tutorial.'},
            {'start': 3.5, 'end': 8.0, 'text': ' Today we learn markdown shortcuts.'},
        ]
        md = transcribe.format_segments(segments, title="tutorial.mp4")
        self.assertIn("# 音频/视频转写：tutorial.mp4", md)
        self.assertIn("**[00:00]** Welcome to ReadMD tutorial.", md)
        self.assertIn("**[00:03]** Today we learn markdown shortcuts.", md)

    def test_supported_extensions(self):
        self.assertTrue(transcribe.is_supported_media("meeting.mp3"))
        self.assertTrue(transcribe.is_supported_media("recording.wav"))
        self.assertTrue(transcribe.is_supported_media("video.mp4"))
        self.assertTrue(transcribe.is_supported_media("lecture.m4a"))
        self.assertFalse(transcribe.is_supported_media("document.pdf"))
        self.assertFalse(transcribe.is_supported_media("notes.md"))

    def test_transcribe_mock_whisper_engine(self):
        fake_model = MagicMock()
        fake_model.transcribe.return_value = {
            'text': 'Hello world test speech.',
            'segments': [
                {'start': 1.0, 'end': 4.0, 'text': ' Hello world test speech.'}
            ]
        }

        with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as f:
            f.write(b"fake audio data")
            tmp_path = f.name

        try:
            with patch('src.readmd_modules.transcribe.check_transcribe_prerequisites', return_value=(True, True)), \
                 patch('src.readmd_modules.transcribe._get_whisper_model', return_value=fake_model):
                md_text, err = transcribe.transcribe_to_md(tmp_path)
                self.assertIsNone(err)
                self.assertIn("**[00:01]** Hello world test speech.", md_text)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_transcribe_fallback_when_no_model(self):
        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
            f.write(b"fake wav data")
            tmp_path = f.name

        try:
            with patch('src.readmd_modules.transcribe.check_transcribe_prerequisites', return_value=(False, False)):
                md_text, err = transcribe.transcribe_to_md(tmp_path)
                self.assertIsNotNone(err)
                self.assertIn("未检测到语音转写模型", md_text)
                self.assertIn("pip install openai-whisper", md_text)
                self.assertIn("FFmpeg", md_text)
                self.assertIn(os.path.basename(tmp_path), md_text)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_format_segments_yaml_frontmatter(self):
        segments = [{'start': 1.0, 'end': 4.0, 'text': 'Test voice content.'}]
        md = transcribe.format_segments(
            segments,
            title="demo.mp3",
            language="zh",
            duration=125.0,
            file_format="mp3",
            model_name="whisper-base"
        )
        self.assertTrue(md.startswith("---\n"))
        self.assertIn('title: "demo.mp3"', md)
        self.assertIn('format: "mp3"', md)
        self.assertIn('duration: "02:05"', md)
        self.assertIn('model: "whisper-base"', md)
        self.assertIn('language: "zh"', md)
        self.assertIn("**[00:01]** Test voice content.", md)

    def test_convert_verbose_av_missing_plugin_is_failure(self):
        from src.readmd_modules import convert
        with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as f:
            f.write(b"fake mp3 audio")
            tmp_path = f.name

        try:
            with patch('src.readmd_modules.transcribe.check_transcribe_prerequisites', return_value=(False, False)):
                text, engine, err = convert.convert_verbose(tmp_path)
                self.assertIsNotNone(err)
                self.assertEqual(engine, 'whisper')
                self.assertEqual(text, '')
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_api_transcribe_file(self):
        from readmd import Api
        api = Api()
        res_bad = api.transcribe_file("invalid.pdf")
        self.assertFalse(res_bad.get('ok'))
        self.assertEqual(res_bad.get('error'), 'unsupported_media_format')


if __name__ == "__main__":
    unittest.main()
