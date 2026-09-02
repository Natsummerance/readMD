# -*- coding: utf-8 -*-
"""ReadMD AI 模块：外部大模型 API 接入（OpenAI 兼容 / Anthropic 双协议）。

提供商预设覆盖主流 OpenAI 兼容厂商。用户自定义连接仅保存于本机；
API Key 优先级：界面配置 > 环境变量 > 空。
"""
import json
import logging
import os
import re
import uuid
import urllib.request
import urllib.error


from ..readmd_core.config import DATA_DIR
from ..readmd_core.service import ReadMDCoreService
from ..readmd_core.utils import save_json as save_json_atomic
from .crypto import (encrypt_api_key, decrypt_api_key, is_crypto_available,
                      store_credential, load_credential, delete_credential)
from .skills import SkillError

CONFIG_FILE = os.path.join(DATA_DIR, 'ai.json')
CONFIG_SCHEMA_VERSION = 3

# 内置预设（只读模板，可被自定义覆盖）
CATALOG_FILE = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(__file__))),
                             "assets", "providers", "provider-catalog.json")


def _load_provider_catalog():
    """Load the pinned provider catalog without network access."""
    try:
        with open(CATALOG_FILE, "r", encoding="utf-8") as handle:
            data = json.load(handle)
        providers = data.get("providers") if isinstance(data, dict) else data
        if not isinstance(providers, list):
            raise ValueError("provider catalog must contain a list")
        return [dict(item) for item in providers if isinstance(item, dict) and item.get("name")]
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        logging.error("provider catalog unavailable: %s", exc)
        return []


PRESETS = _load_provider_catalog()


def _load_upstream_provider_catalog():
    """Load source-only CC-SWITCH entries for the provider workbench.

    These records are intentionally kept separate from selectable ReadMD
    presets: a source entry may not have a safe endpoint or model list yet.
    The UI can search it and import its protocol hints, while runtime calls
    still go through the normal Provider v3 validation path.
    """
    try:
        with open(CATALOG_FILE, "r", encoding="utf-8") as handle:
            data = json.load(handle)
        entries = data.get("upstream_entries", []) if isinstance(data, dict) else []
        return [dict(item) for item in entries if isinstance(item, dict) and item.get("name")]
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        logging.error("upstream provider catalog unavailable: %s", exc)
        return []


UPSTREAM_PROVIDERS = _load_upstream_provider_catalog()

# Backward-compatible action index. Prompt text is intentionally absent; all
# instructions are resolved from assets/skills through ``_skill_messages``.
ACTIONS = {
    "quick_read": ("快速阅读", "readmd-quick-read"), "polish": ("润色", "readmd-polish"),
    "modify": ("修改", "readmd-format-fix"), "expand": ("扩充", "readmd-polish"),
    "continue": ("续写", "readmd-continue"), "translate": ("翻译", "readmd-translate"),
    "ask": ("问答", "readmd-ask"), "format_fix": ("排版自愈修复", "readmd-format-fix"),
}


def load():
    """模块就绪钩子（ai 模块纯标准库，秒级加载）。"""
    ensure_config()


def ensure_config():
    """升级到 v2 配置格式。

    v2.1.1 曾把非通用连接写入用户配置，但旧格式未保存来源标记，无法在
    不保留任何识别信息的前提下安全区分它们。首次迁移因此清空旧自定义项，
    仅保留公开预设；之后用户新建的连接保持在本机。
    """
    cfg = _read_cfg()
    if cfg.get("schema_version") not in (2, CONFIG_SCHEMA_VERSION):
        cfg = {"schema_version": CONFIG_SCHEMA_VERSION, "providers": [], "current": {}}
        _write_cfg(cfg)
    else:
        if cfg.get("schema_version") == 2:
            cfg["schema_version"] = CONFIG_SCHEMA_VERSION
            for provider in cfg.get("providers", []):
                if isinstance(provider, dict):
                    provider.setdefault("endpoint_mode", "prefix")
                    provider.setdefault("capabilities", {})
            _write_cfg(cfg)
        cfg.setdefault("providers", [])
        cfg.setdefault("current", {})
        changed = False
        for provider in cfg["providers"]:
            if isinstance(provider, dict) and not provider.get("id"):
                provider["id"] = "custom:" + uuid.uuid4().hex
                changed = True
            if isinstance(provider, dict) and provider.get("api_key"):
                # Migrate encrypted v2 material into the OS credential store
                # (or the encrypted vault fallback) and remove it from ai.json.
                legacy = str(provider.get("api_key"))
                if legacy.startswith("enc:") and is_crypto_available():
                    secret = decrypt_api_key(legacy)
                    if secret:
                        cid = str(provider.get("credential_id") or ("cred:" + uuid.uuid4().hex))
                        try:
                            backend = store_credential(cid, secret)
                            provider["credential_id"] = cid
                            provider["credential_backend"] = backend
                            provider.pop("api_key", None)
                            changed = True
                        except Exception:
                            logging.error("legacy credential migration failed", exc_info=True)
                            provider.pop("api_key", None)
                            provider.pop("credential_id", None)
                            provider["credential_reset_required"] = True
                            changed = True
                    else:
                        provider.pop("api_key", None)
                        provider.pop("credential_id", None)
                        provider["credential_reset_required"] = True
                        changed = True
                else:
                    # Never keep a plaintext legacy key in the v3 file.  The
                    # UI must request a fresh key instead of exposing it.
                    provider.pop("api_key", None)
                    provider.pop("credential_id", None)
                    provider["credential_reset_required"] = True
                    changed = True
        current = cfg["current"]
        if current.get("provider") and not current.get("provider_id"):
            legacy_name = current.get("provider")
            match = next((p for p in cfg["providers"] if p.get("name") == legacy_name), None)
            current["provider_id"] = (match or {}).get("id", "preset:" + str(legacy_name))
            current.pop("provider", None)
            changed = True
        if changed:
            _write_cfg(cfg)
    return cfg


def _read_cfg():
    try:
        with open(CONFIG_FILE, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception:
        return {"schema_version": CONFIG_SCHEMA_VERSION, "providers": [], "current": {}}


def _write_cfg(cfg):
    if not save_json_atomic(CONFIG_FILE, cfg):
        logging.exception("ai config save failed")
        raise OSError("ai_config_write_failed")


def get_config():
    cfg = ensure_config()
    custom = cfg.get("providers", [])

    def annotate(p):
        d = dict(p)
        d.setdefault("id", "preset:" + str(d.get("name") or "provider"))
        resolved_key = resolve_key(d)
        d["has_key"] = bool(resolved_key)
        d["key_source"] = key_source(d)
        d["mode"] = p.get("mode") or ("messages" if p.get("format") == "anthropic" else "auto")
        d.setdefault("category", "custom" if p.get("custom") else "preset")
        d.setdefault("endpoint_mode", "prefix")
        d.setdefault("capabilities", {"chat": True, "models": bool(p.get("models"))})
        d.setdefault("website", "")
        d.setdefault("provenance", {
            "source": "CC-SWITCH offline snapshot",
            "upstream_commit": "6243e20ad6f1835f9ac94ab39ea0eb62a6795bc0",
            "source_files": ["assets/upstream/manifest.json"],
            "adaptation_notes": ["ReadMD runtime keeps protocol/model fields and omits promotion/affiliate parameters."],
        })
        # credential_id is the only identifier exposed to clients; encrypted
        # material remains server-side in the private config file.
        # Do not advertise a stale credential handle as configured.  A prior
        # install can retain the opaque id after the OS credential was removed;
        # returning it made every UI surface report a ready provider and then
        # fail only when the first request was sent.
        if p.get("credential_id") and resolved_key:
            d["credential_id"] = p.get("credential_id")
        else:
            d.pop("credential_id", None)
        d.setdefault("endpoint_mode", "prefix")
        d.setdefault("capabilities", {})
        # 配置接口只提供状态，绝不把保存在磁盘中的 API Key 回传给前端。
        d.pop("api_key", None)
        return d

    presets = [annotate(dict(p)) for p in PRESETS]
    upstream = []
    for source in UPSTREAM_PROVIDERS:
        item = annotate(dict(source))
        # Source-only records are browseable/importable but cannot silently
        # become the active provider until the user supplies an endpoint.
        item["selectable"] = False
        item["has_key"] = False
        item["provenance"] = {
            "source": "CC-SWITCH offline snapshot",
            "source_ref": source.get("source_ref", ""),
            "source_sha256": source.get("source_sha256", ""),
            "upstream_commit": source.get("upstream_commit", ""),
            "adaptation_notes": source.get("adaptation_notes", []),
        }
        upstream.append(item)
    customs = [annotate(dict(p)) for p in custom]
    return {"schema_version": CONFIG_SCHEMA_VERSION, "presets": presets,
            "custom": customs, "upstream_catalog": upstream,
            "current": cfg.get("current", {})}


def save_config(payload):
    cfg = ensure_config()
    if "providers" in payload:
        old = {p.get("id"): p for p in cfg.get("providers", []) if isinstance(p, dict)}
        old_by_name = {p.get("name"): p for p in cfg.get("providers", []) if isinstance(p, dict)}
        providers = []
        names = set()
        for raw in payload.get("providers") or []:
            if not isinstance(raw, dict):
                continue
            p = dict(raw)
            provider_id = str(p.get("id") or "").strip()
            if not provider_id.startswith("custom:"):
                provider_id = "custom:" + uuid.uuid4().hex
            name = str(p.get("name") or "").strip()
            if not name or name in names:
                continue
            names.add(name)
            p["name"] = name
            p["id"] = provider_id
            p["custom"] = True
            p["models"] = [str(m).strip() for m in (p.get("models") or []) if str(m).strip()]
            p["endpoint_mode"] = str(p.get("endpoint_mode") or "prefix").strip().lower()
            if p["endpoint_mode"] not in ("prefix", "full_url"):
                p["endpoint_mode"] = "prefix"
            if not isinstance(p.get("capabilities"), dict):
                p["capabilities"] = {}
            raw_headers = p.get("headers") if isinstance(p.get("headers"), dict) else {}
            safe_headers = {}
            for header_name, header_value in raw_headers.items():
                header_name = str(header_name).strip()
                lower_name = header_name.lower()
                if (not header_name or len(header_name) > 128 or
                        any(marker in lower_name for marker in ("authorization", "api-key", "apikey", "token", "secret", "password", "cookie"))):
                    continue
                value = str(header_value)
                if len(value) <= 2048 and not any(ord(ch) < 32 for ch in value):
                    safe_headers[header_name] = value
            p["headers"] = safe_headers
            # 前端不会收到旧 Key；编辑其它字段时保留原 Key。只有显式标记才清除。
            previous = old.get(provider_id) or old_by_name.get(name) or {}
            if p.get("api_key"):
                if not is_crypto_available():
                    raise RuntimeError("当前环境缺少凭据加密支持，拒绝以明文保存 API Key")
                candidate = str(p.get("credential_id") or previous.get("credential_id") or "")
                candidate = candidate if re.fullmatch(r"cred:[A-Za-z0-9_-]{8,128}", candidate) else ("cred:" + uuid.uuid4().hex)
                p["credential_id"] = candidate
                p["credential_backend"] = store_credential(candidate, str(p.pop("api_key")))
            elif (previous.get("api_key") or previous.get("credential_id")) and not p.pop("clear_key", False):
                candidate = str(p.get("credential_id") or previous.get("credential_id") or "")
                candidate = candidate if re.fullmatch(r"cred:[A-Za-z0-9_-]{8,128}", candidate) else ("cred:" + uuid.uuid4().hex)
                if previous.get("credential_id") and not previous.get("api_key"):
                    p["credential_id"] = candidate
                    p["credential_backend"] = previous.get("credential_backend", "native")
                else:
                    legacy = str(previous.get("api_key") or "")
                    secret = decrypt_api_key(legacy) if legacy.startswith("enc:") else ""
                    if not secret:
                        raise RuntimeError("旧凭据无法安全迁移，请重新输入 API Key")
                    p["credential_id"] = candidate
                    p["credential_backend"] = store_credential(candidate, secret)
            else:
                p.pop("clear_key", None)
                old_credential = str(previous.get("credential_id") or p.get("credential_id") or "")
                if old_credential:
                    delete_credential(old_credential)
                p.pop("credential_id", None)
                p.pop("credential_backend", None)
            p.pop("api_key", None)
            providers.append(p)
        cfg["providers"] = providers
    if "current" in payload:
        current = payload.get("current") or {}
        cfg["current"] = {"provider_id": str(current.get("provider_id") or current.get("provider") or ""),
                          "model": str(current.get("model") or "")}
    cfg["schema_version"] = CONFIG_SCHEMA_VERSION
    _write_cfg(cfg)
    return True


def find_provider(identifier):
    cfg = ensure_config()
    for p in cfg.get("providers", []):
        if p.get("id") == identifier or p.get("name") == identifier:
            return dict(p, custom=True)
    for p in PRESETS:
        if "preset:" + p["name"] == identifier or p["name"] == identifier:
            return dict(p, id="preset:" + p["name"])
    return None


def find_provider_by_credential(credential_id):
    """Resolve a credential handle without accepting secret material from clients."""
    cid = str(credential_id or "").strip()
    if not cid or len(cid) > 128 or not cid.startswith("cred:"):
        return None
    cfg = ensure_config()
    for p in cfg.get("providers", []):
        if p.get("credential_id") == cid:
            return dict(p, custom=True)
    return None


def resolve_key(p):
    """API Key：界面配置 > 环境变量。"""
    if p.get("credential_id"):
        return load_credential(p.get("credential_id"))
    if p.get("api_key", "").startswith("enc:"):
        return decrypt_api_key(p["api_key"])
    env = p.get("env_key") or ""
    if env:
        v = os.environ.get(env, "")
        if v:
            return v
    return ""


def key_source(p):
    if p.get("credential_id") and load_credential(p.get("credential_id")):
        return "configured"
    env = p.get("env_key") or ""
    if env and os.environ.get(env):
        return "env:" + env
    return ""


def _is_local_provider(provider):
    """Return whether a provider is expected to work without a credential.

    Local OpenAI-compatible servers (Ollama/LM Studio and user supplied
    localhost endpoints) normally do not require authentication.  Keeping
    this decision server-side prevents the UI/MCP clients from having to
    special-case provider names and avoids sending an empty ``Bearer`` value.
    """
    if not isinstance(provider, dict):
        return False
    if str(provider.get("category") or "").lower() == "local":
        return True
    base_url = str(provider.get("base_url") or "").lower()
    return any(host in base_url for host in ("localhost", "127.0.0.1", "::1"))


def _openai_auth_headers(api_key):
    return {"Authorization": "Bearer " + api_key} if api_key else {}


class ChatError(Exception):
    pass


DEFAULT_HTTP_HEADERS = {
    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 ReadMD-AI/2.3",
    "Accept": "application/json, text/plain, */*",
}


def _http_json(url, headers, body, timeout=240):
    req_headers = dict(DEFAULT_HTTP_HEADERS)
    if headers:
        req_headers.update(headers)
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=req_headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:500]
        raise ChatError("HTTP %d：%s" % (e.code, detail)) from e
    except urllib.error.URLError as e:
        raise ChatError("网络错误：%s" % e.reason) from e


def _http_stream(url, headers, body, timeout=300):
    req_headers = dict(DEFAULT_HTTP_HEADERS)
    if headers:
        req_headers.update(headers)
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=req_headers, method="POST")
    try:
        return urllib.request.urlopen(req, timeout=timeout)
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:500]
        raise ChatError("HTTP %d：%s" % (e.code, detail)) from e
    except urllib.error.URLError as e:
        raise ChatError("网络错误：%s" % e.reason) from e


def _openai_messages(messages):
    out = []
    for m in messages:
        role = m.get("role", "user")
        content = m.get("content", "")
        if role == "system":
            out.insert(0, {"role": "system", "content": content})
        else:
            out.append({"role": role, "content": content})
    return out


def _anthropic_messages(messages):
    system = []
    msgs = []
    for m in messages:
        role = m.get("role", "user")
        content = m.get("content", "")
        if role == "system":
            system.append(content)
        else:
            r = "assistant" if role == "assistant" else "user"
            if msgs and msgs[-1]["role"] == r:
                msgs[-1]["content"] += "\n\n" + content
            else:
                msgs.append({"role": r, "content": content})
    if not msgs:
        msgs = [{"role": "user", "content": "..."}]
    return "\n\n".join(system), msgs


def _skill_messages(payload):
    """Resolve a Skill into a system message without accepting raw prompt code."""
    skill_id = str(payload.get("skill_id") or "").strip()
    messages = list(payload.get("messages") or [])
    if not skill_id:
        return messages
    try:
        global _CORE_SERVICE
        if _CORE_SERVICE is None:
            _CORE_SERVICE = ReadMDCoreService()
        else:
            _CORE_SERVICE.reload()
        variables = dict(payload.get("skill_variables") or {})
        if not variables.get("document"):
            users = [m.get("content", "") for m in messages if m.get("role") == "user"]
            variables["document"] = users[-1] if users else ""
        variables.setdefault("language", "the document's language")
        variables.setdefault("request", "")
        variables.setdefault("context", "")
        variables.setdefault("selection", variables.get("document", ""))
        variables.setdefault("output_format", "Markdown")
        system = _CORE_SERVICE.render_skill(skill_id, variables)
    except SkillError as exc:
        raise ChatError(str(exc)) from exc
    except Exception as exc:
        logging.exception("Skill rendering failed: %s", skill_id)
        raise ChatError("Skill 渲染失败：请检查 Skill 模板或文档内容后重试") from exc
    return [{"role": "system", "content": system}] + [m for m in messages if m.get("role") != "system"]


_CORE_SERVICE = None


def chat(payload):
    """调用大模型。payload: {provider?, base_url?, format?, api_key?, model,
    messages, temperature?, stream?, mode?}。统一返回生成器：产出 str 增量，
    末尾可产出 {'usage': {...}} 用量事件。mode: auto|chat|completion|responses|messages。"""
    current = {}
    name = payload.get("provider") or ""
    if not name:
        cfg = ensure_config()
        current = cfg.get("current") if isinstance(cfg.get("current"), dict) else {}
        name = str(current.get("provider_id") or current.get("provider") or "").strip()
    prov = find_provider(name) if name else {}
    if not prov and name:
        raise ChatError("未知提供商：%s" % name)
    if not prov:
        prov = {}
    base_url = (payload.get("base_url") or prov.get("base_url") or "").rstrip("/")
    fmt = payload.get("format") or prov.get("format") or "openai"
    credential_id = str(payload.get("credential_id") or "").strip()
    if credential_id:
        if prov.get("credential_id") and prov.get("credential_id") != credential_id:
            raise ChatError("凭据与提供商不匹配")
        if not prov.get("credential_id"):
            prov = find_provider_by_credential(credential_id) or prov
    # api_key remains a one-version compatibility path for local clients; it
    # is never persisted or accepted through a URL. New clients send only the
    # opaque credential_id handle.
    api_key = payload.get("api_key") or resolve_key(prov)
    if not api_key and not _is_local_provider(prov):
        raise ChatError("未配置 API Key（可填入界面，或设置环境变量 %s）" % (prov.get("env_key") or ""))
    model = payload.get("model") or current.get("model") or (prov.get("models") or [""])[0] or ""
    messages = _skill_messages(payload)
    temperature = payload.get("temperature", 0.4)
    stream = bool(payload.get("stream", True))
    mode = (payload.get("mode") or prov.get("mode") or "").strip().lower()
    if not mode:
        mode = "messages" if fmt == "anthropic" else "auto"
    endpoint_mode = str(payload.get("endpoint_mode") or prov.get("endpoint_mode") or "prefix")
    custom_headers = payload.get("headers") if isinstance(payload.get("headers"), dict) else prov.get("headers")

    if mode in ("messages", "anthropic"):
        return _chat_anthropic(base_url, api_key, model, messages, temperature, stream,
                               endpoint_mode, custom_headers)
    if mode == "completion":
        return _chat_openai_completion(base_url, api_key, model, messages, temperature, stream,
                                       endpoint_mode, custom_headers)
    if mode == "responses":
        return _chat_openai_responses(base_url, api_key, model, messages, temperature, stream,
                                      endpoint_mode, custom_headers)
    return _chat_openai(base_url, api_key, model, messages, temperature, stream,
                        endpoint_mode, custom_headers)
def _normalize_base_url(base_url, endpoint="chat/completions"):
    """清洗与规范化 base_url，防止用户输入带尾部重复端点或多余斜杠。"""
    u = (base_url or "").strip().rstrip("/")
    if not u:
        return u
    suffixes_to_strip = [
        "/chat/completions",
        "/completions",
        "/responses",
        "/v1/messages",
        "/messages",
        "/v1/models",
        "/models",
    ]
    for s in suffixes_to_strip:
        if u.lower().endswith(s):
            u = u[:len(u) - len(s)].rstrip("/")
            break

    if not endpoint:
        return u
    endpoint = endpoint.lstrip("/")
    return u + "/" + endpoint


def _endpoint_url(base_url, endpoint, endpoint_mode="prefix"):
    """Resolve either a URL prefix or a user-supplied complete endpoint."""
    if str(endpoint_mode or "prefix").lower() == "full_url":
        return (base_url or "").strip().rstrip("/")
    return _normalize_base_url(base_url, endpoint)


def _request_headers(base, custom=None):
    """Merge non-sensitive provider headers without allowing auth override."""
    out = dict(base or {})
    if isinstance(custom, dict):
        for key, value in custom.items():
            key = str(key).strip()
            if not key or key.lower() in {"authorization", "x-api-key", "cookie"}:
                continue
            value = str(value)
            if len(key) > 128 or len(value) > 2048 or any(ord(c) < 32 for c in value):
                continue
            out[key] = value
    return out


def _openai_usage(d):
    u = d.get("usage") or {}
    if not u:
        return None
    out = {}
    if u.get("prompt_tokens") is not None:
        out["prompt_tokens"] = int(u["prompt_tokens"])
    if u.get("completion_tokens") is not None:
        out["completion_tokens"] = int(u["completion_tokens"])
    if u.get("total_tokens") is not None:
        out["total_tokens"] = int(u["total_tokens"])
    return out or None


def _chat_openai(base_url, api_key, model, messages, temperature, stream,
                 endpoint_mode="prefix", custom_headers=None):
    url = _endpoint_url(base_url, "chat/completions", endpoint_mode)
    body = {"model": model, "messages": _openai_messages(messages), "stream": stream, "temperature": temperature}
    if stream:
        body["stream_options"] = {"include_usage": True}
    headers = _request_headers({"Content-Type": "application/json", **_openai_auth_headers(api_key)}, custom_headers)

    if not stream:
        text = _http_json(url, headers, body)
        try:
            d = json.loads(text)
            usage = _openai_usage(d)
            content = d["choices"][0]["message"]["content"] or ""

            def g():
                if content:
                    yield content
                if usage:
                    yield {"usage": usage}
            return g()
        except Exception as e:
            raise ChatError("响应解析失败：%s" % text[:300]) from e

    resp = _http_stream(url, headers, body)

    def gen():
        usage = None
        try:
            for raw in resp:
                if not raw:
                    continue
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                if data == "[DONE]":
                    break
                try:
                    d = json.loads(data)
                except Exception:
                    continue
                if d.get("usage"):
                    usage = _openai_usage(d)
                choices = d.get("choices") or []
                if not choices:
                    continue
                delta = choices[0].get("delta") or {}
                if delta.get("content"):
                    yield delta["content"]
                # Some OpenAI-compatible providers send the usage-only event
                # after the final choice carries ``finish_reason``.  Keep
                # reading until the protocol terminator so the shared SSE
                # contract can emit usage before done.
        except urllib.error.URLError as e:
            raise ChatError("连接中断：%s" % e.reason) from e
        finally:
            try:
                resp.close()
            except Exception:
                pass
        if usage:
            yield {"usage": usage}
    return gen()


def _chat_openai_completion(base_url, api_key, model, messages, temperature, stream,
                            endpoint_mode="prefix", custom_headers=None):
    url = _endpoint_url(base_url, "completions", endpoint_mode)
    msgs = _openai_messages(messages)
    prompt_text = "\n\n".join(m.get("content", "") for m in msgs if m.get("content"))
    body = {"model": model, "prompt": prompt_text, "max_tokens": 4096,
            "temperature": temperature, "stream": stream}
    headers = _request_headers({"Content-Type": "application/json", **_openai_auth_headers(api_key)}, custom_headers)

    if not stream:
        text = _http_json(url, headers, body)
        try:
            d = json.loads(text)
            usage = _openai_usage(d)
            content = d["choices"][0].get("text") or ""

            def g():
                if content:
                    yield content
                if usage:
                    yield {"usage": usage}
            return g()
        except Exception as e:
            raise ChatError("响应解析失败：%s" % text[:300]) from e

    resp = _http_stream(url, headers, body)

    def gen():
        usage = None
        try:
            for raw in resp:
                if not raw:
                    continue
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                if data == "[DONE]":
                    break
                try:
                    d = json.loads(data)
                except Exception:
                    continue
                if d.get("usage"):
                    usage = _openai_usage(d)
                choices = d.get("choices") or []
                if not choices:
                    continue
                delta = choices[0].get("delta") or {}
                if delta.get("text"):
                    yield delta["text"]
                # Do not terminate on finish_reason: usage may follow as a
                # choice-less SSE event before [DONE].
        except urllib.error.URLError as e:
            raise ChatError("连接中断：%s" % e.reason) from e
        finally:
            try:
                resp.close()
            except Exception:
                pass
        if usage:
            yield {"usage": usage}
    return gen()


def _chat_openai_responses(base_url, api_key, model, messages, temperature, stream,
                           endpoint_mode="prefix", custom_headers=None):
    url = _endpoint_url(base_url, "responses", endpoint_mode)
    msgs = []
    for m in _openai_messages(messages):
        if m["role"] == "system":
            msgs.insert(0, {"role": "system", "content": m["content"]})
        else:
            msgs.append(m)
    body = {"model": model, "input": msgs, "stream": stream, "temperature": temperature}
    headers = _request_headers({"Content-Type": "application/json", **_openai_auth_headers(api_key)}, custom_headers)

    if not stream:
        text = _http_json(url, headers, body)
        try:
            d = json.loads(text)
            usage = _openai_usage(d)
            content = d.get("output_text") or ""

            def g():
                if content:
                    yield content
                if usage:
                    yield {"usage": usage}
            return g()
        except Exception as e:
            raise ChatError("响应解析失败：%s" % text[:300]) from e

    resp = _http_stream(url, headers, body)

    def gen():
        usage = None
        try:
            for raw in resp:
                if not raw:
                    continue
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                if data == "[DONE]":
                    break
                try:
                    d = json.loads(data)
                except Exception:
                    continue
                if d.get("type") == "response.output_text.delta":
                    dt = d.get("delta") or {}
                    if dt.get("text"):
                        yield dt["text"]
                elif d.get("type") == "response.completed":
                    r2 = d.get("response") or {}
                    if r2.get("usage"):
                        usage = _openai_usage(r2)
                elif d.get("type") == "error":
                    raise ChatError("提供商错误：%s" % json.dumps(d, ensure_ascii=False)[:300])
        except urllib.error.URLError as e:
            raise ChatError("连接中断：%s" % e.reason) from e
        finally:
            try:
                resp.close()
            except Exception:
                pass
        if usage:
            yield {"usage": usage}
    return gen()
def _anthropic_usage(u):
    out = {}
    if u.get("input_tokens") is not None:
        out["prompt_tokens"] = int(u["input_tokens"])
    if u.get("output_tokens") is not None:
        out["completion_tokens"] = int(u["output_tokens"])
    p = out.get("prompt_tokens")
    c = out.get("completion_tokens")
    if p is not None and c is not None:
        out["total_tokens"] = p + c
    return out or None


def _chat_anthropic(base_url, api_key, model, messages, temperature, stream,
                    endpoint_mode="prefix", custom_headers=None):
    url = _endpoint_url(base_url, "v1/messages", endpoint_mode)
    system, msgs = _anthropic_messages(messages)
    body = {"model": model, "max_tokens": 4096, "messages": msgs, "temperature": temperature, "stream": stream}
    if system:
        body["system"] = system
    headers = _request_headers({"Content-Type": "application/json", **({"x-api-key": api_key} if api_key else {}), "anthropic-version": "2023-06-01"}, custom_headers)

    if not stream:
        text = _http_json(url, headers, body)
        try:
            d = json.loads(text)
            content = "".join(b.get("text", "") for b in d.get("content", []) if b.get("type") == "text")
            usage = _anthropic_usage(d.get("usage") or {})

            def g():
                if content:
                    yield content
                if usage:
                    yield {"usage": usage}
            return g()
        except Exception as e:
            raise ChatError("响应解析失败：%s" % text[:300]) from e

    resp = _http_stream(url, headers, body)

    def gen():
        input_tokens = None
        output_tokens = 0
        try:
            for raw in resp:
                if not raw:
                    continue
                line = raw.decode("utf-8", "replace").strip()
                if not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                try:
                    d = json.loads(data)
                except Exception:
                    continue
                if d.get("type") == "message_start":
                    u = (d.get("message") or {}).get("usage") or {}
                    if u.get("input_tokens") is not None:
                        input_tokens = int(u["input_tokens"])
                elif d.get("type") == "content_block_delta":
                    delta = d.get("delta") or {}
                    if delta.get("type") == "text_delta" and delta.get("text"):
                        yield delta["text"]
                elif d.get("type") == "message_delta":
                    u = d.get("usage") or {}
                    if u.get("output_tokens") is not None:
                        output_tokens = int(u["output_tokens"])
                elif d.get("type") == "message_stop":
                    break
                elif d.get("type") == "error":
                    raise ChatError("提供商错误：%s" % json.dumps(d, ensure_ascii=False)[:300])
        except urllib.error.URLError as e:
            raise ChatError("连接中断：%s" % e.reason) from e
        finally:
            try:
                resp.close()
            except Exception:
                pass
        usage = _anthropic_usage({"input_tokens": input_tokens, "output_tokens": output_tokens})
        if usage:
            yield {"usage": usage}
    return gen()

def _http_get_json(url, headers, timeout=30):
    req_headers = dict(DEFAULT_HTTP_HEADERS)
    if headers:
        req_headers.update(headers)
    req = urllib.request.Request(url, headers=req_headers, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:500]
        raise ChatError("HTTP %d：%s" % (e.code, detail)) from e
    except urllib.error.URLError as e:
        raise ChatError("网络错误：%s" % e.reason) from e


def list_models(base_url, api_key="", mode="auto", endpoint_mode="prefix", custom_headers=None):
    """通过 API Key 获取可用模型列表。智能适配 OpenAI, OpenCode Zen, Anthropic, Ollama 等各类端点。"""
    base_url = (base_url or "").strip().rstrip("/")
    if not base_url:
        raise ChatError("请先填写 Base URL")

    clean_base = _normalize_base_url(base_url, endpoint="")
    mode = (mode or "").strip().lower()

    urls_to_try = []
    if mode in ("messages", "anthropic"):
        auth_hdrs = {"x-api-key": api_key, "anthropic-version": "2023-06-01"} if api_key else {}
        urls_to_try.append((clean_base + "/v1/models" if not clean_base.endswith("/v1") else clean_base + "/models", auth_hdrs))
    else:
        auth_hdrs = {"Authorization": "Bearer " + api_key} if api_key else {}
        if clean_base.endswith("/v1"):
            urls_to_try.append((clean_base + "/models", auth_hdrs))
            urls_to_try.append((clean_base[:-3].rstrip("/") + "/models", auth_hdrs))
        else:
            urls_to_try.append((clean_base + "/models", auth_hdrs))
            urls_to_try.append((clean_base + "/v1/models", auth_hdrs))
            urls_to_try.append((clean_base + "/api/tags", auth_hdrs))

    if str(endpoint_mode or "prefix").lower() == "full_url":
        # In full-url mode the configured URL is the model endpoint itself.
        auth = ({"x-api-key": api_key, "anthropic-version": "2023-06-01"}
                if mode in ("messages", "anthropic") else
                ({"Authorization": "Bearer " + api_key} if api_key else {}))
        urls_to_try = [(base_url.strip().rstrip("/"), _request_headers(auth, custom_headers))]
    else:
        urls_to_try = [(u, _request_headers(h, custom_headers)) for u, h in urls_to_try]

    last_err = None
    data = None
    for url, hdrs in urls_to_try:
        try:
            data = _http_get_json(url, hdrs)
            if data:
                break
        except Exception as e:
            last_err = e

    if not data:
        if last_err:
            raise last_err
        raise ChatError("未能连接到模型接口")

    try:
        d = json.loads(data)
    except Exception as e:
        raise ChatError("模型列表解析失败：%s" % data[:300]) from e

    raw_items = []
    if isinstance(d, dict):
        raw_items = d.get("data") or d.get("models") or []
    elif isinstance(d, list):
        raw_items = d

    ids = []
    for it in raw_items:
        if isinstance(it, dict):
            mid = it.get("id") or it.get("name") or it.get("model")
            if mid:
                ids.append(str(mid))
        elif isinstance(it, str) and it.strip():
            ids.append(it.strip())

    if not ids:
        raise ChatError("接口未返回模型列表（data 为空）")
    return ids
