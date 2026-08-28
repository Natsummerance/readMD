# -*- coding: utf-8 -*-
"""Unit tests for AI protocol streaming/non-streaming response contract and URL normalization."""

import unittest
import json
import os
from unittest.mock import patch, MagicMock
from src.readmd_modules.ai import _normalize_base_url, _chat_openai, _openai_usage, _is_local_provider


class TestAiProtocolAndUrlNormalization(unittest.TestCase):
    def test_pinned_provider_catalog_is_local_and_versioned(self):
        path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "assets", "providers", "provider-catalog.json")
        with open(path, encoding="utf-8") as handle:
            catalog = json.load(handle)
        self.assertEqual(catalog["license"], "MIT")
        self.assertRegex(catalog["upstream_commit"], r"^[0-9a-f]{40}$")
        self.assertGreaterEqual(len(catalog["providers"]), 20)

    def test_normalize_base_url_standard(self):
        url = _normalize_base_url("https://api.openai.com/v1", "chat/completions")
        self.assertEqual(url, "https://api.openai.com/v1/chat/completions")

    def test_normalize_base_url_trailing_slash(self):
        url = _normalize_base_url("https://api.openai.com/v1/", "chat/completions")
        self.assertEqual(url, "https://api.openai.com/v1/chat/completions")

    def test_normalize_base_url_duplicate_endpoint(self):
        url = _normalize_base_url("https://api.opencodezen.com/v1/chat/completions", "chat/completions")
        self.assertEqual(url, "https://api.opencodezen.com/v1/chat/completions")

    def test_normalize_base_url_duplicate_completions(self):
        url = _normalize_base_url("https://api.opencodezen.com/v1/completions", "completions")
        self.assertEqual(url, "https://api.opencodezen.com/v1/completions")

    def test_normalize_base_url_duplicate_responses(self):
        url = _normalize_base_url("https://api.opencodezen.com/v1/responses", "responses")
        self.assertEqual(url, "https://api.opencodezen.com/v1/responses")

    def test_normalize_base_url_duplicate_anthropic(self):
        url = _normalize_base_url("https://api.anthropic.com/v1/messages", "v1/messages")
        self.assertEqual(url, "https://api.anthropic.com/v1/messages")

    def test_full_url_mode_does_not_append_endpoint(self):
        from src.readmd_modules.ai import _endpoint_url
        self.assertEqual(_endpoint_url("https://gateway.test/chat", "chat/completions", "full_url"),
                         "https://gateway.test/chat")

    def test_opencode_zen_request_builder(self):
        with patch("src.readmd_modules.ai._http_json") as mock_http:
            mock_http.return_value = '{"choices": [{"message": {"content": "Hello Zen"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}}'
            gen = _chat_openai(
                base_url="https://api.opencodezen.com/v1/chat/completions",
                api_key="test-sk-zen-masked",
                model="zen-v1",
                messages=[{"role": "user", "content": "hello"}],
                temperature=0.7,
                stream=False
            )
            items = list(gen)
            self.assertEqual(items[0], "Hello Zen")
            self.assertEqual(items[1], {"usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}})
            mock_http.assert_called_once()
            call_url = mock_http.call_args[0][0]
            call_headers = mock_http.call_args[0][1]
            self.assertEqual(call_url, "https://api.opencodezen.com/v1/chat/completions")
            self.assertEqual(call_headers["Authorization"], "Bearer test-sk-zen-masked")

    def test_local_provider_can_stream_without_empty_bearer_header(self):
        self.assertTrue(_is_local_provider({"category": "local", "base_url": "http://localhost:11434/v1"}))
        with patch("src.readmd_modules.ai._http_json") as mock_http:
            mock_http.return_value = '{"choices": [{"message": {"content": "local"}}]}'
            gen = _chat_openai(
                base_url="http://localhost:11434/v1",
                api_key="",
                model="llama3.1",
                messages=[{"role": "user", "content": "hello"}],
                temperature=0.2,
                stream=False,
            )
            self.assertEqual(list(gen), ["local"])
            self.assertNotIn("Authorization", mock_http.call_args[0][1])


if __name__ == "__main__":
    unittest.main()
