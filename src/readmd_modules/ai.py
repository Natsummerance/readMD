# -*- coding: utf-8 -*-
"""ReadMD AI 模块：外部大模型 API 接入（OpenAI 兼容 / Anthropic 双协议）。

提供商预设覆盖主流 OpenAI 兼容厂商。用户自定义连接仅保存于本机；
API Key 优先级：界面配置 > 环境变量 > 空。
"""
import json
import logging
import os
import uuid
import urllib.request
import urllib.error


from ..readmd_core.config import DATA_DIR
from .crypto import encrypt_api_key, decrypt_api_key

CONFIG_FILE = os.path.join(DATA_DIR, 'ai.json')
CONFIG_SCHEMA_VERSION = 2

# 内置预设（只读模板，可被自定义覆盖）
PRESETS = [
    {"name": "OpenAI", "base_url": "https://api.openai.com/v1", "format": "openai",
     "models": ["gpt-4o", "gpt-4o-mini", "gpt-4.1", "o3-mini"],
     "env_key": "OPENAI_API_KEY", "website": "https://platform.openai.com",
     "note": "OpenAI 官方（国内直连需代理）"},
    {"name": "DeepSeek", "base_url": "https://api.deepseek.com/v1", "format": "openai",
     "models": ["deepseek-chat", "deepseek-reasoner"],
     "env_key": "DEEPSEEK_API_KEY", "website": "https://platform.deepseek.com",
     "note": "深度求索，性价比高"},
    {"name": "Kimi (Moonshot)", "base_url": "https://api.moonshot.cn/v1", "format": "openai",
     "models": ["kimi-k2-0711-preview", "moonshot-v1-32k", "moonshot-v1-128k"],
     "env_key": "MOONSHOT_API_KEY", "website": "https://platform.moonshot.cn",
     "note": "月之暗面 Kimi"},
    {"name": "智谱 GLM", "base_url": "https://open.bigmodel.cn/api/paas/v4", "format": "openai",
     "models": ["glm-4-plus", "glm-4-flash", "glm-4-air"],
     "env_key": "ZHIPU_API_KEY", "website": "https://open.bigmodel.cn",
     "note": "智谱 AI"},
    {"name": "通义千问 Qwen", "base_url": "https://dashscope.aliyuncs.com/compatible-mode/v1", "format": "openai",
     "models": ["qwen-max", "qwen-plus", "qwen-turbo"],
     "env_key": "DASHSCOPE_API_KEY", "website": "https://dashscope.aliyun.com",
     "note": "阿里云百炼"},
    {"name": "硅基流动 SiliconFlow", "base_url": "https://api.siliconflow.cn/v1", "format": "openai",
     "models": ["deepseek-ai/DeepSeek-V3", "Qwen/Qwen2.5-72B-Instruct", "THUDM/glm-4-9b-chat"],
     "env_key": "SILICONFLOW_API_KEY", "website": "https://siliconflow.cn",
     "note": "国内聚合，含众多开源模型"},
    {"name": "OpenRouter", "base_url": "https://openrouter.ai/api/v1", "format": "openai",
     "models": ["openai/gpt-4o", "anthropic/claude-3.5-sonnet", "deepseek/deepseek-chat"],
     "env_key": "OPENROUTER_API_KEY", "website": "https://openrouter.ai",
     "note": "国际聚合，可访问 Claude / GPT / Gemini"},
    {"name": "xAI Grok", "base_url": "https://api.x.ai/v1", "format": "openai",
     "models": ["grok-2-latest", "grok-beta"],
     "env_key": "XAI_API_KEY", "website": "https://x.ai/api"},
    {"name": "Groq", "base_url": "https://api.groq.com/openai/v1", "format": "openai",
     "models": ["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
     "env_key": "GROQ_API_KEY", "website": "https://console.groq.com"},
    {"name": "Mistral", "base_url": "https://api.mistral.ai/v1", "format": "openai",
     "models": ["mistral-large-latest", "mistral-small-latest"],
     "env_key": "MISTRAL_API_KEY", "website": "https://console.mistral.ai"},
    {"name": "Gemini (Google)", "base_url": "https://generativelanguage.googleapis.com/v1beta/openai/", "format": "openai",
     "models": ["gemini-1.5-pro", "gemini-1.5-flash", "gemini-2.0-flash"],
     "env_key": "GEMINI_API_KEY", "website": "https://ai.google.dev/",
     "note": "通过 Google 的 OpenAI 兼容端点访问"},
    {"name": "火山方舟 Ark", "base_url": "https://ark.cn-beijing.volces.com/api/v3", "format": "openai",
     "models": ["doubao-1-5-pro-32k-250115", "doubao-seed-1-6-flash"],
     "env_key": "ARK_API_KEY", "website": "https://console.volcengine.com/ark",
     "note": "字节跳动豆包"},
    {"name": "腾讯混元 Hunyuan", "base_url": "https://api.hunyuan.cloud.tencent.com/v1", "format": "openai",
     "models": ["hunyuan-turbo", "hunyuan-lite"],
     "env_key": "HUNYUAN_API_KEY", "website": "https://cloud.tencent.com/product/hunyuan"},
    {"name": "Ollama (本地)", "base_url": "http://localhost:11434/v1", "format": "openai",
     "models": ["llama3.1", "qwen2.5:7b", "gemma2:9b"],
     "env_key": "", "website": "https://ollama.com",
     "note": "本机 Ollama，免费离线（需先安装并启动 Ollama）"},
    {"name": "Anthropic Claude", "base_url": "https://api.anthropic.com", "format": "anthropic",
     "models": ["claude-sonnet-4-5", "claude-3-5-haiku-latest", "claude-opus-4-1"],
     "env_key": "ANTHROPIC_API_KEY", "website": "https://console.anthropic.com",
     "note": "Anthropic 官方 Messages 协议（国内直连需代理）"},
]

# 动作预设（前端会附带，服务端兜底一份，防止直接调 API 时缺少）
ACTIONS = {
    "quick_read": ("快速阅读", "你是 ReadMD 的文档阅读助手。对用户给出的 Markdown 文档做快速阅读，"
                   "输出：1) 一句话概述；2) 核心要点列表；3) 文档结构目录；4) 值得注意的细节或疑问。使用 Markdown 格式。"),
    "polish": ("润色", "你是资深中文编辑。润色用户给出的 Markdown 文档：修正错别字、病句、表达生硬之处，"
               "保留原有结构与全部 Markdown 标记，只输出润色后的完整文档，不要加任何解释。"),
    "modify": ("修改", "你是文档修订助手。根据用户要求修改文档，修正明显错误（错别字、标点、Markdown 格式错误）。"
               "只输出修改后的完整文档，不要加任何解释。"),
    "expand": ("扩充", "你是文档扩充助手。在保持原有结构与语气的前提下，为文档补充细节、示例、解释，使内容更丰富。"
               "只输出扩充后的完整文档，不要加任何解释。"),
    "continue": ("续写", "你是文档续写助手。从文档末尾自然延续写作，保持风格一致。只输出续写的新增内容，不要重复原文。"),
    "translate": ("翻译", "你是专业翻译。将用户给出的文档翻译成指定语言，保留 Markdown 结构、表格与代码块，只输出译文。"),
    "ask": ("问答", "你是文档问答助手。基于用户给出的文档内容回答问题；文档中没有的内容请明确说明。"),
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
    if cfg.get("schema_version") != CONFIG_SCHEMA_VERSION:
        cfg = {"schema_version": CONFIG_SCHEMA_VERSION, "providers": [], "current": {}}
        _write_cfg(cfg)
    else:
        cfg.setdefault("providers", [])
        cfg.setdefault("current", {})
        changed = False
        for provider in cfg["providers"]:
            if isinstance(provider, dict) and not provider.get("id"):
                provider["id"] = "custom:" + uuid.uuid4().hex
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
    try:
        os.makedirs(DATA_DIR, exist_ok=True)
        tmp = CONFIG_FILE + ".tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
        os.replace(tmp, CONFIG_FILE)
    except Exception as e:
        logging.exception("ai config save failed")
        raise


def get_config():
    cfg = ensure_config()
    custom = cfg.get("providers", [])

    def annotate(p):
        d = dict(p)
        d.setdefault("id", "preset:" + str(d.get("name") or "provider"))
        d["has_key"] = bool(resolve_key(d))
        d["key_source"] = key_source(d)
        d["mode"] = p.get("mode") or ("messages" if p.get("format") == "anthropic" else "auto")
        # 配置接口只提供状态，绝不把保存在磁盘中的 API Key 回传给前端。
        d.pop("api_key", None)
        return d

    presets = [annotate(dict(p)) for p in PRESETS]
    customs = [annotate(dict(p)) for p in custom]
    return {"schema_version": CONFIG_SCHEMA_VERSION, "presets": presets,
            "custom": customs, "current": cfg.get("current", {})}


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
            # 前端不会收到旧 Key；编辑其它字段时保留原 Key。只有显式标记才清除。
            previous = old.get(provider_id) or old_by_name.get(name) or {}
            if p.get("api_key"):
                p["api_key"] = encrypt_api_key(p["api_key"])
            elif previous.get("api_key") and not p.pop("clear_key", False):
                p["api_key"] = previous["api_key"]
            else:
                p.pop("clear_key", None)
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


def resolve_key(p):
    """API Key：界面配置 > 环境变量。"""
    if p.get("api_key"):
        return decrypt_api_key(p["api_key"])
    env = p.get("env_key") or ""
    if env:
        v = os.environ.get(env, "")
        if v:
            return v
    return ""


def key_source(p):
    if p.get("api_key"):
        return "configured"
    env = p.get("env_key") or ""
    if env and os.environ.get(env):
        return "env:" + env
    return ""


class ChatError(Exception):
    pass


def _http_json(url, headers, body, timeout=240):
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:500]
        raise ChatError("HTTP %d：%s" % (e.code, detail)) from e
    except urllib.error.URLError as e:
        raise ChatError("网络错误：%s" % e.reason) from e


def _http_stream(url, headers, body, timeout=300):
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers=headers, method="POST")
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
    if not any(m["role"] == "system" for m in out):
        out.insert(0, {"role": "system", "content": "You are ReadMD, a helpful Markdown document assistant."})
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


def chat(payload):
    """调用大模型。payload: {provider?, base_url?, format?, api_key?, model,
    messages, temperature?, stream?, mode?}。统一返回生成器：产出 str 增量，
    末尾可产出 {'usage': {...}} 用量事件。mode: auto|chat|completion|responses|messages。"""
    name = payload.get("provider") or ""
    prov = find_provider(name) if name else {}
    if not prov and name:
        raise ChatError("未知提供商：%s" % name)
    if not prov:
        prov = {}
    base_url = (payload.get("base_url") or prov.get("base_url") or "").rstrip("/")
    fmt = payload.get("format") or prov.get("format") or "openai"
    api_key = payload.get("api_key") or resolve_key(prov)
    if not api_key:
        raise ChatError("未配置 API Key（可填入界面，或设置环境变量 %s）" % (prov.get("env_key") or ""))
    model = payload.get("model") or (prov.get("models") or [""])[0] or ""
    messages = payload.get("messages") or []
    temperature = payload.get("temperature", 0.4)
    stream = bool(payload.get("stream", True))
    mode = (payload.get("mode") or prov.get("mode") or "").strip().lower()
    if not mode:
        mode = "messages" if fmt == "anthropic" else "auto"

    if mode in ("messages", "anthropic"):
        return _chat_anthropic(base_url, api_key, model, messages, temperature, stream)
    if mode == "completion":
        return _chat_openai_completion(base_url, api_key, model, messages, temperature, stream)
    if mode == "responses":
        return _chat_openai_responses(base_url, api_key, model, messages, temperature, stream)
    return _chat_openai(base_url, api_key, model, messages, temperature, stream)
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


def _chat_openai(base_url, api_key, model, messages, temperature, stream):
    url = _normalize_base_url(base_url, "chat/completions")
    body = {"model": model, "messages": _openai_messages(messages), "stream": stream, "temperature": temperature}
    if stream:
        body["stream_options"] = {"include_usage": True}
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + api_key}

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
                if choices[0].get("finish_reason"):
                    break
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


def _chat_openai_completion(base_url, api_key, model, messages, temperature, stream):
    url = _normalize_base_url(base_url, "completions")
    msgs = _openai_messages(messages)
    prompt_text = "\n\n".join(m.get("content", "") for m in msgs if m.get("content"))
    body = {"model": model, "prompt": prompt_text, "max_tokens": 4096,
            "temperature": temperature, "stream": stream}
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + api_key}

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
                if choices[0].get("finish_reason"):
                    break
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


def _chat_openai_responses(base_url, api_key, model, messages, temperature, stream):
    url = _normalize_base_url(base_url, "responses")
    msgs = []
    for m in _openai_messages(messages):
        if m["role"] == "system":
            msgs.insert(0, {"role": "system", "content": m["content"]})
        else:
            msgs.append(m)
    body = {"model": model, "input": msgs, "stream": stream, "temperature": temperature}
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + api_key}

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


def _chat_anthropic(base_url, api_key, model, messages, temperature, stream):
    url = _normalize_base_url(base_url, "v1/messages")
    system, msgs = _anthropic_messages(messages)
    body = {"model": model, "max_tokens": 4096, "messages": msgs, "temperature": temperature, "stream": stream}
    if system:
        body["system"] = system
    headers = {"Content-Type": "application/json", "x-api-key": api_key, "anthropic-version": "2023-06-01"}

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
    req = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.read().decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", "replace")[:500]
        raise ChatError("HTTP %d：%s" % (e.code, detail)) from e
    except urllib.error.URLError as e:
        raise ChatError("网络错误：%s" % e.reason) from e


def list_models(base_url, api_key, mode="auto"):
    """通过 API Key 获取可用模型列表。OpenAI 兼容 GET {base}/models；Anthropic GET {base}/v1/models。"""
    base_url = (base_url or "").rstrip("/")
    if not base_url:
        raise ChatError("请先填写 Base URL")
    if not api_key:
        raise ChatError("请先填写 API Key 再获取模型列表")
    mode = (mode or "").strip().lower()
    if mode in ("messages", "anthropic"):
        url = base_url + "/v1/models"
        headers = {"x-api-key": api_key, "anthropic-version": "2023-06-01"}
    else:
        url = base_url + "/models"
        headers = {"Authorization": "Bearer " + api_key}
    data = _http_get_json(url, headers)
    try:
        d = json.loads(data)
    except Exception as e:
        raise ChatError("模型列表解析失败：%s" % data[:300]) from e
    items = d.get("data") or []
    ids = []
    for it in items:
        if isinstance(it, dict) and it.get("id"):
            ids.append(str(it["id"]))
    if not ids:
        raise ChatError("接口未返回模型列表（data 为空）")
    return ids
