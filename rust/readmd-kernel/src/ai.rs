//! AI/LLM integration module for ReadMD kernel.
//!
//! Provides OpenAI-compatible and Anthropic Claude API clients with:
//! - Provider catalog management (pre-configured vendors)
//! - AI settings persistence (`src/readmd_modules/ai.py:78-349`) whose secrets
//!   live in the OS credential store / Fernet vault, never in the settings file
//! - Streaming responses
//! - Model listing and selection
//! - Custom provider configuration
//!
//! # Design Principles
//!
//! - **Offline-first**: All credentials stored locally, no telemetry
//! - **Provider-agnostic**: Abstract over different LLM APIs
//! - **Streaming support**: SSE for real-time chat
//! - **Error resilience**: Graceful fallbacks and clear error codes
//! - **Fail-closed secrets**: a write that cannot be encrypted is refused
//!   (`ai.py:267-269`), it never degrades to plaintext

use crate::error::{ApiError, Error, Result as KernelResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use ureq::{Agent, AgentBuilder};

/// Pre-configured AI providers with their endpoints and requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCatalog {
    pub providers: Vec<Provider>,
}

impl Default for ProviderCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderCatalog {
    /// Create the default provider catalog with common vendors.
    pub fn new() -> Self {
        let mut providers = Vec::new();

        // OpenAI providers
        providers.push(Provider {
            id: "openai".into(),
            name: "OpenAI".into(),
            base_url: "https://api.openai.com/v1".into(),
            models: vec![
                "gpt-4o".into(),
                "gpt-4o-mini".into(),
                "gpt-4-turbo".into(),
                "gpt-4".into(),
                "gpt-3.5-turbo".into(),
            ],
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
        });

        providers.push(Provider {
            id: "azure-openai".into(),
            name: "Azure OpenAI".into(),
            base_url: "".into(), // User must provide custom endpoint
            models: vec![
                "gpt-4o".into(),
                "gpt-4".into(),
                "gpt-3.5-turbo".into(),
            ],
            requires_api_key: true,
            auth_header: "api-key".into(),
            auth_prefix: "".into(),
        });

        // Anthropic providers
        providers.push(Provider {
            id: "anthropic".into(),
            name: "Anthropic Claude".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            models: vec![
                "claude-3-5-sonnet-latest".into(),
                "claude-3-5-sonnet-20241022".into(),
                "claude-3-opus-latest".into(),
                "claude-3-opus-20240229".into(),
                "claude-3-haiku-latest".into(),
                "claude-3-haiku-20240307".into(),
            ],
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
        });

        // OpenRouter (unified API for multiple models)
        providers.push(Provider {
            id: "openrouter".into(),
            name: "OpenRouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            models: vec![
                "anthropic/claude-3.5-sonnet".into(),
                "google/gemini-2.0-flash".into(),
                "meta-llama/llama-3.1-70b-instruct".into(),
                "mistralai/mistral-nemo".into(),
            ],
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
        });

        // Ollama (local deployment)
        providers.push(Provider {
            id: "ollama".into(),
            name: "Ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            models: vec![], // Dynamic model discovery
            requires_api_key: false,
            auth_header: "".into(),
            auth_prefix: "".into(),
        });

        // LM Studio (local server)
        providers.push(Provider {
            id: "lm-studio".into(),
            name: "LM Studio".into(),
            base_url: "http://localhost:1234/v1".into(),
            models: vec![], // Dynamic model discovery
            requires_api_key: false,
            auth_header: "".into(),
            auth_prefix: "".into(),
        });

        // Groq (fast inference)
        providers.push(Provider {
            id: "groq".into(),
            name: "Groq".into(),
            base_url: "https://api.groq.com/openai/v1".into(),
            models: vec![
                "llama3-70b-8192".into(),
                "llama3-8b-8192".into(),
                "mixtral-8x7b-32768".into(),
                "gemma2-9b-it".into(),
            ],
            requires_api_key: true,
            auth_header: "Authorization".into(),
            auth_prefix: "Bearer ".into(),
        });

        ProviderCatalog { providers }
    }

    /// Get all available provider IDs.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.id.as_str()).collect()
    }

    /// Get a specific provider by ID.
    pub fn get_provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// Check if a provider exists.
    pub fn has_provider(&self, id: &str) -> bool {
        self.providers.iter().any(|p| p.id == id)
    }
}

/// AI provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Unique identifier for this provider.
    pub id: String,
    /// Display name shown in UI.
    pub name: String,
    /// Base URL for API requests.
    pub base_url: String,
    /// List of supported model IDs. Empty means dynamic discovery.
    pub models: Vec<String>,
    /// Whether an API key is required.
    pub requires_api_key: bool,
    /// HTTP header name for authentication.
    pub auth_header: String,
    /// Prefix to prepend to API key (e.g., "Bearer ").
    pub auth_prefix: String,
}

/// Chat message types for LLM interactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role")]
pub enum Message {
    #[serde(rename = "user")]
    User {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        images: Option<Vec<ImageData>>,
    },
    #[serde(rename = "assistant")]
    Assistant { content: String },
    #[serde(rename = "system")]
    System { content: String },
    #[serde(rename = "tool")]
    Tool {
        content: String,
        tool_call_id: String,
    },
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Message::User { content: content.into(), images: None }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Message::Assistant { content: content.into() }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Message::System { content: content.into() }
    }

    pub fn tool(content: impl Into<String>, tool_call_id: impl Into<String>) -> Self {
        Message::Tool { content: content.into(), tool_call_id: tool_call_id.into() }
    }
}

/// Image data for multimodal models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub url: String, // data URI or remote URL
}

/// Chat completion request.
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
}

impl ChatCompletionRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        ChatCompletionRequest {
            model: model.into(),
            messages,
            temperature: Some(0.7),
            max_tokens: None,
            stream: Some(false),
            top_p: None,
            stop: None,
        }
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: i32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }
}

/// Chat completion response chunk (streaming).
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkChoice {
    pub index: i32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// Chat completion response (non-streaming).
#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// AI client for making LLM API calls.
pub struct AIClient {
    agent: Agent,
    provider: Provider,
    api_key: Option<String>,
}

impl AIClient {
    /// Create a new AI client with a specific provider.
    pub fn new(provider: Provider, api_key: Option<String>) -> Result<Self, String> {
        let agent = AgentBuilder::new()
            .timeout_read(std::time::Duration::from_secs(60))
            .timeout_write(std::time::Duration::from_secs(60))
            .build();

        Ok(AIClient { agent, provider, api_key })
    }

    /// Set the API key for this client.
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Send a chat completion request (non-streaming).
    ///
    /// Failure codes follow `/api/ai/chat`: every problem below is a `ChatError`
    /// in Python (`ai.py:505`, `ai.py:392-395`, `ai.py:610`), which the route's
    /// `except Exception` answers as `502 provider_error`
    /// (`readmd.py:2449-2453`).  The reason travels in `detail`, which
    /// `crate::error::ApiError` keeps out of the response body — the same
    /// guarantee `_send_api_error` makes at `readmd.py:1421-1430`.
    pub fn chat(&self, request: &ChatCompletionRequest) -> Result<ChatCompletionResponse, ApiError> {
        // Validate API key if required
        if self.provider.requires_api_key && self.api_key.is_none() {
            return Err(AiRoute::Chat.raised_error("api key not configured"));
        }

        // Build the request URL — `ai.py:552-556 _endpoint_url`, so a Base URL
        // that already carries a trailing endpoint is normalised, not doubled.
        let url = endpoint_url(&self.provider.base_url, "chat/completions", "prefix");

        // Serialize request body
        let body = serde_json::to_string(request)
            .map_err(|e| AiRoute::Chat.raised_error(format!("request encode failed: {e}")))?;

        // Build headers and send request
        let mut req = self.agent.post(&url);
        req = req.set("Content-Type", "application/json");
        if let Some(ref api_key) = self.api_key {
            let auth_header = format!("{} {}", self.provider.auth_prefix, api_key);
            req = req.set("Authorization", &auth_header);
        }
        
        let response = req
            .send_string(&body)
            .map_err(|e| AiRoute::Chat.raised_error(format!("request failed: {e}")))?;

        // Parse response
        let status = response.status();
        if status != 200 {
            let error_body = response.into_string().unwrap_or_default();
            // `_http_json` raises `ChatError("HTTP %d：%s")` for a non-2xx
            // (`ai.py:391-393`), so this is the same raised branch.
            return Err(AiRoute::Chat.raised_error(format!("HTTP {status}: {}", truncate(&error_body))));
        }

        let resp: ChatCompletionResponse = response.into_json().map_err(|e| {
            AiRoute::Chat.raised_error(format!("响应解析失败：{e}"))
        })?;

        Ok(resp)
    }

    /// Send a streaming chat completion request.
    pub fn chat_stream(
        &self,
        request: &ChatCompletionRequest,
        mut handler: Box<dyn FnMut(Result<ChatCompletionChunk, ApiError>) + Send>,
    ) -> Result<(), ApiError> {
        // Validate API key if required
        if self.provider.requires_api_key && self.api_key.is_none() {
            return Err(AiRoute::Chat.raised_error("api key not configured"));
        }

        // Build the request URL — `ai.py:552-556 _endpoint_url`, so a Base URL
        // that already carries a trailing endpoint is normalised, not doubled.
        let url = endpoint_url(&self.provider.base_url, "chat/completions", "prefix");

        // Serialize request body
        let body = serde_json::to_string(request)
            .map_err(|e| AiRoute::Chat.raised_error(format!("request encode failed: {e}")))?;

        // Build headers and send request
        let mut req = self.agent.post(&url);
        req = req.set("Content-Type", "application/json");
        if let Some(ref api_key) = self.api_key {
            let auth_header = format!("{} {}", self.provider.auth_prefix, api_key);
            req = req.set("Authorization", &auth_header);
        }
        let response = req
            .send_string(&body)
            .map_err(|e| AiRoute::Chat.raised_error(format!("request failed: {e}")))?;

        let status = response.status();
        if status != 200 {
            let error_body = response.into_string().unwrap_or_default();
            return Err(AiRoute::Chat.raised_error(format!("HTTP {status}: {}", truncate(&error_body))));
        }

        // Process streaming response using into_reader()
        let mut reader = response.into_reader();
        let mut buffer = String::new();
        let mut buf = [0u8; 4096];
        
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    buffer.push_str(&String::from_utf8_lossy(&buf[..n]));

                    // Process complete lines
                    while let Some(pos) = buffer.find('\n') {
                        let line = buffer[..pos].trim().to_string();
                        buffer.drain(..pos + 1);

                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data == "[DONE]" {
                                return Ok(());
                            }

                            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                handler(Ok(chunk));
                            }
                        }
                    }
                }
                Err(e) => {
                    // `ai.py:642-643` — `ChatError("连接中断：%s")`, the same
                    // raised branch as every other send failure.
                    return Err(AiRoute::Chat
                        .raised_error(format!("stream read failed: {e}")));
                }
            }
        }

        Ok(())
    }

    /// `ai.py:883-950 list_models` — the provider *is* asked.
    ///
    /// The stub this replaces returned only the locally configured catalogue,
    /// which broke the Models route in two ways: it answered 200 with a list the
    /// provider never confirmed, and it could not fail, so the
    /// `400 model_list_failed` branch (`readmd.py:2376-2378`) was unreachable.
    /// Every problem below is therefore a *raise* — an empty list is never a
    /// success here, exactly like `ai.py:949`, which raises
    /// `ChatError("接口未返回模型列表（data 为空）")`.
    pub fn list_models(&self) -> Result<Vec<String>, ApiError> {
        // `ai.py:885-887`
        let clean = normalize_base_url(&self.provider.base_url);
        if clean.is_empty() {
            return Err(AiRoute::Models.raised_error("请先填写 Base URL"));
        }

        // `ai.py:892-904` — the same candidate order, so the same endpoint wins.
        let mut candidates: Vec<String> = Vec::new();
        if clean.ends_with("/v1") {
            candidates.push(format!("{clean}/models"));
            candidates.push(format!("{}/models", clean[..clean.len() - 3].trim_end_matches('/')));
        } else {
            candidates.push(format!("{clean}/models"));
            candidates.push(format!("{clean}/v1/models"));
            candidates.push(format!("{clean}/api/tags"));
        }

        let mut last_error: Option<String> = None;
        let mut body: Option<String> = None;
        for url in candidates {
            match self.http_get_json(&url) {
                Ok(text) => {
                    if !text.trim().is_empty() {
                        body = Some(text);
                        break;
                    }
                }
                Err(e) => last_error = Some(e),
            }
        }
        // `ai.py:918-922` — re-raise the transport error verbatim.
        let text = match body {
            Some(text) => text,
            None => {
                return Err(AiRoute::Models.raised_error(match last_error {
                    Some(e) => e,
                    None => "未能连接到模型接口".to_string(),
                }))
            }
        };
        model_ids_from_json(&text).map_err(|message| AiRoute::Models.raised_error(message))
    }

    /// `ai.py:868-880 _http_get_json` — the GET half of the HTTP layer.
    ///
    /// The reason string is diagnostics only: `AiRoute::Models::raised_error`
    /// puts it in `detail`, so a provider body never reaches the client.
    fn http_get_json(&self, url: &str) -> Result<String, String> {
        let mut request = self.agent.get(url);
        if let Some(ref api_key) = self.api_key {
            let header = if self.provider.auth_header.is_empty() {
                "Authorization".to_string()
            } else {
                self.provider.auth_header.clone()
            };
            request = request.set(&header, &format!("{}{}", self.provider.auth_prefix, api_key));
        }
        match request.call() {
            Ok(response) => {
                let status = response.status();
                let mut text = String::new();
                let _ = response
                    .into_reader()
                    .read_to_string(&mut text)
                    .map_err(|e| format!("网络错误：{e}"));
                if status != 200 {
                    return Err(format!("HTTP {status}：{}", truncate(&text)));
                }
                Ok(text)
            }
            Err(ureq::Error::Status(code, response)) => {
                let mut text = String::new();
                let _ = response.into_reader().read_to_string(&mut text);
                Err(format!("HTTP {code}：{}", truncate(&text)))
            }
            Err(e) => Err(format!("网络错误：{e}")),
        }
    }
}

/// `ai.py:925-949`, split out so the parsing contract is testable without a
/// provider on the other end of a socket.
fn model_ids_from_json(text: &str) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| format!("模型列表解析失败：{}", truncate_to(text, 300)))?;
    // `ai.py:926` is `d.get('data') or d.get('models') or []`: an *empty* `data`
    // is falsy in Python and therefore falls through to `models`.
    let first_truthy_array = |obj: &Map<String, Value>, keys: [&str; 2]| -> Vec<Value> {
        keys.iter()
            .filter_map(|key| obj.get(*key))
            .filter(|v| py_truthy(v))
            .find_map(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let raw_items: Vec<Value> = match &value {
        Value::Array(items) => items.clone(),
        Value::Object(obj) => first_truthy_array(obj, ["data", "models"]),
        _ => Vec::new(),
    };
    let mut ids: Vec<String> = Vec::new();
    for item in raw_items {
        match item {
            Value::Object(obj) => {
                // `ai.py:936`: `id` or `name` or `model`, skipping any value
                // CPython would treat as false.
                let id = ["id", "name", "model"]
                    .iter()
                    .filter_map(|key| obj.get(*key))
                    .filter(|v| py_truthy(v))
                    .next()
                    .map(py_str)
                    .unwrap_or_default();
                let id = id.trim().to_string();
                if !id.is_empty() {
                    ids.push(id);
                }
            }
            Value::String(name) => {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    ids.push(name);
                }
            }
            _ => {}
        }
    }
    if ids.is_empty() {
        return Err("接口未返回模型列表（data 为空）".to_string());
    }
    Ok(ids)
}

/// `ai.py:527-549 _normalize_base_url` with `endpoint=''`, i.e. the cleaned
/// prefix `list_models` builds its candidates from.
fn normalize_base_url(base_url: &str) -> String {
    let mut u = base_url.trim().trim_end_matches('/').to_string();
    if u.is_empty() {
        return u;
    }
    const SUFFIXES: &[&str] = &[
        "/chat/completions",
        "/completions",
        "/responses",
        "/v1/messages",
        "/messages",
        "/v1/models",
        "/models",
    ];
    let lowered = u.to_lowercase();
    for suffix in SUFFIXES {
        if lowered.ends_with(suffix) {
            let keep = u.chars().count() - suffix.chars().count();
            u = u.chars().take(keep).collect::<String>().trim_end_matches('/').to_string();
            break;
        }
    }
    u
}

/// `ai.py:552-556 _endpoint_url` — a prefix-mode endpoint is appended to the
/// normalised base, a `full_url` mode base is used verbatim.
fn endpoint_url(base_url: &str, endpoint: &str, endpoint_mode: &str) -> String {
    if endpoint_mode.trim().to_lowercase() == "full_url" {
        return base_url.trim().trim_end_matches('/').to_string();
    }
    let base = normalize_base_url(base_url);
    if base.is_empty() || endpoint.is_empty() {
        return base;
    }
    format!("{}/{}", base, endpoint.trim_start_matches('/'))
}

/// `ai.py:392` — `e.read().decode(...)[:500]`, i.e. a provider error body is
/// clamped before it becomes diagnostics.  Slicing on `count < 500` chars
/// rather than bytes keeps CPython's "500 characters" reading without ever
/// cutting a UTF-8 sequence in half.
fn truncate(body: &str) -> String {
    truncate_to(body, 500)
}

/// The same clamp at an explicit width; `ai.py:929` uses `[:300]` for a model
/// list that will not parse.
fn truncate_to(body: &str, limit: usize) -> String {
    let mut out = String::new();
    for ch in body.chars().take(limit) {
        out.push(ch);
    }
    out
}

// ---------------------------------------------------------------------------
// Secrets + AI settings persistence — port of `src/readmd_modules/ai.py:78-349`
// ---------------------------------------------------------------------------
//
// The security contract this section exists to hold is stated in the Python
// module docstring (`crypto.py:4-7`): *"API keys are never written as
// plaintext.  A missing/failed crypto backend is a hard error for writes"*.
// Python enforces it in three places, and all three are mirrored here:
//
// * `ai.py:267-269` — a save that carries an `api_key` with no crypto backend
//   raises `RuntimeError` instead of persisting the key.
// * `ai.py:134-140` — a legacy *plaintext* key on disk is deleted and the
//   provider is flagged `credential_reset_required`; it is never read back.
// * `ai.py:200-201` — the config endpoint reports key *status* only;
//   `d.pop("api_key", None)` keeps the secret out of every response.
//
// The secret itself goes to `crate::crypto`, the kernel's own port of
// `crypto.py`: `store_credential` tries the OS credential store
// (`crypto.py:136-164`) and falls back to the Fernet-encrypted
// `credentials.vault` (`crypto.py:106-118`).  What never happens on either
// path is a plaintext key.

/// `ai.py:24` — `CONFIG_SCHEMA_VERSION = 3`.
pub const CONFIG_SCHEMA_VERSION: u32 = 3;

/// `ai.py:23` — `CONFIG_FILE = os.path.join(DATA_DIR, 'ai.json')`.
///
/// The name is spelled exactly as Python spells it: the settings file is a
/// shared on-disk contract with the legacy app, and Windows preserves the
/// case of the first creator while Linux honours it.
pub const AI_CONFIG_FILE_NAME: &str = "ai.json";

/// `crypto.py:36` / `crypto.py:107` — the two files the credential backend
/// owns.  They are named *only* to pin the spelling here; `crate::crypto`
/// builds them.
pub const ENCRYPTION_KEY_FILE_NAME: &str = "encryption.key";
pub const CREDENTIALS_VAULT_FILE_NAME: &str = "credentials.vault";

/// `src/readmd_core/config.py:39`/`:41`/`:43` — the app data directory
/// component Python creates (`'ReadMD'`, mixed case) on macOS, Windows and
/// Linux.  Pinned so the divergence at `lib.rs:316` cannot be forgotten:
/// see [`python_data_dir_name_matches_kernel_base`].
pub const PYTHON_DATA_DIR_NAME: &str = "ReadMD";

/// `crypto.py:20` — `SERVICE_NAME = "ReadMD"`, the keychain service and the
/// `credentials.vault` key prefix.
pub const CREDENTIAL_SERVICE_NAME: &str = "ReadMD";

/// `crypto.py:21` — `_CREDENTIAL_RE = re.compile(r"^cred:[A-Za-z0-9_-]{8,128}$")`.
///
/// Hand-scanned rather than via `regex` so the ASCII-only class stays exactly
/// CPython's, and mirrors `crypto.rs::validate_credential_id`.
pub fn is_valid_credential_id(raw: &str) -> bool {
    let cid = raw.trim();
    match cid.strip_prefix("cred:") {
        Some(body) => {
            let len = body.chars().count();
            (8..=128).contains(&len)
                && body
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }
        None => false,
    }
}

/// A new opaque handle, `ai.py:271`'s `"cred:" + uuid.uuid4().hex`.
fn new_credential_id() -> String {
    format!("cred:{}", uuid::Uuid::new_v4().simple())
}

/// The seam that keeps the secret-handling policy testable without touching
/// the real keychain or the process-wide `READMD_DATA_DIR`.
///
/// The five methods are `crypto.py`'s five module-level credential functions.
pub trait SecretBackend: Send + Sync {
    /// `crypto.store_credential` (`crypto.py:208-217`), returning Python's
    /// backend tag `"native"` or `"encrypted-vault"`.
    fn store(&self, credential_id: &str, secret: &str) -> KernelResult<String>;
    /// `crypto.load_credential` (`crypto.py:220-228`).
    fn load(&self, credential_id: &str) -> KernelResult<String>;
    /// `crypto.delete_credential` (`crypto.py:231-239`).
    fn delete(&self, credential_id: &str) -> KernelResult<()>;
    /// `crypto.is_crypto_available` (`crypto.py:30-32`).
    fn available(&self) -> bool;
}

/// The production backend: `crate::crypto`, i.e. the OS credential store with
/// the encrypted-vault fallback.  No plaintext branch exists.
pub struct OsSecretBackend;

impl SecretBackend for OsSecretBackend {
    fn store(&self, credential_id: &str, secret: &str) -> KernelResult<String> {
        // `crypto.py:210-211` — an empty secret is a `ValueError` before any
        // store is touched, so a blank field can never wipe a good credential.
        if secret.is_empty() {
            return Err(Error::Crypto("credential secret is empty".to_string()));
        }
        let backend = crate::crypto::store_credential(credential_id, secret)?;
        Ok(match backend {
            crate::crypto::StoreBackend::Native => "native".to_string(),
            crate::crypto::StoreBackend::EncryptedVault => "encrypted-vault".to_string(),
        })
    }

    fn load(&self, credential_id: &str) -> KernelResult<String> {
        crate::crypto::load_credential(credential_id)
    }

    fn delete(&self, credential_id: &str) -> KernelResult<()> {
        crate::crypto::delete_credential(credential_id)
    }

    fn available(&self) -> bool {
        crate::crypto::is_crypto_available()
    }
}

/// `crypto.py`'s credential API as a value, so the AI settings file can be
/// written without a secret ever appearing in it.
pub struct CredentialStore<B: SecretBackend = OsSecretBackend> {
    backend: B,
}

impl CredentialStore<OsSecretBackend> {
    /// The kernel's own store: OS credential manager, else encrypted vault.
    pub fn os_backend() -> CredentialStore<OsSecretBackend> {
        CredentialStore {
            backend: OsSecretBackend,
        }
    }
}

impl<B: SecretBackend> CredentialStore<B> {
    pub fn with_backend(backend: B) -> Self {
        CredentialStore { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// `crypto.store_credential`.  The returned string is what Python persists
    /// in `credential_backend` (`ai.py:273`), never the secret.
    pub fn store(&self, credential_id: &str, secret: &str) -> KernelResult<String> {
        self.backend.store(credential_id, secret)
    }

    /// `crypto.load_credential`.  Python's `except` arms all collapse to `''`,
    /// i.e. "no key configured", so this cannot fail.
    pub fn load(&self, credential_id: &str) -> String {
        if credential_id.is_empty() {
            return String::new();
        }
        self.backend
            .load(credential_id)
            .unwrap_or_else(|e| {
                log::warn!("credential lookup failed: {e}");
                String::new()
            })
    }

    /// `crypto.delete_credential`.
    pub fn delete(&self, credential_id: &str) -> KernelResult<()> {
        if credential_id.is_empty() {
            return Ok(());
        }
        self.backend.delete(credential_id)
    }

    /// `crypto.is_crypto_available` — the gate `ai.py:268` refuses a plaintext
    /// write behind.
    pub fn is_crypto_available(&self) -> bool {
        self.backend.available()
    }

    /// `ai.py:329-340 resolve_key`: credential store > legacy `enc:` blob >
    /// `env_key` > empty.
    ///
    /// Python's first branch `return`s whatever `load_credential` gives it — an
    /// unreadable credential therefore yields `''` and does *not* fall through
    /// to the environment, which is why the branch is not an `if`-guard here.
    /// A plaintext `api_key` on disk is never read: only an `enc:` value is
    /// handed to `decrypt_api_key`, and `crypto.py:79-81` refuses anything else.
    pub fn resolve_key(&self, provider: &Value) -> String {
        if let Some(cid) = provider.get("credential_id").and_then(|v| v.as_str()) {
            if !cid.is_empty() {
                return self.load(cid);
            }
        }
        if let Some(legacy) = provider.get("api_key").and_then(|v| v.as_str()) {
            if legacy.starts_with("enc:") {
                return crate::crypto::decrypt_api_key(legacy, None).unwrap_or_default();
            }
        }
        let env = provider.get("env_key").and_then(|v| v.as_str()).unwrap_or("");
        if !env.is_empty() {
            if let Some(v) = std::env::var(env).ok().filter(|v| !v.is_empty()) {
                return v;
            }
        }
        String::new()
    }

    /// `ai.py:343-349 key_source`.
    pub fn key_source(&self, provider: &Value) -> String {
        let cid = provider.get("credential_id").and_then(|v| v.as_str()).unwrap_or("");
        if !cid.is_empty() && !self.load(cid).is_empty() {
            return "configured".to_string();
        }
        let env = provider.get("env_key").and_then(|v| v.as_str()).unwrap_or("");
        if !env.is_empty() && !std::env::var(env).unwrap_or_default().is_empty() {
            return format!("env:{env}");
        }
        String::new()
    }
}

/// `ai.py:253-264` — the custom-header allow-list that keeps an
/// `Authorization`/`api-key`/`token` value from sneaking a secret into the
/// settings file under another name.
fn sanitize_headers(raw: Option<&Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let Some(obj) = raw.and_then(|v| v.as_object()) else {
        return out;
    };
    for (name, value) in obj {
        let name = name.trim();
        let lower = name.to_lowercase();
        let banned = ["authorization", "api-key", "apikey", "token", "secret", "password", "cookie"]
            .iter()
            .any(|marker| lower.contains(marker));
        if name.is_empty() || name.chars().count() > 128 || banned {
            continue;
        }
        let value = py_str(value);
        if value.chars().count() <= 2048 && !value.chars().any(|c| (c as u32) < 32) {
            out.insert(name.to_string(), json!(value));
        }
    }
    out
}

/// CPython's `str(x)` for the scalars that reach `ai.py:262`.
fn py_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// CPython's truthiness for the values that flow through `ai.py`'s `or` chains
/// (`ai.py:926`, `ai.py:936`): `None`, `False`, `0`, `''`, `[]` and `{}` are
/// false and therefore fall through to the next candidate.
fn py_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
        Value::Number(n) => n.as_f64().map(|v| v != 0.0).unwrap_or(true),
    }
}

/// `ai.py:163` — `raise OSError("ai_config_write_failed")` when
/// `utils.save_json` reports a failed write.
fn write_failed() -> Error {
    Error::Msg("ai_config_write_failed".to_string())
}

/// The AI settings file: `DATA_DIR/ai.json` (`ai.py:23`), schema v3.
pub struct AiSettings<B: SecretBackend = OsSecretBackend> {
    path: PathBuf,
    credentials: CredentialStore<B>,
}

impl AiSettings<OsSecretBackend> {
    /// Python's own location: `os.path.join(config.DATA_DIR, 'ai.json')`.
    ///
    /// `crate::paths::data_dir()` is used rather than a locally rebuilt path so
    /// this file and `credentials.vault`/`encryption.key` (`crypto.rs:311/315`)
    /// can never land in two different directories.
    pub fn at_default_location() -> AiSettings<OsSecretBackend> {
        AiSettings {
            path: crate::paths::data_dir().join(AI_CONFIG_FILE_NAME),
            credentials: CredentialStore::os_backend(),
        }
    }
}

impl<B: SecretBackend> AiSettings<B> {
    pub fn new(path: impl Into<PathBuf>, credentials: CredentialStore<B>) -> Self {
        AiSettings {
            path: path.into(),
            credentials,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn credentials(&self) -> &CredentialStore<B> {
        &self.credentials
    }

    /// `ai.py:153-158 _read_cfg`: a missing or malformed file is the empty v3
    /// config, never an error.
    fn read_cfg(&self) -> Value {
        let default = || json!({"schema_version": CONFIG_SCHEMA_VERSION, "providers": [], "current": {}});
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str::<Value>(&text).unwrap_or_else(|_| default()),
            Err(_) => default(),
        }
    }

    /// `ai.py:161-164 _write_cfg` over `utils.save_json` (`utils.py:55-80`):
    /// create the parent, `indent=2` + non-ASCII kept, unique `.tmp` sibling,
    /// then the rename.  A failed write raises, like `_write_cfg` does.
    fn write_cfg(&self, cfg: &Value) -> KernelResult<()> {
        let target = &self.path;
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|_| write_failed())?;
            }
        }
        let text = serde_json::to_string_pretty(cfg).map_err(|_| write_failed())?;
        // `tempfile.mkstemp(prefix=basename + '.', suffix='.tmp')` — a unique
        // sibling, not a fixed `.tmp`, so a second writer cannot truncate the
        // first one's temp file.
        let stem = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| AI_CONFIG_FILE_NAME.to_string());
        let tmp = {
            let mut base = target.clone();
            base.set_file_name(format!("{stem}.{}.tmp", uuid::Uuid::new_v4().simple()));
            base
        };
        std::fs::write(&tmp, text).map_err(|_| write_failed())?;
        if let Err(e) = std::fs::rename(&tmp, target) {
            let _ = std::fs::remove_file(&tmp);
            return Err(Error::Msg(format!("ai_config_write_failed: {e}")));
        }
        Ok(())
    }

    /// `ai.py:83-150 ensure_config`.
    ///
    /// Two jobs: upgrade v2, and — the security-relevant one — get every
    /// `api_key` out of the file.  An `enc:` blob is migrated into the
    /// credential backend; anything else (plaintext, or an `enc:` blob that
    /// will not decrypt) is *deleted* and the provider is flagged
    /// `credential_reset_required`, so no code path can hand a stale or
    /// unprotected secret back to the UI.
    pub fn ensure_config(&self) -> KernelResult<Value> {
        let mut cfg = self.read_cfg();
        let version = cfg.get("schema_version").and_then(|v| v.as_i64()).unwrap_or(0);
        if version != 2 && version != CONFIG_SCHEMA_VERSION as i64 {
            let fresh = json!({"schema_version": CONFIG_SCHEMA_VERSION, "providers": [], "current": {}});
            self.write_cfg(&fresh)?;
            return Ok(fresh);
        }

        let mut changed = false;
        if version == 2 {
            cfg["schema_version"] = json!(CONFIG_SCHEMA_VERSION);
            if let Some(providers) = cfg.get_mut("providers").and_then(|v| v.as_array_mut()) {
                for provider in providers.iter_mut().filter_map(|p| p.as_object_mut()) {
                    provider
                        .entry("endpoint_mode".to_string())
                        .or_insert_with(|| json!("prefix"));
                    provider
                        .entry("capabilities".to_string())
                        .or_insert_with(|| json!({}));
                }
            }
            // Python writes here unconditionally (`ai.py:101`), not only when
            // `changed` is set later.
            self.write_cfg(&cfg)?;
        }

        let mut providers: Vec<Value> = cfg
            .get("providers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for provider in providers.iter_mut() {
            let Some(p) = provider.as_object_mut() else { continue };
            if p.get("id").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
                p.insert("id".into(), json!(format!("custom:{}", uuid::Uuid::new_v4().simple())));
                changed = true;
            }
            let legacy = p
                .get("api_key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_default();
            if legacy.is_empty() {
                continue;
            }
            if legacy.starts_with("enc:") && self.credentials.is_crypto_available() {
                let secret = crate::crypto::decrypt_api_key(&legacy, None).unwrap_or_default();
                if !secret.is_empty() {
                    let cid = p
                        .get("credential_id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(new_credential_id);
                    match self.credentials.store(&cid, &secret) {
                        Ok(backend) => {
                            // `ai.py:119-122`: handle in, material out.
                            p.insert("credential_id".into(), json!(cid));
                            p.insert("credential_backend".into(), json!(backend));
                            p.remove("api_key");
                        }
                        Err(e) => {
                            // `ai.py:123-128`: the migration failure discards
                            // the key rather than leaving it behind.
                            log::error!("legacy credential migration failed: {e}");
                            p.remove("api_key");
                            p.remove("credential_id");
                            p.insert("credential_reset_required".into(), json!(true));
                        }
                    }
                    changed = true;
                    continue;
                }
            }
            // `ai.py:134-140` — "Never keep a plaintext legacy key in the v3
            // file."
            p.remove("api_key");
            p.remove("credential_id");
            p.insert("credential_reset_required".into(), json!(true));
            changed = true;
        }
        if changed {
            cfg["providers"] = json!(providers);
        }
        // `ai.py:102-103` setdefault, then `ai.py:141-147`'s legacy
        // `current.provider` -> `current.provider_id` rename.  Python has no
        // guard here: a non-object root raises `AttributeError` out of
        // `ensure_config` and reaches the route's 500, so this does the same
        // rather than silently rewriting the file.
        let Some(root) = cfg.as_object_mut() else {
            return Err(Error::Msg("ai_config_write_failed".to_string()));
        };
        root.entry("providers".to_string()).or_insert_with(|| json!([]));
        root.entry("current".to_string()).or_insert_with(|| json!({}));

        let providers_snapshot: Vec<Value> = root
            .get("providers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut current = root
            .get("current")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let has_provider = current
            .get("provider")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        let has_id = current
            .get("provider_id")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        if has_provider && !has_id {
            let legacy_name = current
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let match_id = providers_snapshot
                .iter()
                .find_map(|p| {
                    (p.get("name").and_then(|v| v.as_str()) == Some(legacy_name.as_str()))
                        .then(|| p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
                })
                .unwrap_or_default();
            current.insert(
                "provider_id".to_string(),
                json!(if match_id.is_empty() {
                    format!("preset:{legacy_name}")
                } else {
                    match_id
                }),
            );
            current.remove("provider");
            root.insert("current".to_string(), Value::Object(current));
            changed = true;
        }

        if changed {
            self.write_cfg(&cfg)?;
        }
        Ok(cfg)
    }

    /// `ai.py:226-303 save_config`, restricted to what this kernel persists.
    ///
    /// The invariant is `ai.py:294`'s unconditional `p.pop("api_key", None)`:
    /// every branch below ends with the secret either handed to the credential
    /// backend or dropped, never written.
    pub fn save_config(&self, payload: &Value) -> KernelResult<Value> {
        let mut cfg = self.ensure_config()?;
        if payload.get("providers").is_some() {
            let previous: Vec<Value> = cfg
                .get("providers")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let find_prev = |id: &str, name: &str| -> Value {
                previous
                    .iter()
                    .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(id))
                    .or_else(|| {
                        previous
                            .iter()
                            .find(|p| p.get("name").and_then(|v| v.as_str()) == Some(name))
                    })
                    .cloned()
                    .unwrap_or_else(|| json!({}))
            };

            let mut providers: Vec<Value> = Vec::new();
            let mut names: Vec<String> = Vec::new();
            let incoming = payload
                .get("providers")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for raw in incoming.into_iter().filter(|v| v.is_object()) {
                let Some(mut p) = raw.as_object().cloned() else { continue };
                let mut provider_id = p
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !provider_id.starts_with("custom:") {
                    provider_id = format!("custom:{}", uuid::Uuid::new_v4().simple());
                }
                let name = p
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() || names.contains(&name) {
                    continue;
                }
                names.push(name.clone());
                p.insert("name".into(), json!(name));
                p.insert("id".into(), json!(provider_id));
                p.insert("custom".into(), json!(true));

                let models: Vec<String> = p
                    .get("models")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(py_str)
                            .map(|m| m.trim().to_string())
                            .filter(|m| !m.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                p.insert("models".into(), json!(models));

                let mut endpoint_mode = p
                    .get("endpoint_mode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("prefix")
                    .trim()
                    .to_lowercase();
                if endpoint_mode != "prefix" && endpoint_mode != "full_url" {
                    endpoint_mode = "prefix".to_string();
                }
                p.insert("endpoint_mode".into(), json!(endpoint_mode));
                if !p.get("capabilities").map(|v| v.is_object()).unwrap_or(false) {
                    p.insert("capabilities".into(), json!({}));
                }
                let headers = sanitize_headers(p.get("headers"));
                p.insert("headers".into(), Value::Object(headers));

                let previous_provider = find_prev(&provider_id, &name);
                let api_key = p
                    .get("api_key")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if !api_key.is_empty() {
                    // `ai.py:268-269` — refuse, do not degrade.
                    if !self.credentials.is_crypto_available() {
                        return Err(Error::Msg(
                            "当前环境缺少凭据加密支持，拒绝以明文保存 API Key".to_string(),
                        ));
                    }
                    let candidate = p
                        .get("credential_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .filter(|s| is_valid_credential_id(s))
                        .or_else(|| {
                            previous_provider
                                .get("credential_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                                .filter(|s| is_valid_credential_id(s))
                        })
                        .unwrap_or_else(new_credential_id);
                    let backend = self.credentials.store(&candidate, &api_key)?;
                    p.insert("credential_id".into(), json!(candidate));
                    p.insert("credential_backend".into(), json!(backend));
                } else {
                    let had_key = !previous_provider
                        .get("api_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .is_empty()
                        || !previous_provider
                            .get("credential_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .is_empty();
                    let clear = p
                        .remove("clear_key")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if had_key && !clear {
                        let candidate = p
                            .get("credential_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .filter(|s| is_valid_credential_id(s))
                            .or_else(|| {
                                previous_provider
                                    .get("credential_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                                    .filter(|s| is_valid_credential_id(s))
                            })
                            .unwrap_or_else(new_credential_id);
                        let prev_had_handle = previous_provider
                            .get("credential_id")
                            .and_then(|v| v.as_str())
                            .map(|s| !s.is_empty())
                            .unwrap_or(false);
                        let prev_api_key = previous_provider
                            .get("api_key")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if prev_had_handle && prev_api_key.is_empty() {
                            p.insert("credential_id".into(), json!(candidate));
                            p.insert(
                                "credential_backend".into(),
                                json!(previous_provider
                                    .get("credential_backend")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("native")),
                            );
                        } else {
                            let secret = if prev_api_key.starts_with("enc:") {
                                crate::crypto::decrypt_api_key(&prev_api_key, None).unwrap_or_default()
                            } else {
                                String::new()
                            };
                            if secret.is_empty() {
                                // `ai.py:284` — a key this side cannot unwrap is
                                // dropped, never written out in the clear.
                                return Err(Error::Msg(
                                    "旧凭据无法安全迁移，请重新输入 API Key".to_string(),
                                ));
                            }
                            let backend = self.credentials.store(&candidate, &secret)?;
                            p.insert("credential_id".into(), json!(candidate));
                            p.insert("credential_backend".into(), json!(backend));
                        }
                    } else {
                        let old_credential = previous_provider
                            .get("credential_id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();
                        let old_credential = if old_credential.is_empty() {
                            p.get("credential_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string()
                        } else {
                            old_credential
                        };
                        if !old_credential.is_empty() {
                            self.credentials.delete(&old_credential)?;
                        }
                        p.remove("credential_id");
                        p.remove("credential_backend");
                    }
                }
                // `ai.py:294` — the unconditional redaction that closes every
                // branch above.
                p.remove("api_key");
                providers.push(Value::Object(p));
            }
            cfg["providers"] = json!(providers);
        }
        if payload.get("current").is_some() {
            let current = payload.get("current").cloned().unwrap_or_else(|| json!({}));
            let provider_id = current
                .get("provider_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    current
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or_default();
            cfg["current"] = json!({
                "provider_id": provider_id,
                "model": current.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            });
        }
        cfg["schema_version"] = json!(CONFIG_SCHEMA_VERSION);
        self.write_cfg(&cfg)?;
        Ok(cfg)
    }

    /// `ai.py:171-202`'s `annotate()`, i.e. the one provider as the client sees
    /// it: status flags plus the opaque handle, and never a key.
    pub fn annotated_provider(&self, provider: &Value) -> Value {
        let mut d = provider.as_object().cloned().unwrap_or_default();
        let name = d
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("provider")
            .to_string();
        d.entry("id".to_string())
            .or_insert_with(|| json!(format!("preset:{name}")));
        let resolved_key = self.credentials.resolve_key(&Value::Object(d.clone()));
        let has_key = !resolved_key.is_empty();
        d.insert("has_key".into(), json!(has_key));
        d.insert(
            "key_source".into(),
            json!(self.credentials.key_source(&Value::Object(d.clone()))),
        );
        let format_is_anthropic = d.get("format").and_then(|v| v.as_str()) == Some("anthropic");
        let mode = d
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or(if format_is_anthropic { "messages" } else { "auto" })
            .to_string();
        d.insert("mode".into(), json!(mode));
        let default_category = if d.get("custom").and_then(|v| v.as_bool()).unwrap_or(false) {
            "custom"
        } else {
            "preset"
        };
        d.entry("category".to_string())
            .or_insert_with(|| json!(default_category));
        d.entry("endpoint_mode".to_string()).or_insert_with(|| json!("prefix"));
        d.entry("website".to_string()).or_insert_with(|| json!(""));
        // `ai.py:194-197`: a handle whose secret is gone is not advertised, so
        // the UI cannot claim a provider is ready and then fail on send.
        if d.get("credential_id").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false)
            && has_key
        {
        } else {
            d.remove("credential_id");
        }
        d.entry("capabilities".to_string()).or_insert_with(|| json!({}));
        // `ai.py:200-201` — the reason this module owns the secret boundary.
        d.remove("api_key");
        Value::Object(d)
    }

    /// `ai.py:167-223 get_config()`'s top-level dict: exactly
    /// [`CONFIG_GET_KEYS`].
    pub fn config_view(&self, presets: Vec<Value>, upstream_catalog: Vec<Value>) -> KernelResult<Value> {
        let cfg = self.ensure_config()?;
        let custom: Vec<Value> = cfg
            .get("providers")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|p| self.annotated_provider(&p))
            .collect();
        let presets: Vec<Value> = presets.into_iter().map(|p| self.annotated_provider(&p)).collect();
        let upstream: Vec<Value> = upstream_catalog
            .into_iter()
            .map(|source| {
                let mut item = self.annotated_provider(&source);
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("selectable".into(), json!(false));
                    obj.insert("has_key".into(), json!(false));
                    obj.insert("credential_id".into(), json!(""));
                    obj.remove("credential_id");
                }
                item
            })
            .collect();
        Ok(json!({
            "schema_version": CONFIG_SCHEMA_VERSION,
            "presets": presets,
            "custom": custom,
            "upstream_catalog": upstream,
            "current": cfg.get("current").cloned().unwrap_or_else(|| json!({})),
        }))
    }
}

// ---------------------------------------------------------------------------
// AI route envelopes: returned vs. raised (mirrors `parity_pets::PetOutcome`)
// ---------------------------------------------------------------------------
//
// `parity_pets.rs:2412` already models the distinction the AI routes need:
// Python answers some failures from *inside* the api method and lets others
// escape into the route's `except Exception`.  Collapsing both into "an
// `ApiError`" loses the difference, because the two paths carry different key
// sets.  This is the AI copy of that idea, so no third convention appears.

/// One of the three AI routes this module backs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiRoute {
    Config,
    Models,
    Chat,
}

impl AiRoute {
    /// The `(status, error_code)` the route's generic `except Exception`
    /// answers with: `readmd.py:2328-2330`, `readmd.py:2376-2378`,
    /// `readmd.py:2449-2453`.
    ///
    /// Note the deliberate asymmetry Python keeps: a config failure is a 500,
    /// a model-list failure is a **400**, and a chat failure is a **502** —
    /// none of them a bare 500, and none of them carry the exception text.
    pub const fn failure(self) -> (u16, &'static str) {
        match self {
            AiRoute::Config => (500, "ai_config_failed"),
            AiRoute::Models => (400, "model_list_failed"),
            AiRoute::Chat => (502, "provider_error"),
        }
    }

    /// The `ApiError` for the *raised* branch.  `detail` is diagnostics only:
    /// `crate::error::ApiError::noted("detail", ..)` keeps it out of the body,
    /// which is what `_send_api_error` guarantees (`readmd.py:1421-1430`,
    /// "without leaking host details").
    pub fn raised_error(self, detail: impl Into<String>) -> ApiError {
        let (status, code) = self.failure();
        ApiError::new(status, code).noted("detail", detail)
    }

    /// Map a kernel-level `Result` onto the Python branch it belongs to.
    ///
    /// `Ok(value)` and an in-method guard become a [`AiOutcome::Returned`] with
    /// the status Python uses; everything else is [`AiOutcome::Raised`] and the
    /// caller answers with [`AiRoute::failure`] — the same contract
    /// `parity_pets::PetOutcome` gives its route.
    pub fn outcome(self, result: KernelResult<Value>) -> AiOutcome {
        match result {
            Ok(payload) => AiOutcome::Returned {
                status: 200,
                payload,
            },
            Err(e) => match self.envelope_for(&e) {
                Some((status, payload)) => AiOutcome::Returned { status, payload },
                None => AiOutcome::Raised,
            },
        }
    }

    /// Failures Python answers from *inside* the method, with the status and
    /// key set it uses.  Anything else raises.
    fn envelope_for(self, error: &Error) -> Option<(u16, Value)> {
        let text = error.to_string();
        match self {
            // `readmd.py:2386-2399`: three guards run before `ai.chat()`, each
            // answering 400 with its own body rather than raising.
            AiRoute::Chat => {
                if let Some(message) = chat_guard_message(&text) {
                    Some((
                        400,
                        json!({"ok": false, "error_code": "invalid_request", "error": message}),
                    ))
                } else if text.contains("skill_required") {
                    Some((400, api_error_envelope("skill_required")))
                } else {
                    None
                }
            }
            // `readmd.py:2346-2348` / `2358-2360` are the *legacy* `{'error':
            // text}` shape, not `ok`/`error_code`; they stay with the handler
            // and are pinned by [`MODELS_GUARD_REJECTIONS`].
            _ => None,
        }
    }

    /// The success envelope's key set, straight out of the Python source.
    pub const fn success_keys(self) -> &'static [&'static str] {
        match self {
            AiRoute::Config => CONFIG_SAVE_KEYS,
            AiRoute::Models => MODELS_OK_KEYS,
            AiRoute::Chat => CHAT_JSON_KEYS,
        }
    }
}

/// `readmd.py:2322` — `mod.get_config()`, i.e. the 5-key GET body.  There is
/// deliberately **no** `ok` key here; a kernel that adds one diverges.
pub const CONFIG_GET_KEYS: &[&str] = &[
    "schema_version",
    "presets",
    "custom",
    "upstream_catalog",
    "current",
];
/// `readmd.py:2327` — the POST success body.
pub const CONFIG_SAVE_KEYS: &[&str] = &["ok"];
/// `readmd.py:2375` — `{'models': ids}`, again with no `ok` key.
pub const MODELS_OK_KEYS: &[&str] = &["models"];
/// `readmd.py:2421` — `{'ok': True, 'content': ...}`; `usage` is added only
/// when the provider reported it (`readmd.py:2422-2423`).
pub const CHAT_JSON_KEYS: &[&str] = &["ok", "content"];
pub const CHAT_JSON_OPTIONAL_KEYS: &[&str] = &["usage"];
/// `readmd.py:2432-2448` — the SSE event types `/api/ai/chat` may emit.
pub const CHAT_SSE_TYPES: &[&str] = &["meta", "delta", "usage", "error", "done"];

/// The two `/api/ai/models` guards the handler answers *itself*, with the
/// pre-`error_code` body and — note the asymmetry — two **different** statuses:
/// `readmd.py:2347` rejects a key in the query string at 400, and
/// `readmd.py:2359` rejects a credential/provider mismatch at **403**.
pub const MODELS_GUARD_REJECTIONS: &[(u16, &str)] = &[
    (400, "API Key 不得出现在 URL，请使用 POST 请求体"),
    (403, "凭据与提供商不匹配"),
];

/// A `{'ok': False, 'error_code': ...}` body, i.e. `_send_api_error`'s shape.
pub fn api_error_envelope(code: &str) -> Value {
    json!({"ok": false, "error_code": code})
}

/// `readmd.py:2347` and `readmd.py:2359` reject two `/api/ai/models` requests
/// with the pre-`error_code` body `{'error': text}` — no `ok`, no
/// `error_code`, and at two different statuses
/// ([`MODELS_GUARD_REJECTIONS`]).  Named here so a handler that "helpfully"
/// upgrades them to `_send_api_error` shows up as a parity diff instead of
/// looking correct.
pub fn legacy_error_envelope(text: &str) -> Value {
    json!({"error": text})
}

/// The exact guard text `readmd.py:2388` / `:2392` put in `error`, or `None`
/// when the failure is not one of the two `invalid_request` guards.
///
/// The body must carry Python's own wording: the client displays it verbatim.
fn chat_guard_message(text: &str) -> Option<&'static str> {
    if text.contains("请求体必须是 JSON 对象") || text.contains("payload_not_object") {
        Some("请求体必须是 JSON 对象")
    } else if text.contains("请求格式错误") || text.contains("invalid_request") {
        Some("请求格式错误")
    } else {
        None
    }
}

/// `readmd.py:2375`.
pub fn models_envelope(ids: Vec<String>) -> Value {
    json!({"models": ids})
}

/// `readmd.py:2421-2424`.
pub fn chat_json_envelope(content: &str, usage: Option<Value>) -> Value {
    let mut body = json!({"ok": true, "content": content});
    if let (Some(obj), Some(usage)) = (body.as_object_mut(), usage) {
        obj.insert("usage".to_string(), usage);
    }
    body
}

/// Returned-vs-raised for one AI request, mirroring `parity_pets::PetOutcome`.
#[derive(Debug, Clone, PartialEq)]
pub enum AiOutcome {
    /// The api method answered with `_send_json(status, payload)` itself —
    /// including the failures Python answers at a fixed status, e.g. a
    /// provider error dict surfacing as `502 provider_error`
    /// (`readmd.py:2416-2418`).
    Returned { status: u16, payload: Value },
    /// An exception escaped the api method's `try` and reaches the route's
    /// `except Exception`, which emits [`AiRoute::failure`].
    Raised,
}

impl AiOutcome {
    /// The 200 body, for the callers that swallow the raise — `PetOutcome::value`
    /// with the same role.
    pub fn payload(&self) -> Option<&Value> {
        match self {
            AiOutcome::Returned { payload, .. } => Some(payload),
            AiOutcome::Raised => None,
        }
    }

    pub fn status(&self) -> Option<u16> {
        match self {
            AiOutcome::Returned { status, .. } => Some(*status),
            AiOutcome::Raised => None,
        }
    }

    /// `{'ok': True, ...}` at 200.
    pub fn ok(payload: Value) -> Self {
        AiOutcome::Returned { status: 200, payload }
    }

    pub fn raised() -> Self {
        AiOutcome::Raised
    }

    /// Whether this outcome is a 200 that still reports a failure, the shape
    /// the parity harness has to distinguish from a real success.
    pub fn is_ok_envelope(&self) -> bool {
        self.payload()
            .and_then(|v| v.get("ok"))
            .and_then(|v| v.as_bool())
            == Some(true)
    }
}

// --------------------------------------------------------------------------
// OS locale probe
// --------------------------------------------------------------------------
//
// Port of `src/readmd_core/config.py:55 get_system_language()`, the only
// language-detection helper the desktop-pet configure path consumes
// (`readmd.py:5073` -> `_pet_locale_line_pack`, and `parity_pets::pet_locale`).
// It is a *locale* probe, not the `language` setting served by
// `/api/system/language`; the two must not be conflated.

/// `config.py:71-79` — Python's `lang_map`, transcribed in source order with
/// the same keys and the same lower-case bare-subtag values.  Chinese is absent
/// because `config.py:63` handles it before the map is consulted.
const SYSTEM_LANGUAGE_PRIMARY_MAP: &[(u16, &str)] = &[
    (0x09, "en"),
    (0x11, "ja"),
    (0x12, "ko"),
    (0x0c, "fr"),
    (0x07, "de"),
    (0x0a, "es"),
    (0x16, "pt"),
    (0x19, "ru"),
    (0x10, "it"),
    (0x01, "ar"),
    (0x0d, "he"),
    (0x1e, "th"),
    (0x2a, "vi"),
    (0x21, "id"),
    (0x39, "hi"),
    (0x45, "bn"),
    (0x55, "my"),
    (0x54, "lo"),
    (0x53, "km"),
    (0x3e, "ms"),
    (0x06, "da"),
    (0x0b, "fi"),
    (0x14, "no"),
    (0x1d, "sv"),
    (0x13, "nl"),
    (0x1a, "hr"),
    (0x18, "ro"),
    (0x61, "ne"),
    (0x24, "sl"),
    (0x1f, "tr"),
    (0x22, "uk"),
    (0x08, "el"),
    (0x0e, "hu"),
];

/// `config.py:63-70` — the `PRIMARYLANGID == 0x04` branch, same test order and
/// same return values.  Python's inline comment mislabels `0x04` as `zh-SG`
/// while returning `'zh-HK'` for it; the *behaviour* is what is mirrored here.
fn chinese_language(sub: u16) -> &'static str {
    if sub == 0x02 {
        "zh-CN"
    } else if sub == 0x01 || sub == 0x04 {
        if sub == 0x01 {
            "zh-TW"
        } else {
            "zh-HK"
        }
    } else if sub == 0x03 {
        "zh-HK"
    } else {
        "zh-CN"
    }
}

/// `config.py:58-81`: `ctypes.windll.kernel32.GetUserDefaultUILanguage()` and
/// the `PRIMARYLANGID` / `SUBLANGID` decode.
///
/// `None` means "keep going", which is exactly Python's control flow when the
/// primary id is neither Chinese nor present in `lang_map` (the `try` block then
/// ends without a `return`), and on every non-Windows platform, where
/// `sys.platform == 'win32'` is false and the branch is skipped whole.  The
/// `ctypes` call itself is the thing Python wraps in `except Exception: pass`;
/// `GetUserDefaultUILanguage` cannot fail in a way that raises, and a `0`
/// LANGID lands in the `None` fall-through like Python's would.
fn ui_language_code() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        let lang_id = unsafe { GetUserDefaultUILanguage() };
        let primary = lang_id & 0x3ff;
        let sub = (lang_id >> 10) & 0x3f;
        if primary == 0x04 {
            return Some(chinese_language(sub).to_string());
        }
        if let Some((_, code)) = SYSTEM_LANGUAGE_PRIMARY_MAP
            .iter()
            .find(|(id, _)| *id == primary)
        {
            return Some((*code).to_string());
        }
    }
    None
}

/// `locale.getdefaultlocale()[0]` for the values the app ships bundles for.
///
/// CPython resolves that as: env lookup over
/// `envvars=('LC_ALL', 'LC_CTYPE', 'LANG', 'LANGUAGE')` with the first non-empty
/// winner and `LANGUAGE` truncated at its first `:`; when nothing is set the
/// name is `'C'`.  `_parse_localename` then strips any `@modifier`, splits on the
/// first `.` and keeps the language part, and maps the bare names `'C'` and
/// `'UTF-8'` to *no* language code (`None`).  `locale.normalize()`'s alias table
/// is deliberately not reproduced: it only rewrites archaic spellings such as
/// `'english'`, and for every `xx_YY[.ENC]` value the identity mapping yields the
/// same language subtag.
///
/// Windows is a second, smaller deviation: CPython prefers `_locale._getdefaultlocale()`
/// (a `GetUserDefaultLangID` + `locale.windows_locale` lookup) over the env path.
/// That difference is not observable through this function, because `ui_language_code`
/// already answers from the UI LANGID on Windows, and both routes reduce to the same
/// app code for every locale `assets/i18n` carries a bundle for.
fn locale_env_localename() -> Option<String> {
    for name in ["LC_ALL", "LC_CTYPE", "LANG", "LANGUAGE"] {
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        let value = if name == "LANGUAGE" {
            value.split(':').next().unwrap_or_default().to_string()
        } else {
            value
        };
        if !value.is_empty() {
            return Some(value);
        }
    }
    // Nothing set: CPython uses 'C', which `_parse_localename` turns into
    // `(None, None)` — i.e. no language code — so the caller falls through.
    None
}

/// `_parse_localename` + `config.py:87-90` for one raw locale name.
/// Split out from [`locale_env_localename`] so it is testable without mutating
/// the process environment.
fn language_from_localename(localename: &str) -> Option<String> {
    let mut code = localename;
    if let Some((head, _modifier)) = code.split_once('@') {
        code = head;
    }
    if let Some((head, _encoding)) = code.split_once('.') {
        code = head;
    }
    if code.is_empty() || code == "C" || code == "UTF-8" {
        return None;
    }
    Some(app_language_code(code))
}

/// `config.py:87-90` verbatim: `'_'` -> `'-'`, then the Chinese special case,
/// then the primary subtag.
fn app_language_code(loc: &str) -> String {
    let loc = loc.replace('_', "-");
    if loc.starts_with("zh") {
        if loc.contains("HK") || loc.contains("Hant") {
            return "zh-HK".to_string();
        }
        if loc.contains("TW") {
            return "zh-TW".to_string();
        }
        return "zh-CN".to_string();
    }
    loc.split('-').next().unwrap_or_default().to_string()
}

/// `src/readmd_core/config.py:55 get_system_language()` — the OS default *UI*
/// language as the app's locale code: `'zh-CN'`, `'zh-TW'`, `'zh-HK'`, `'en'`,
/// `'ja'`, `'ko'`, …
///
/// Python swallows every exception in this function (`config.py:82-83`,
/// `config.py:91-92`) and its last resort is the literal `'zh-CN'`
/// (`config.py:93`), so it never raises and never yields `'en'` by accident.
/// The `Err` arm exists only to match the `Result` shape its caller already
/// handles; it is unreachable by construction, mirroring how
/// `readmd.py:5074-5075`'s `except` -> `'en'` is likewise unreachable.
pub fn system_language() -> crate::Result<String> {
    if let Some(code) = ui_language_code() {
        return Ok(code);
    }
    if let Some(code) = locale_env_localename().as_deref().and_then(language_from_localename) {
        return Ok(code);
    }
    Ok("zh-CN".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_catalog() {
        let catalog = ProviderCatalog::new();
        assert_eq!(catalog.provider_ids().len(), 7);
        assert!(catalog.has_provider("openai"));
        assert!(catalog.has_provider("anthropic"));
        assert!(catalog.has_provider("ollama"));
    }

    #[test]
    fn test_message_creation() {
        let user_msg = Message::user("Hello, world!");
        if let Message::User { content, .. } = user_msg {
            assert_eq!(content, "Hello, world!");
        } else {
            panic!("Expected User message");
        }

        let assistant_msg = Message::assistant("Hi there!");
        if let Message::Assistant { content } = assistant_msg {
            assert_eq!(content, "Hi there!");
        } else {
            panic!("Expected Assistant message");
        }
    }

    #[test]
    fn test_chat_request_builder() {
        let request = ChatCompletionRequest::new("gpt-4", vec![Message::user("Test")])
            .with_temperature(0.9)
            .with_max_tokens(100)
            .with_stream(true);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.temperature, Some(0.9));
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.stream, Some(true));
    }

    // ---- R3-1: the AI settings file must never carry a secret --------------

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    /// A literal that is unmistakably not a credential, so a leak in a test
    /// artifact can never be mistaken for a real key.
    const FAKE_KEY: &str = "sk-FAKE-not-a-real-key-0123456789abcdef";
    const FAKE_CID: &str = "cred:fake0001";

    /// The `crypto.py` credential API without the OS store or the
    /// process-wide `READMD_DATA_DIR`, so the secret-boundary tests cannot
    /// race `crypto.rs`' own data-dir tests or touch a real keychain.
    #[derive(Default)]
    struct MemoryBackend {
        available: bool,
        secrets: Mutex<HashMap<String, String>>,
        stores: Mutex<Vec<(String, String)>>,
        deletes: Mutex<Vec<String>>,
    }

    impl MemoryBackend {
        fn online() -> Arc<MemoryBackend> {
            Arc::new(MemoryBackend {
                available: true,
                ..Default::default()
            })
        }
        fn offline() -> Arc<MemoryBackend> {
            Arc::new(MemoryBackend {
                available: false,
                ..Default::default()
            })
        }
        fn store_calls(&self) -> Vec<(String, String)> {
            self.stores.lock().unwrap().clone()
        }
        fn delete_calls(&self) -> Vec<String> {
            self.deletes.lock().unwrap().clone()
        }
    }

    impl SecretBackend for MemoryBackend {
        fn store(&self, credential_id: &str, secret: &str) -> KernelResult<String> {
            if secret.is_empty() {
                return Err(Error::Crypto("credential secret is empty".to_string()));
            }
            if !is_valid_credential_id(credential_id) {
                return Err(Error::Crypto("invalid credential id".to_string()));
            }
            self.stores
                .lock()
                .unwrap()
                .push((credential_id.to_string(), secret.to_string()));
            self.secrets
                .lock()
                .unwrap()
                .insert(credential_id.to_string(), secret.to_string());
            Ok("encrypted-vault".to_string())
        }
        fn load(&self, credential_id: &str) -> KernelResult<String> {
            Ok(self
                .secrets
                .lock()
                .unwrap()
                .get(credential_id)
                .cloned()
                .unwrap_or_default())
        }
        fn delete(&self, credential_id: &str) -> KernelResult<()> {
            self.deletes.lock().unwrap().push(credential_id.to_string());
            self.secrets.lock().unwrap().remove(credential_id);
            Ok(())
        }
        fn available(&self) -> bool {
            self.available
        }
    }

    /// Lets a test keep observing the backend after handing it to a store.
    impl<B: SecretBackend + ?Sized> SecretBackend for Arc<B> {
        fn store(&self, c: &str, s: &str) -> KernelResult<String> {
            (**self).store(c, s)
        }
        fn load(&self, c: &str) -> KernelResult<String> {
            (**self).load(c)
        }
        fn delete(&self, c: &str) -> KernelResult<()> {
            (**self).delete(c)
        }
        fn available(&self) -> bool {
            (**self).available()
        }
    }

    fn settings_in(dir: &Path, backend: Arc<MemoryBackend>) -> AiSettings<Arc<MemoryBackend>> {
        AiSettings::new(
            dir.join(AI_CONFIG_FILE_NAME),
            CredentialStore::with_backend(backend),
        )
    }

    fn on_disk(dir: &Path) -> String {
        std::fs::read_to_string(dir.join(AI_CONFIG_FILE_NAME)).unwrap_or_default()
    }

    fn sorted_keys(value: &Value) -> Vec<String> {
        let mut keys: Vec<String> = value
            .as_object()
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        keys
    }

    /// The bug R3-1 reported: the store wrote `{"<provider>": "<key>"}` into a
    /// plaintext `credentials.json`.  This is the test that would have caught
    /// it, and it is the invariant every branch below has to keep.
    #[test]
    fn a_saved_api_key_never_appears_in_the_settings_file() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::online();
        let store = settings_in(dir.path(), backend.clone());

        store
            .save_config(&json!({"providers": [{
                "name": "Relay",
                "base_url": "https://relay.invalid/v1",
                "api_key": FAKE_KEY,
            }]}))
            .expect("save_config");

        let text = on_disk(dir.path());
        assert!(
            !text.contains(FAKE_KEY),
            "ai.json carried the secret in the clear"
        );
        assert!(!text.contains("api_key"), "ai.json still has an api_key field");
        // What Python persists instead: the opaque handle plus the backend tag
        // (`ai.py:272-273`).
        assert!(text.contains("\"credential_id\""), "handle missing: {text}");
        assert!(text.contains("encrypted-vault"), "backend tag missing: {text}");

        // The secret went to exactly one place: the credential backend.
        let calls = backend.store_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, FAKE_KEY);

        // ...and the key still resolves, through the handle.
        let cfg = store.ensure_config().unwrap();
        assert_eq!(
            store.credentials().resolve_key(&cfg["providers"][0]),
            FAKE_KEY
        );
    }

    /// `ai.py:134-140` — "Never keep a plaintext legacy key in the v3 file".
    /// A pre-existing key is deleted and the provider is flagged for a fresh
    /// one; it is *not* migrated (Python cannot tell it was ever valid) and it
    /// is never read back.
    #[test]
    fn a_legacy_plaintext_key_on_disk_is_deleted_not_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::online();
        std::fs::write(
            dir.path().join(AI_CONFIG_FILE_NAME),
            json!({
                "schema_version": 3,
                "providers": [{"id": "custom:legacy", "name": "Legacy", "api_key": FAKE_KEY}],
                "current": {},
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let store = settings_in(dir.path(), backend.clone());

        let cfg = store.ensure_config().unwrap();

        let text = on_disk(dir.path());
        assert!(!text.contains(FAKE_KEY), "legacy key survived the migration");
        assert!(backend.store_calls().is_empty(), "plaintext must not be stored");
        let provider = &cfg["providers"][0];
        assert_eq!(provider.get("api_key"), None);
        assert_eq!(provider.get("credential_id"), None);
        assert_eq!(provider["credential_reset_required"], json!(true));
        assert_eq!(store.credentials().resolve_key(provider), "");
    }

    /// `ai.py:267-269` — no crypto backend means a *refusal*, not a plaintext
    /// fallback.  This is the "never weaker on secrets" clause.
    #[test]
    fn save_refuses_a_key_when_no_credential_backend_is_available() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::offline();
        let store = settings_in(dir.path(), backend.clone());

        let error = store
            .save_config(&json!({"providers": [{"name": "No Crypto", "api_key": FAKE_KEY}]}))
            .expect_err("a plaintext write must be refused");
        assert!(
            error.to_string().contains("明文"),
            "unexpected refusal: {error}"
        );
        assert!(!on_disk(dir.path()).contains(FAKE_KEY));
        assert!(backend.store_calls().is_empty());
    }

    /// `ai.py:287-293` — an explicit clear deletes the secret from the store;
    /// editing other fields without the flag keeps it (`ai.py:274-279`).
    #[test]
    fn clear_key_removes_the_secret_and_keeps_it_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::online();
        let store = settings_in(dir.path(), backend.clone());
        store
            .save_config(&json!({"providers": [{"name": "Relay", "api_key": FAKE_KEY}]}))
            .unwrap();
        let cfg = store.ensure_config().unwrap();
        let provider_id = cfg["providers"][0]["id"].as_str().unwrap().to_string();
        let cid = cfg["providers"][0]["credential_id"].as_str().unwrap().to_string();

        // Edit another field: the handle must survive and nothing is deleted.
        store
            .save_config(&json!({"providers": [{
                "id": provider_id,
                "name": "Relay",
                "base_url": "https://moved.invalid/v1",
            }]}))
            .unwrap();
        let kept = store.ensure_config().unwrap();
        assert_eq!(kept["providers"][0]["credential_id"], json!(cid));
        assert!(backend.delete_calls().is_empty());
        assert_eq!(
            store.credentials().resolve_key(&kept["providers"][0]),
            FAKE_KEY,
            "editing base_url must not lose the key"
        );

        // Explicit clear: the credential goes, and so does the handle.
        store
            .save_config(&json!({"providers": [{
                "id": provider_id,
                "name": "Relay",
                "clear_key": true,
            }]}))
            .unwrap();
        assert_eq!(backend.delete_calls(), vec![cid.clone()]);
        let cleared = store.ensure_config().unwrap();
        assert_eq!(cleared["providers"][0].get("credential_id"), None);
        assert_eq!(store.credentials().resolve_key(&cleared["providers"][0]), "");
        assert!(!on_disk(dir.path()).contains(FAKE_KEY));
    }

    /// `ai.py:200-201` — the config endpoint reports status only.
    #[test]
    fn config_view_exposes_status_flags_and_never_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::online();
        let store = settings_in(dir.path(), backend.clone());
        store
            .save_config(&json!({"providers": [{"name": "Relay", "api_key": FAKE_KEY}]}))
            .unwrap();

        let view = store.config_view(vec![], vec![]).unwrap();
        let text = serde_json::to_string(&view).unwrap();
        assert!(!text.contains(FAKE_KEY), "config response leaked the key");

        let provider = &view["custom"][0];
        assert_eq!(provider["has_key"], json!(true));
        assert_eq!(provider["key_source"], json!("configured"));
        assert_eq!(provider.get("api_key"), None);
        assert!(
            provider["credential_id"].as_str().unwrap().starts_with("cred:"),
            "only the opaque handle is exposed"
        );
    }

    /// `ai.py:190-197` — a handle left over from a removed OS credential must
    /// not be advertised as configured, or every UI surface claims success and
    /// the send fails.
    #[test]
    fn a_handle_without_a_secret_is_not_advertised() {
        let dir = tempfile::tempdir().unwrap();
        // Backend online but empty: the credential is gone.
        let backend = MemoryBackend::online();
        std::fs::write(
            dir.path().join(AI_CONFIG_FILE_NAME),
            json!({
                "schema_version": 3,
                "providers": [{
                    "id": "custom:stale",
                    "name": "Stale",
                    "credential_id": FAKE_CID,
                    "credential_backend": "native",
                }],
                "current": {},
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let store = settings_in(dir.path(), backend);

        let view = store.config_view(vec![], vec![]).unwrap();
        let provider = &view["custom"][0];
        assert_eq!(provider["has_key"], json!(false));
        assert_eq!(provider["key_source"], json!(""));
        assert_eq!(
            provider.get("credential_id"),
            None,
            "a stale handle must not be returned"
        );
    }

    /// `ai.py:253-264` — a header named like a credential is dropped, so a key
    /// cannot be smuggled into the settings file under another field.
    #[test]
    fn custom_headers_cannot_smuggle_a_key_into_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let backend = MemoryBackend::online();
        let store = settings_in(dir.path(), backend);
        store
            .save_config(&json!({"providers": [{
                "name": "Relay",
                "headers": {
                    "Authorization": "Bearer smuggled",
                    "X-Api-Key": "smuggled",
                    "My-Token": "smuggled",
                    "X-Trace-Id": "kept",
                },
            }]}))
            .unwrap();

        let text = on_disk(dir.path());
        assert!(!text.contains("smuggled"), "a credential-shaped header persisted");
        let cfg = store.ensure_config().unwrap();
        assert_eq!(
            sorted_keys(&cfg["providers"][0]["headers"]),
            vec!["X-Trace-Id".to_string()]
        );
    }

    #[test]
    fn credential_id_gate_matches_cryptos_regex() {
        // crypto.py:21 — `^cred:[A-Za-z0-9_-]{8,128}$`.
        assert!(is_valid_credential_id(FAKE_CID));
        assert!(is_valid_credential_id("cred:A0z_-9876543210"));
        assert!(is_valid_credential_id(&format!("cred:{}", "x".repeat(128))));
        assert!(!is_valid_credential_id(&format!("cred:{}", "x".repeat(129))));
        assert!(!is_valid_credential_id("cred:short7")); // 7 chars
        assert!(!is_valid_credential_id("abcd1234")); // no `cred:` prefix
        assert!(!is_valid_credential_id("cred:not ok!")); // non-ASCII class
        assert!(!is_valid_credential_id(""));
    }

    /// The production backend must reject the same inputs `crypto.py` rejects,
    /// *before* touching any store — these two calls never reach the disk.
    #[test]
    fn os_backend_fails_closed_on_an_unusable_credential() {
        let store = CredentialStore::os_backend();
        assert!(store.is_crypto_available(), "the kernel always ships Fernet");
        assert!(store
            .store(FAKE_CID, "")
            .unwrap_err()
            .to_string()
            .contains("credential secret is empty"));
        assert!(store
            .store("not-a-credential-id", FAKE_KEY)
            .unwrap_err()
            .to_string()
            .contains("invalid credential id"));
        assert_eq!(store.load(""), "");
        store.delete("").unwrap();
    }

    #[test]
    fn test_credential_store_roundtrip() {
        let backend = MemoryBackend::online();
        let store = CredentialStore::with_backend(backend.clone());

        let cid = new_credential_id();
        assert!(is_valid_credential_id(&cid), "uuid hex handle must be valid");
        store.store(&cid, FAKE_KEY).unwrap();
        assert_eq!(store.load(&cid), FAKE_KEY);

        store.delete(&cid).unwrap();
        assert_eq!(store.load(&cid), "");
        assert_eq!(backend.delete_calls(), vec![cid]);
    }

    // ---- R3-2: on-disk names have to match Python byte for byte -------------

    #[test]
    fn ai_settings_paths_use_pythons_exact_names() {
        // ai.py:23, crypto.py:36, crypto.py:107.
        assert_eq!(AI_CONFIG_FILE_NAME, "ai.json");
        assert_eq!(ENCRYPTION_KEY_FILE_NAME, "encryption.key");
        assert_eq!(CREDENTIALS_VAULT_FILE_NAME, "credentials.vault");
        assert_eq!(CREDENTIAL_SERVICE_NAME, "ReadMD");
        assert_eq!(CONFIG_SCHEMA_VERSION, 3);

        // The old store invented `credentials.json`, a name Python never
        // writes; `at_default_location` must not resurrect it.
        let settings = AiSettings::at_default_location();
        assert_eq!(settings.path().file_name().unwrap(), "ai.json");
        // ...and it lives in the same directory as the credential vault, so
        // the settings file and the secrets cannot diverge.
        assert_eq!(
            settings.path().parent().unwrap(),
            crate::paths::data_dir().as_path()
        );
    }

    /// R3-2's casing claim, pinned rather than argued.
    ///
    /// Python creates `%APPDATA%\ReadMD` / `~/.local/share/ReadMD`
    /// (`config.py:39`/`:41`/`:43`); the kernel derives
    /// `crate::paths::data_dir()` as lowercase `readmd` (`lib.rs:316`), while
    /// `parity_pets.rs:2833` already uses `ReadMD`.  On Windows the two
    /// spellings open the same directory (case-insensitive, case-preserving),
    /// so whichever process creates it first fixes the name on disk; on
    /// Linux/macOS they are genuinely different directories, which would split
    /// the Rust kernel's `ai.json`/`credentials.vault` away from Python's.
    ///
    /// The one-line fix belongs in `lib.rs::paths::data_dir`, which WD6 does
    /// not own.  Until then this test keeps the *deviation itself* pinned: an
    /// unexpected third spelling fails it, and once `lib.rs` is fixed the
    /// `known_deviations` report below goes quiet.
    #[test]
    fn python_data_dir_name_matches_kernel_base() {
        if std::env::var("READMD_DATA_DIR").is_ok() {
            // An explicit override is honoured verbatim by both sides
            // (`config.py:27-37`), so there is nothing to compare.
            return;
        }
        let base = crate::paths::data_dir();
        let component = base
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        assert!(
            !component.is_empty(),
            "data_dir has no app component: {}",
            base.display()
        );
        assert_eq!(
            component.to_lowercase(),
            PYTHON_DATA_DIR_NAME.to_lowercase(),
            "R3-2: kernel data dir {component:?} is not Python's directory even case-insensitively"
        );
        let known_deviations = ["readmd"];
        if component != PYTHON_DATA_DIR_NAME {
            assert!(
                known_deviations.contains(&component.as_str()),
                "R3-2: unexpected data dir spelling {component:?}; Python uses {PYTHON_DATA_DIR_NAME:?}"
            );
            eprintln!(
                "R3-2 KNOWN DEVIATION: crate::paths::data_dir() ends in {component:?}, \
                 config.py uses {PYTHON_DATA_DIR_NAME:?} (fix: lib.rs:316)"
            );
        }
    }

    // ---- R3-3: envelope and error-code fidelity on the AI routes ------------

    #[test]
    fn route_failure_codes_are_the_python_handlers() {
        // readmd.py:2330 / :2378 / :2452 — note 400 and 502, not three 500s.
        assert_eq!(AiRoute::Config.failure(), (500, "ai_config_failed"));
        assert_eq!(AiRoute::Models.failure(), (400, "model_list_failed"));
        assert_eq!(AiRoute::Chat.failure(), (502, "provider_error"));
    }

    #[test]
    fn config_get_body_has_exactly_pythons_five_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = settings_in(dir.path(), MemoryBackend::online());
        let view = store.config_view(vec![], vec![]).unwrap();

        let mut keys = sorted_keys(&view);
        keys.sort();
        let mut expected = CONFIG_GET_KEYS.to_vec();
        expected.sort();
        assert_eq!(keys, expected);
        // `get_config()` never carries the `ok` key the POST body does.
        assert_eq!(view.get("ok"), None);
        assert_eq!(view["schema_version"], json!(3));
    }

    #[test]
    fn models_success_body_has_no_ok_key() {
        // readmd.py:2375 is `{'models': ids}` and nothing else.
        let body = models_envelope(vec!["gpt-fake".to_string()]);
        assert_eq!(sorted_keys(&body), MODELS_OK_KEYS.to_vec());
    }

    /// `ai.py:925-949` — the shapes a provider may answer with, and the two
    /// reasons the route still raises instead of sending an empty 200.
    #[test]
    fn model_id_extraction_follows_ai_py() {
        assert_eq!(
            model_ids_from_json(r#"{"data":[{"id":"a"},{"id":"b"}]}"#).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        // `data` first, then `models` (`ai.py:926`).
        assert_eq!(
            model_ids_from_json(r#"{"models":[" x ","y"]}"#).unwrap(),
            vec!["x".to_string(), "y".to_string()]
        );
        // A bare array is legal too (Ollama-ish proxies).
        assert_eq!(
            model_ids_from_json(r#"["one","two"]"#).unwrap(),
            vec!["one".to_string(), "two".to_string()]
        );
        // `id` or `name` or `model`, in that order (`ai.py:936`).
        assert_eq!(
            model_ids_from_json(
                r#"{"data":[{"name":"n1","model":"m1"},{"model":"m2"},{"nope":1}]}"#
            )
            .unwrap(),
            vec!["n1".to_string(), "m2".to_string()]
        );
        // An empty list is a *raise*, never a 200 with nothing in it.
        assert_eq!(
            model_ids_from_json(r#"{"data":[]}"#).unwrap_err(),
            "接口未返回模型列表（data 为空）"
        );
        let error = model_ids_from_json(&format!("not json {}", "x".repeat(600))).unwrap_err();
        assert!(error.starts_with("模型列表解析失败："), "{error}");
        // `ai.py:929` clamps the echoed body to 300 characters.
        assert_eq!(error.chars().count(), "模型列表解析失败：".chars().count() + 300);
    }

    /// `ai.py:527-556` — a Base URL that already names an endpoint must not be
    /// doubled, and `full_url` mode must be used verbatim.
    #[test]
    fn base_url_normalization_matches_ai_py() {
        assert_eq!(
            normalize_base_url("https://relay.invalid/v1/chat/completions"),
            "https://relay.invalid/v1"
        );
        assert_eq!(normalize_base_url("  https://relay.invalid/  "), "https://relay.invalid");
        // Case-insensitive match, original case preserved (`ai.py:542`).
        assert_eq!(
            normalize_base_url("https://Relay.Invalid/API/v1/Messages"),
            "https://Relay.Invalid/API"
        );
        assert_eq!(normalize_base_url("   "), "");
        assert_eq!(
            endpoint_url("https://relay.invalid/v1", "chat/completions", "prefix"),
            "https://relay.invalid/v1/chat/completions"
        );
        assert_eq!(
            endpoint_url("https://relay.invalid/v1/models", "models", "prefix"),
            "https://relay.invalid/models"
        );
        assert_eq!(
            endpoint_url("https://relay.invalid/custom/endpoint", "chat/completions", "Full_URL"),
            "https://relay.invalid/custom/endpoint"
        );
    }

    #[test]
    fn chat_json_body_carries_usage_only_when_the_provider_reported_it() {
        // readmd.py:2421-2424.
        let plain = chat_json_envelope("text", None);
        assert_eq!(sorted_keys(&plain), vec!["content".to_string(), "ok".to_string()]);
        assert_eq!(plain.get("usage"), None);

        let with_usage = chat_json_envelope("text", Some(json!({"total_tokens": 3})));
        let keys = sorted_keys(&with_usage);
        assert!(keys.contains(&"usage".to_string()));
        assert_eq!(with_usage["ok"], json!(true));

        // The SSE contract (readmd.py:2432-2448).
        assert_eq!(CHAT_SSE_TYPES, &["meta", "delta", "usage", "error", "done"]);
        assert_eq!(CHAT_JSON_OPTIONAL_KEYS, &["usage"]);
    }

    #[test]
    fn a_raised_failure_keeps_exception_text_out_of_the_body() {
        // `_send_api_error` (readmd.py:1421-1430) sends only `ok` + `error_code`
        // plus explicit extras; host paths must never ride along.
        let absolute_path = "C:\\Users\\someone\\AppData\\Roaming\\ReadMD\\ai.json";
        let error = AiRoute::Config.raised_error(format!("cannot write {absolute_path}"));
        assert_eq!(error.status, 500);
        assert_eq!(error.code, "ai_config_failed");
        let payload = error.payload();
        assert_eq!(payload, api_error_envelope("ai_config_failed"));
        let text = serde_json::to_string(&payload).unwrap();
        assert!(!text.contains("AppData"), "body leaked a host path: {text}");
        assert!(!text.contains("cannot write"));
    }

    #[test]
    fn outcome_separates_a_returned_body_from_a_raised_exception() {
        // The `PetOutcome` distinction, applied to the AI routes:
        // a guard the method answers itself vs. an exception it lets escape.
        let returned = AiRoute::Chat.outcome(Err(Error::Msg("skill_required".to_string())));
        assert_eq!(
            returned,
            AiOutcome::Returned {
                status: 400,
                payload: api_error_envelope("skill_required"),
            }
        );
        assert_eq!(returned.status(), Some(400));

        let invalid = AiRoute::Chat.outcome(Err(Error::Msg("请求格式错误".to_string())));
        // readmd.py:2388 sends `ok` + `error_code` + the legacy `error` text.
        assert_eq!(sorted_keys(invalid.payload().unwrap()).len(), 3);

        let raised = AiRoute::Models.outcome(Err(Error::Msg("网络错误：timeout".to_string())));
        assert_eq!(raised, AiOutcome::Raised);
        assert_eq!(raised.payload(), None);
        assert_eq!(raised.status(), None);
        assert!(!raised.is_ok_envelope());

        let saved = AiRoute::Config.outcome(Ok(json!({"ok": true})));
        assert_eq!(saved.status(), Some(200));
        assert!(saved.is_ok_envelope());
        assert_eq!(saved.payload().unwrap(), &json!({"ok": true}));
        assert_eq!(AiRoute::Config.success_keys(), CONFIG_SAVE_KEYS);
        assert_eq!(AiRoute::Chat.success_keys(), CHAT_JSON_KEYS);
    }

    #[test]
    fn the_two_models_rejections_keep_the_legacy_error_shape() {
        // readmd.py:2347 and :2359 answer `{'error': text}` — no `ok`, no
        // `error_code`, and deliberately at two different statuses.  Upgrading
        // either of them would be a parity break.
        assert_eq!(
            MODELS_GUARD_REJECTIONS,
            &[
                (400, "API Key 不得出现在 URL，请使用 POST 请求体"),
                (403, "凭据与提供商不匹配"),
            ]
        );
        for (status, text) in MODELS_GUARD_REJECTIONS {
            let body = legacy_error_envelope(text);
            assert_eq!(sorted_keys(&body), vec!["error".to_string()]);
            assert!(*status != 500, "handler-level rejections never become 500s");
        }
    }

    /// Both `invalid_request` guards answer *inside* the chat route, so they
    /// must not collapse into the route's 502 `provider_error`.
    #[test]
    fn the_two_chat_guards_answer_400_with_pythons_own_wording() {
        for (error_text, message) in [
            ("请求格式错误", "请求格式错误"), // readmd.py:2388
            ("payload_not_object", "请求体必须是 JSON 对象"), // readmd.py:2392
        ] {
            let outcome = AiRoute::Chat.outcome(Err(Error::Msg(error_text.to_string())));
            assert_eq!(outcome.status(), Some(400), "guard must not reach the 502");
            let payload = outcome.payload().unwrap();
            assert_eq!(
                sorted_keys(payload),
                vec!["error".to_string(), "error_code".to_string(), "ok".to_string()]
            );
            assert_eq!(payload["error_code"], json!("invalid_request"));
            assert_eq!(payload["error"], json!(message));
            assert_eq!(payload["ok"], json!(false));
        }

        // skill_required is the third guard and it sends only ok + error_code.
        let skill = AiRoute::Chat.outcome(Err(Error::Msg("skill_required".to_string())));
        assert_eq!(skill.payload(), Some(&api_error_envelope("skill_required")));
        // Anything else is a real raise: the route's 502, no body of its own.
        assert_eq!(
            AiRoute::Chat.outcome(Err(Error::Msg("Connection reset".to_string()))),
            AiOutcome::Raised
        );
    }

    #[test]
    fn provider_error_bodies_are_clamped_like_python() {
        // ai.py:392 `[:500]`, on characters rather than bytes.
        assert_eq!(truncate("short"), "short");
        assert_eq!(truncate(&"a".repeat(600)).chars().count(), 500);
        let cjk = "错误".repeat(400);
        assert_eq!(truncate(&cjk).chars().count(), 500);
        // Slicing must still be valid UTF-8 — a byte slice would panic.
        assert!(std::str::from_utf8(truncate(&cjk).as_bytes()).is_ok());
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::new(400, "bad_request");
        assert_eq!(err.status, 400);
        assert_eq!(err.code, "bad_request");

        let err_with_detail = ApiError::with(500, "internal", "something went wrong");
        assert_eq!(err_with_detail.detail, Some("something went wrong".to_string()));
    }

    // ---- system_language() — pins against readmd_core/config.py:55 ----

    /// `locale_env_localename()` reads the process environment, so any test that
    /// overrides it must exclude every other env-reading test for its whole call
    /// graph — the convention `server.rs`'s `authorize()` tests and `batch2.rs`'s
    /// `is_win7()` tests both use.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// RAII override for one locale variable: a failing assertion inside the
    /// guarded region must not leak the value into the next probe.
    struct EnvVar(&'static str, Option<String>);

    impl Drop for EnvVar {
        fn drop(&mut self) {
            match &self.1 {
                Some(value) => std::env::set_var(self.0, value),
                None => std::env::remove_var(self.0),
            }
        }
    }

    fn set_env(name: &'static str, value: Option<&str>) -> EnvVar {
        let previous = std::env::var(name).ok();
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
        EnvVar(name, previous)
    }

    #[test]
    fn system_language_always_returns_a_code_like_python() {
        // `config.py:82-83`/`91-92` swallow every exception and `config.py:93`
        // ends on the literal 'zh-CN', so Python's get_system_language() never
        // raises.  `parity_pets::pet_locale`'s Err arm ("en") must therefore
        // stay unreachable for parity to hold.
        let code = system_language().expect("system_language must not fail");
        assert!(!code.is_empty(), "empty locale code");
        assert!(
            code.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "unexpected locale code {code:?}"
        );
    }

    #[test]
    fn chinese_sublang_branches_match_python() {
        // config.py:63-70, in order: 0x02 -> zh-CN, 0x01 -> zh-TW,
        // 0x04 -> zh-HK, 0x03 -> zh-HK, everything else -> zh-CN.
        for (sub, expected) in [
            (0x02u16, "zh-CN"),
            (0x01u16, "zh-TW"),
            (0x04u16, "zh-HK"),
            (0x03u16, "zh-HK"),
            (0x00u16, "zh-CN"),
            (0x16u16, "zh-CN"),
            (0x3fu16, "zh-CN"),
        ] {
            assert_eq!(chinese_language(sub), expected, "sublang {sub:#04x}");
        }
    }

    #[test]
    fn primary_lang_map_matches_python_table() {
        // config.py:71-79 has exactly 33 entries; a dropped or re-cased one
        // silently changes which i18n bundle the pet's line pack resolves to.
        assert_eq!(SYSTEM_LANGUAGE_PRIMARY_MAP.len(), 33);
        for (primary, expected) in [
            (0x09u16, "en"),
            (0x11u16, "ja"),
            (0x12u16, "ko"),
            (0x0cu16, "fr"),
            (0x07u16, "de"),
            (0x0au16, "es"),
            (0x16u16, "pt"),
            (0x19u16, "ru"),
            (0x10u16, "it"),
            (0x01u16, "ar"),
            (0x0du16, "he"),
            (0x1eu16, "th"),
            (0x2au16, "vi"),
            (0x21u16, "id"),
            (0x39u16, "hi"),
            (0x45u16, "bn"),
            (0x55u16, "my"),
            (0x54u16, "lo"),
            (0x53u16, "km"),
            (0x3eu16, "ms"),
            (0x06u16, "da"),
            (0x0bu16, "fi"),
            (0x14u16, "no"),
            (0x1du16, "sv"),
            (0x13u16, "nl"),
            (0x1au16, "hr"),
            (0x18u16, "ro"),
            (0x61u16, "ne"),
            (0x24u16, "sl"),
            (0x1fu16, "tr"),
            (0x22u16, "uk"),
            (0x08u16, "el"),
            (0x0eu16, "hu"),
        ] {
            let found = SYSTEM_LANGUAGE_PRIMARY_MAP
                .iter()
                .find(|(id, _)| *id == primary)
                .map(|(_, code)| *code);
            assert_eq!(found, Some(expected), "primary {primary:#04x}");
        }
        // 0x04 (Chinese) is *not* in the map — it is answered before it.
        assert!(SYSTEM_LANGUAGE_PRIMARY_MAP.iter().all(|(id, _)| *id != 0x04));
        // An unmapped primary falls through to the locale probe, like Python.
        assert_eq!(
            SYSTEM_LANGUAGE_PRIMARY_MAP.iter().find(|(id, _)| *id == 0x1c),
            None,
            "0x1c (Indonesian legacy slot) must fall through"
        );
    }

    #[test]
    fn app_language_code_matches_python_string_rules() {
        // loc = loc.replace('_','-'); zh special case; else loc.split('-')[0].
        for (loc, expected) in [
            ("zh_CN", "zh-CN"),
            ("zh-CN", "zh-CN"),
            ("zh_TW", "zh-TW"),
            ("zh_HK", "zh-HK"),
            ("zh_MO", "zh-CN"),
            ("zh_SG", "zh-CN"),
            // Python's 'Hant' test wins over the 'TW' test, so a Hant-TW locale
            // answers zh-HK.  Odd, but it is the observable behaviour.
            ("zh_Hant_TW", "zh-HK"),
            ("en_US", "en"),
            ("en-GB", "en"),
            ("ja_JP", "ja"),
            ("pt_BR", "pt"),
            ("nb_NO", "nb"),
            ("C", "C"),
            ("", ""),
        ] {
            assert_eq!(app_language_code(loc), expected, "loc {loc:?}");
        }
    }

    #[test]
    fn localename_parsing_matches_parse_localename() {
        for (name, expected) in [
            ("zh_CN.UTF-8", Some("zh-CN")),
            ("en_US.ISO8859-1", Some("en")),
            ("ja_JP.eucJP", Some("ja")),
            ("de_DE@euro", Some("de")),
            ("fr_FR.ISO-8859-15@euro", Some("fr")),
            ("en", Some("en")),
            // _parse_localename returns (None, None) / (None, 'UTF-8') for these,
            // so config.py's `if loc:` skips them and get_system_language() lands
            // on its 'zh-CN' default.
            ("C", None),
            ("UTF-8", None),
            ("", None),
        ] {
            assert_eq!(
                language_from_localename(name).as_deref(),
                expected,
                "localename {name:?}"
            );
        }
    }

    #[test]
    fn locale_env_localename_follows_envvars_order_and_language_split() {
        let _guard = env_guard();
        // One long-lived RAII guard per variable: an unbound `set_env(...)`
        // temporary would restore its previous value on the same statement and
        // silently make every later assertion test the ambient environment.
        let _all = set_env("LC_ALL", None);
        let _ct = set_env("LC_CTYPE", None);
        let _lang = set_env("LANG", None);
        let _lg = set_env("LANGUAGE", None);
        assert_eq!(locale_env_localename(), None, "nothing set -> 'C' -> None");

        std::env::set_var("LANG", "ja_JP.UTF-8");
        assert_eq!(locale_env_localename().as_deref(), Some("ja_JP.UTF-8"));

        // LC_ALL precedes LANG, exactly like CPython's envvars tuple.
        std::env::set_var("LC_ALL", "de_DE.UTF-8");
        assert_eq!(locale_env_localename().as_deref(), Some("de_DE.UTF-8"));

        // An empty value is falsy in Python and does not stop the scan.
        std::env::set_var("LC_ALL", "");
        assert_eq!(locale_env_localename().as_deref(), Some("ja_JP.UTF-8"));

        // LANGUAGE is GNU gettext's colon list; only its first entry counts.
        std::env::set_var("LANG", "");
        std::env::set_var("LC_CTYPE", "");
        std::env::set_var("LANGUAGE", "ko_KR:en_US");
        assert_eq!(locale_env_localename().as_deref(), Some("ko_KR"));
    }
}
