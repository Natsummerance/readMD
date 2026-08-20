"""Comprehensive tests for ReadMD AI module."""
import json
import os
import sys
import tempfile
import unittest
import urllib.error
from unittest import mock
from io import BytesIO

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))
from src.readmd_modules import ai


class TestAIConfig(unittest.TestCase):
    """Test AI configuration management."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.old_config_file = ai.CONFIG_FILE
        self.old_data_dir = ai.DATA_DIR
        ai.DATA_DIR = self.temp_dir
        ai.CONFIG_FILE = os.path.join(self.temp_dir, 'ai.json')

    def tearDown(self):
        """Clean up test fixtures."""
        ai.CONFIG_FILE = self.old_config_file
        ai.DATA_DIR = self.old_data_dir
        if os.path.exists(self.temp_dir):
            import shutil
            shutil.rmtree(self.temp_dir)

    def test_ensure_config_creates_default(self):
        """Test that ensure_config creates default config when file doesn't exist."""
        cfg = ai.ensure_config()
        self.assertEqual(cfg['schema_version'], ai.CONFIG_SCHEMA_VERSION)
        self.assertEqual(cfg['providers'], [])
        self.assertEqual(cfg['current'], {})

    def test_ensure_config_upgrades_old_schema(self):
        """Test that ensure_config upgrades from old schema version."""
        old_cfg = {'schema_version': 1, 'providers': [{'name': 'test'}], 'current': {}}
        cfg_path = ai.CONFIG_FILE
        with open(cfg_path, 'w', encoding='utf-8') as f:
            json.dump(old_cfg, f)
        
        cfg = ai.ensure_config()
        self.assertEqual(cfg['schema_version'], ai.CONFIG_SCHEMA_VERSION)
        self.assertEqual(cfg['providers'], [])

    def test_ensure_config_adds_missing_ids(self):
        """Test that ensure_config adds IDs to providers without them."""
        cfg = {
            'schema_version': ai.CONFIG_SCHEMA_VERSION,
            'providers': [{'name': 'Custom Provider', 'base_url': 'https://example.com'}],
            'current': {}
        }
        cfg_path = ai.CONFIG_FILE
        with open(cfg_path, 'w', encoding='utf-8') as f:
            json.dump(cfg, f)
        
        result = ai.ensure_config()
        self.assertTrue(result['providers'][0]['id'].startswith('custom:'))

    def test_ensure_config_migrates_legacy_current(self):
        """Test migration of legacy current.provider to provider_id."""
        cfg = {
            'schema_version': ai.CONFIG_SCHEMA_VERSION,
            'providers': [{'name': 'OpenAI', 'id': 'preset:OpenAI'}],
            'current': {'provider': 'OpenAI', 'model': 'gpt-4'}
        }
        cfg_path = ai.CONFIG_FILE
        with open(cfg_path, 'w', encoding='utf-8') as f:
            json.dump(cfg, f)
        
        result = ai.ensure_config()
        self.assertIn('provider_id', result['current'])
        self.assertNotIn('provider', result['current'])
        self.assertEqual(result['current']['model'], 'gpt-4')

    def test_get_config_returns_presets_and_custom(self):
        """Test get_config returns both presets and custom providers."""
        cfg = ai.get_config()
        self.assertIn('presets', cfg)
        self.assertIn('custom', cfg)
        self.assertIn('current', cfg)
        self.assertGreater(len(cfg['presets']), 0)

    def test_get_config_annotates_providers(self):
        """Test that get_config annotates providers with has_key and key_source."""
        cfg = ai.get_config()
        for preset in cfg['presets']:
            self.assertIn('has_key', preset)
            self.assertIn('key_source', preset)
            self.assertIn('mode', preset)
            self.assertNotIn('api_key', preset)

    def test_save_config_adds_custom_provider(self):
        """Test saving a custom provider."""
        payload = {
            'providers': [{
                'name': 'My Custom API',
                'base_url': 'https://custom.example.com/v1',
                'format': 'openai',
                'models': ['model-1', 'model-2'],
                'api_key': 'sk-test123'
            }]
        }
        result = ai.save_config(payload)
        self.assertTrue(result)
        
        cfg = ai.get_config()
        self.assertEqual(len(cfg['custom']), 1)
        self.assertEqual(cfg['custom'][0]['name'], 'My Custom API')

    def test_save_config_updates_current(self):
        """Test updating current provider selection."""
        payload = {
            'current': {
                'provider_id': 'preset:OpenAI',
                'model': 'gpt-4o'
            }
        }
        ai.save_config(payload)
        
        cfg = ai.get_config()
        self.assertEqual(cfg['current']['provider_id'], 'preset:OpenAI')
        self.assertEqual(cfg['current']['model'], 'gpt-4o')

    def test_save_config_skips_duplicate_names(self):
        """Test that duplicate provider names are skipped."""
        payload = {
            'providers': [
                {'name': 'Test', 'base_url': 'https://a.com'},
                {'name': 'Test', 'base_url': 'https://b.com'}
            ]
        }
        ai.save_config(payload)
        
        cfg = ai.get_config()
        self.assertEqual(len(cfg['custom']), 1)

    def test_save_config_preserves_existing_api_key(self):
        """Test that existing API keys are preserved when not cleared."""
        ai.save_config({
            'providers': [{
                'name': 'Test',
                'base_url': 'https://test.com',
                'api_key': 'original-key'
            }]
        })
        
        ai.save_config({
            'providers': [{
                'name': 'Test',
                'base_url': 'https://test.com',
                'models': ['model-1']
            }]
        })
        
        cfg = ai.get_config()
        provider = cfg['custom'][0]
        self.assertEqual(provider['has_key'], True)


class TestAIFindProvider(unittest.TestCase):
    """Test provider lookup functionality."""

    def test_find_preset_by_name(self):
        """Test finding a preset provider by name."""
        provider = ai.find_provider('OpenAI')
        self.assertIsNotNone(provider)
        self.assertEqual(provider['name'], 'OpenAI')
        self.assertEqual(provider['format'], 'openai')

    def test_find_preset_by_id(self):
        """Test finding a preset provider by ID."""
        provider = ai.find_provider('preset:OpenAI')
        self.assertIsNotNone(provider)
        self.assertEqual(provider['name'], 'OpenAI')

    def test_find_unknown_provider(self):
        """Test finding a non-existent provider returns None."""
        provider = ai.find_provider('NonExistent')
        self.assertIsNone(provider)

    def test_all_presets_have_required_fields(self):
        """Test that all presets have required fields."""
        for preset in ai.PRESETS:
            self.assertIn('name', preset)
            self.assertIn('base_url', preset)
            self.assertIn('format', preset)
            self.assertIn('models', preset)
            self.assertIsInstance(preset['models'], list)
            self.assertGreater(len(preset['models']), 0)


class TestAIResolveKey(unittest.TestCase):
    """Test API key resolution."""

    def test_resolve_key_from_config(self):
        """Test resolving key from provider config."""
        provider = {'api_key': 'configured-key'}
        key = ai.resolve_key(provider)
        self.assertEqual(key, 'configured-key')

    def test_resolve_key_from_env(self):
        """Test resolving key from environment variable."""
        with mock.patch.dict(os.environ, {'TEST_API_KEY': 'env-key'}):
            provider = {'env_key': 'TEST_API_KEY'}
            key = ai.resolve_key(provider)
            self.assertEqual(key, 'env-key')

    def test_resolve_key_priority_config_over_env(self):
        """Test that configured key takes priority over env var."""
        with mock.patch.dict(os.environ, {'TEST_API_KEY': 'env-key'}):
            provider = {'api_key': 'config-key', 'env_key': 'TEST_API_KEY'}
            key = ai.resolve_key(provider)
            self.assertEqual(key, 'config-key')

    def test_resolve_key_empty_when_no_key(self):
        """Test empty string returned when no key available."""
        provider = {}
        key = ai.resolve_key(provider)
        self.assertEqual(key, '')

    def test_key_source_configured(self):
        """Test key_source returns 'configured' when key is set."""
        provider = {'api_key': 'test-key'}
        source = ai.key_source(provider)
        self.assertEqual(source, 'configured')

    def test_key_source_env(self):
        """Test key_source returns env var name when using env."""
        with mock.patch.dict(os.environ, {'TEST_KEY': 'value'}):
            provider = {'env_key': 'TEST_KEY'}
            source = ai.key_source(provider)
            self.assertEqual(source, 'env:TEST_KEY')

    def test_key_source_empty(self):
        """Test key_source returns empty when no key available."""
        provider = {}
        source = ai.key_source(provider)
        self.assertEqual(source, '')


class TestAIMessageFormatting(unittest.TestCase):
    """Test message formatting for different APIs."""

    def test_openai_messages_adds_system(self):
        """Test OpenAI messages add default system prompt."""
        messages = [{'role': 'user', 'content': 'Hello'}]
        formatted = ai._openai_messages(messages)
        self.assertEqual(formatted[0]['role'], 'system')
        self.assertIn('ReadMD', formatted[0]['content'])

    def test_openai_messages_preserves_existing_system(self):
        """Test existing system messages are preserved."""
        messages = [
            {'role': 'system', 'content': 'Custom system'},
            {'role': 'user', 'content': 'Hello'}
        ]
        formatted = ai._openai_messages(messages)
        self.assertEqual(formatted[0]['content'], 'Custom system')

    def test_anthropic_messages_separates_system(self):
        """Test Anthropic messages separate system from conversation."""
        messages = [
            {'role': 'system', 'content': 'System prompt'},
            {'role': 'user', 'content': 'User message'}
        ]
        (system, msgs) = ai._anthropic_messages(messages)
        self.assertEqual(system, 'System prompt')
        self.assertEqual(len(msgs), 1)
        self.assertEqual(msgs[0]['role'], 'user')

    def test_anthropic_messages_consecutive_same_role(self):
        """Test Anthropic merges consecutive messages with same role."""
        messages = [
            {'role': 'user', 'content': 'First'},
            {'role': 'user', 'content': 'Second'}
        ]
        (system, msgs) = ai._anthropic_messages(messages)
        self.assertEqual(len(msgs), 1)
        self.assertIn('First', msgs[0]['content'])
        self.assertIn('Second', msgs[0]['content'])

    def test_anthropic_messages_empty_input(self):
        """Test Anthropic handles empty messages."""
        (system, msgs) = ai._anthropic_messages([])
        self.assertEqual(len(msgs), 1)
        self.assertEqual(msgs[0]['role'], 'user')


class TestAIChatError(unittest.TestCase):
    """Test Chat error handling."""

    def test_chat_error_creation(self):
        """Test ChatError can be created and raised."""
        error = ai.ChatError('Test error')
        self.assertEqual(str(error), 'Test error')

    def test_chat_raises_for_unknown_provider(self):
        """Test chat raises error for unknown provider."""
        with self.assertRaises(ai.ChatError) as context:
            ai.chat({'provider': 'UnknownProvider'})
        self.assertIn('未知提供商', str(context.exception))

    def test_chat_raises_for_missing_api_key(self):
        """Test chat raises error when no API key is configured."""
        # OpenAI preset has env_key='OPENAI_API_KEY', so we need to ensure it's not set
        with mock.patch.dict(os.environ, {}, clear=False):
            if 'OPENAI_API_KEY' in os.environ:
                del os.environ['OPENAI_API_KEY']
            # Mock find_provider to return a provider without api_key and without env_key
            with mock.patch.object(ai, 'find_provider', return_value={'name': 'OpenAI', 'base_url': 'https://api.openai.com/v1', 'format': 'openai', 'models': ['gpt-4'], 'env_key': ''}):
                with self.assertRaises(ai.ChatError) as context:
                    ai.chat({'provider': 'OpenAI', 'model': 'gpt-4'})
                self.assertIn('API Key', str(context.exception))


class TestAIUsageParsing(unittest.TestCase):
    """Test usage token parsing."""

    def test_openai_usage_extraction(self):
        """Test extracting usage from OpenAI response."""
        data = {
            'usage': {
                'prompt_tokens': 10,
                'completion_tokens': 20,
                'total_tokens': 30
            }
        }
        usage = ai._openai_usage(data)
        self.assertEqual(usage['prompt_tokens'], 10)
        self.assertEqual(usage['completion_tokens'], 20)
        self.assertEqual(usage['total_tokens'], 30)

    def test_openai_usage_none_when_missing(self):
        """Test None returned when usage is missing."""
        usage = ai._openai_usage({})
        self.assertIsNone(usage)

    def test_anthropic_usage_extraction(self):
        """Test extracting usage from Anthropic response."""
        data = {
            'input_tokens': 10,
            'output_tokens': 20
        }
        usage = ai._anthropic_usage(data)
        self.assertEqual(usage['prompt_tokens'], 10)
        self.assertEqual(usage['completion_tokens'], 20)
        self.assertEqual(usage['total_tokens'], 30)


class TestAIListModels(unittest.TestCase):
    """Test model listing functionality."""

    @mock.patch('urllib.request.urlopen')
    def test_list_models_openai_format(self, mock_urlopen):
        """Test listing models in OpenAI format."""
        mock_response = mock.MagicMock()
        mock_response.read.return_value = json.dumps({
            'data': [
                {'id': 'gpt-4'},
                {'id': 'gpt-3.5-turbo'}
            ]
        }).encode('utf-8')
        mock_response.__enter__ = mock.MagicMock(return_value=mock_response)
        mock_response.__exit__ = mock.MagicMock(return_value=False)
        mock_urlopen.return_value = mock_response
        
        models = ai.list_models('https://api.openai.com/v1', 'test-key')
        self.assertEqual(models, ['gpt-4', 'gpt-3.5-turbo'])

    @mock.patch('urllib.request.urlopen')
    def test_list_models_anthropic_format(self, mock_urlopen):
        """Test listing models in Anthropic format."""
        mock_response = mock.MagicMock()
        mock_response.read.return_value = json.dumps({
            'data': [
                {'id': 'claude-3-opus'},
                {'id': 'claude-3-sonnet'}
            ]
        }).encode('utf-8')
        mock_response.__enter__ = mock.MagicMock(return_value=mock_response)
        mock_response.__exit__ = mock.MagicMock(return_value=False)
        mock_urlopen.return_value = mock_response
        
        models = ai.list_models('https://api.anthropic.com', 'test-key', mode='messages')
        self.assertEqual(models, ['claude-3-opus', 'claude-3-sonnet'])

    def test_list_models_requires_base_url(self):
        """Test list_models raises error without base URL."""
        with self.assertRaises(ai.ChatError) as context:
            ai.list_models(None, 'test-key')
        self.assertIn('Base URL', str(context.exception))

    def test_list_models_requires_api_key(self):
        """Test list_models raises error without API key."""
        with self.assertRaises(ai.ChatError) as context:
            ai.list_models('https://api.example.com', '')
        self.assertIn('API Key', str(context.exception))

    @mock.patch('urllib.request.urlopen')
    def test_list_models_handles_http_error(self, mock_urlopen):
        """Test list_models handles HTTP errors."""
        http_error = urllib.error.HTTPError(
            'https://api.example.com/models', 401, 'Unauthorized',
            {}, BytesIO(b'{"error": "Invalid API key"}')
        )
        mock_urlopen.side_effect = http_error
        
        with self.assertRaises(ai.ChatError) as context:
            ai.list_models('https://api.example.com', 'invalid-key')
        self.assertIn('HTTP 401', str(context.exception))


class TestAILoad(unittest.TestCase):
    """Test module load hook."""

    def test_load_calls_ensure_config(self):
        """Test that load() calls ensure_config()."""
        with mock.patch.object(ai, 'ensure_config') as mock_ensure:
            ai.load()
            mock_ensure.assert_called_once()


class TestAISaveConfigEdgeCases(unittest.TestCase):
    """Test edge cases in save_config."""

    def setUp(self):
        """Set up test fixtures."""
        self.temp_dir = tempfile.mkdtemp()
        self.old_config_file = ai.CONFIG_FILE
        self.old_data_dir = ai.DATA_DIR
        ai.DATA_DIR = self.temp_dir
        ai.CONFIG_FILE = os.path.join(self.temp_dir, 'ai.json')

    def tearDown(self):
        """Clean up test fixtures."""
        ai.CONFIG_FILE = self.old_config_file
        ai.DATA_DIR = self.old_data_dir
        if os.path.exists(self.temp_dir):
            import shutil
            shutil.rmtree(self.temp_dir)

    def test_save_config_with_invalid_provider_type(self):
        """Test that non-dict providers are skipped."""
        payload = {
            'providers': ['string', 123, None, {'name': 'Valid', 'base_url': 'https://valid.com'}]
        }
        ai.save_config(payload)
        
        cfg = ai.get_config()
        self.assertEqual(len(cfg['custom']), 1)

    def test_save_config_clears_key_when_requested(self):
        """Test that clear_key flag removes existing API key."""
        ai.save_config({
            'providers': [{
                'name': 'Test',
                'base_url': 'https://test.com',
                'api_key': 'original-key'
            }]
        })
        
        ai.save_config({
            'providers': [{
                'name': 'Test',
                'base_url': 'https://test.com',
                'clear_key': True
            }]
        })
        
        cfg = ai.get_config()
        provider = cfg['custom'][0]
        self.assertEqual(provider['has_key'], False)

    def test_save_config_strips_model_names(self):
        """Test that model names are stripped of whitespace."""
        payload = {
            'providers': [{
                'name': 'Test',
                'base_url': 'https://test.com',
                'models': [' model-1 ', '  ', 'model-2']
            }]
        }
        ai.save_config(payload)
        
        cfg = ai.get_config()
        self.assertEqual(cfg['custom'][0]['models'], ['model-1', 'model-2'])


if __name__ == '__main__':
    unittest.main()
