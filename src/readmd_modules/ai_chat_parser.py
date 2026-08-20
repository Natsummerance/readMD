# -*- coding: utf-8 -*-
"""ReadMD AI Chat Share Parser.

Extracts structured conversation transcripts from AI chat share links including:
- Google Gemini (share.gemini.google, gemini.google.com/share)
- OpenAI ChatGPT (chatgpt.com/share, chat.openai.com/share)
- Anthropic Claude (claude.ai/share)
- DeepSeek (chat.deepseek.com/share)
- Kimi / Moonshot (kimi.moonshot.cn/share, kimi.ai/share)
- Perplexity AI (perplexity.ai/search, perplexity.ai/page)
- Baidu ERNIE / Yiyan, Alibaba Tongyi, ByteDance Doubao, Zhipu GLM
- Generic conversation turns in DOM
"""

from __future__ import absolute_import

import json
import re
from urllib.parse import urlparse
from bs4 import BeautifulSoup, Comment
from markdownify import markdownify as _md


# AI Platform metadata
AI_PLATFORMS = {
    'gemini': {
        'name': 'Google Gemini',
        'icon': '♊',
        'role_assistant': 'Gemini',
        'hosts': ('share.gemini.google', 'gemini.google.com', 'bard.google.com')
    },
    'chatgpt': {
        'name': 'OpenAI ChatGPT',
        'icon': '🤖',
        'role_assistant': 'ChatGPT',
        'hosts': ('chatgpt.com', 'chat.openai.com')
    },
    'claude': {
        'name': 'Anthropic Claude',
        'icon': '🟣',
        'role_assistant': 'Claude',
        'hosts': ('claude.ai',)
    },
    'deepseek': {
        'name': 'DeepSeek',
        'icon': '🐋',
        'role_assistant': 'DeepSeek',
        'hosts': ('chat.deepseek.com', 'deepseek.com')
    },
    'kimi': {
        'name': 'Kimi AI',
        'icon': '🌙',
        'role_assistant': 'Kimi',
        'hosts': ('kimi.moonshot.cn', 'kimi.ai')
    },
    'perplexity': {
        'name': 'Perplexity AI',
        'icon': '🔍',
        'role_assistant': 'Perplexity',
        'hosts': ('perplexity.ai', 'www.perplexity.ai')
    },
    'doubao': {
        'name': '豆包 (Doubao)',
        'icon': '🌱',
        'role_assistant': '豆包',
        'hosts': ('doubao.com', 'www.doubao.com')
    },
    'tongyi': {
        'name': '通义千问 (Qwen)',
        'icon': '🔮',
        'role_assistant': '通义千问',
        'hosts': ('tongyi.aliyun.com', 'qianwen.aliyun.com')
    },
    'yiyan': {
        'name': '文心一言 (ERNIE)',
        'icon': '✨',
        'role_assistant': '文心一言',
        'hosts': ('yiyan.baidu.com',)
    },
    'chatglm': {
        'name': '智谱清言 (GLM)',
        'icon': '💡',
        'role_assistant': '智谱清言',
        'hosts': ('chatglm.cn',)
    }
}


def detect_ai_platform(url):
    """Detect AI platform identifier from URL."""
    if not url:
        return 'generic', None
    parsed = urlparse(url.lower())
    host = parsed.netloc.split(':')[0]
    for plat_id, info in AI_PLATFORMS.items():
        if any(h == host or host.endswith('.' + h) for h in info['hosts']):
            return plat_id, info
    return 'generic', None


def is_ai_chat_url(url):
    """Check if URL points to an AI chat share link."""
    if not url:
        return False
    plat_id, _ = detect_ai_platform(url)
    if plat_id != 'generic':
        return True
    path = urlparse(url.lower()).path
    return '/share/' in path or '/share' in path or '/chat/' in path


def _clean_html_node(node):
    """Clean unwanted tags from an HTML node before markdown conversion."""
    if not node:
        return ''
    for tag in node.find_all(['script', 'style', 'noscript', 'svg', 'button', 'input']):
        tag.decompose()
    for comment in node.find_all(string=lambda text: isinstance(text, Comment)):
        comment.extract()
    return str(node)


def _html_to_clean_md(html_str):
    """Convert HTML snippet to clean Markdown."""
    if not html_str:
        return ''
    md = _md(
        html_str,
        heading_style='atx',
        bullets='-',
        code_language='',
        strip=['script', 'style', 'button']
    )
    md = re.sub(r'\n{3,}', '\n\n', md).strip()
    return md


def _clean_chat_tokens(text):
    """Clean internal token markers from text."""
    if not text:
        return ''
    text = re.sub(r'entity\[.*?\]', '', text)
    text = re.sub(r'cite.*?', '', text)
    text = re.sub(r'url.*?', '', text)
    return text.strip()


def _detect_node_role(node):
    """Inspect node tag, classes, and children to determine user vs assistant role."""
    if not node:
        return 'assistant'
    tag_name = getattr(node, 'name', '').lower()
    classes = ' '.join(node.get('class', [])).lower()
    role_attr = (node.get('data-message-author-role') or node.get('data-role') or node.get('data-testid') or '').lower()
    
    if any(w in role_attr for w in ('user', 'human', 'prompt')):
        return 'user'
    if any(w in role_attr for w in ('assistant', 'model', 'claude', 'bot', 'gemini')):
        return 'assistant'
        
    if tag_name in ('user-query', 'user-message', 'prompt'):
        return 'user'
    if tag_name in ('model-response', 'claude-message', 'assistant-message'):
        return 'assistant'
        
    if node.find(['user-query', 'div'], class_=re.compile(r'user-query|query-content|user-prompt', re.I)):
        return 'user'
    if node.find(['model-response', 'div'], class_=re.compile(r'model-response|response-content', re.I)):
        return 'assistant'
        
    if any(w in classes for w in ('user-query', 'query-content', 'user-turn', 'user-prompt', 'user-message', 'human')):
        return 'user'
    if any(w in classes for w in ('model-response', 'response-content', 'model-turn', 'response-container', 'assistant')):
        return 'assistant'
        
    return 'assistant'


def _unflatten_turbo_stream(arr):
    """Unflatten Turbo-Stream serialized array into Python object structure."""
    memo = {}

    def resolve(idx):
        if not isinstance(idx, int):
            return idx
        if idx < 0 or idx >= len(arr):
            return None
        if idx in memo:
            return memo[idx]

        val = arr[idx]
        if isinstance(val, (int, float, bool, str)) or val is None:
            return val

        if isinstance(val, list):
            res_list = []
            memo[idx] = res_list
            for item in val:
                res_list.append(resolve(item))
            return res_list

        if isinstance(val, dict):
            res_dict = {}
            memo[idx] = res_dict
            for k, v in val.items():
                if k.startswith("_"):
                    try:
                        key_idx = int(k[1:])
                        key_name = resolve(key_idx)
                        val_resolved = resolve(v)
                        if isinstance(key_name, str):
                            res_dict[key_name] = val_resolved
                    except Exception:
                        pass
                else:
                    res_dict[k] = resolve(v)
            return res_dict

        return val

    return resolve(0)


def _find_key_recursive(obj, target_key, visited=None):
    """Recursively search for a key in a deserialized nested dictionary graph."""
    if visited is None:
        visited = set()
    obj_id = id(obj)
    if obj_id in visited:
        return None
    visited.add(obj_id)

    if isinstance(obj, dict):
        if target_key in obj:
            return obj[target_key]
        for v in obj.values():
            res = _find_key_recursive(v, target_key, visited)
            if res is not None:
                return res
    elif isinstance(obj, list):
        for item in obj:
            res = _find_key_recursive(item, target_key, visited)
            if res is not None:
                return res
    return None


def parse_chatgpt_json(html):
    """Extract conversation from ChatGPT (supports Next.js SSR JSON & React Router streaming)."""
    soup = BeautifulSoup(html, 'lxml')
    
    title = 'ChatGPT 对话'
    if soup.title and soup.title.string:
        clean_t = soup.title.string.replace('ChatGPT - ', '').replace(' - ChatGPT', '').strip()
        if clean_t:
            title = clean_t
            
    # 策略 1: React Router / Remix streaming streamController.enqueue (Turbo-Stream)
    raw_chunks = []
    for s in soup.find_all("script"):
        st = s.string or s.text or ""
        if "streamController.enqueue" in st:
            for part in st.split("streamController.enqueue(")[1:]:
                idx = part.rfind(")")
                if idx != -1:
                    inner = part[:idx].strip()
                    if inner.startswith('"') and inner.endswith('"'):
                        try:
                            raw_chunks.append(json.loads(inner))
                        except Exception:
                            pass
                            
    if raw_chunks:
        full_stream = "".join(raw_chunks)
        lines = full_stream.split("\n")
        try:
            raw_array = json.loads(lines[0])
            if isinstance(raw_array, list):
                unflattened = _unflatten_turbo_stream(raw_array)
                stream_title = _find_key_recursive(unflattened, 'pageTitle') or _find_key_recursive(unflattened, 'title')
                if stream_title and isinstance(stream_title, str) and stream_title.strip():
                    title = stream_title.strip()
                    
                linear = _find_key_recursive(unflattened, 'linear_conversation')
                if linear and isinstance(linear, list):
                    turns = []
                    for item in linear:
                        if isinstance(item, dict):
                            msg = item.get('message') or item
                            author = msg.get('author') or {}
                            role = author.get('role') if isinstance(author, dict) else None
                            content = msg.get('content') or {}
                            parts = content.get('parts') or [] if isinstance(content, dict) else []
                            text_parts = []
                            for p in parts:
                                if isinstance(p, (str, int, float)):
                                    text_parts.append(_clean_chat_tokens(str(p)))
                                elif isinstance(p, dict) and p.get('text'):
                                    text_parts.append(_clean_chat_tokens(str(p['text'])))
                            final_text = '\n\n'.join(text_parts).strip()
                            if role in ('user', 'assistant') and final_text:
                                turns.append({'role': role, 'text': final_text})
                    if turns:
                        return {'title': title, 'turns': turns}
        except Exception:
            pass

    # 策略 2: 传统 Next.js __NEXT_DATA__ JSON
    script = soup.find('script', id='__NEXT_DATA__')
    if script and script.string:
        try:
            data = json.loads(script.string)
            page_props = data.get('props', {}).get('pageProps', {})
            server_resp = page_props.get('serverResponse', {}).get('data', {})
            title = page_props.get('title') or server_resp.get('title') or title
            
            linear_conv = server_resp.get('linear_conversation')
            turns = []
            if isinstance(linear_conv, list):
                for item in linear_conv:
                    msg = item.get('message', {})
                    role = msg.get('author', {}).get('role')
                    if role not in ('user', 'assistant'):
                        continue
                    content_obj = msg.get('content', {})
                    parts = content_obj.get('parts', [])
                    text = '\n\n'.join(_clean_chat_tokens(str(p)) for p in parts if isinstance(p, (str, int, float)))
                    if text.strip():
                        turns.append({
                            'role': 'user' if role == 'user' else 'assistant',
                            'text': text.strip()
                        })
            elif 'mapping' in server_resp:
                mapping = server_resp['mapping']
                for node in mapping.values():
                    msg = node.get('message')
                    if not msg:
                        continue
                    role = msg.get('author', {}).get('role')
                    if role not in ('user', 'assistant'):
                        continue
                    parts = msg.get('content', {}).get('parts', [])
                    text = '\n\n'.join(_clean_chat_tokens(str(p)) for p in parts if isinstance(p, (str, int, float)))
                    if text.strip():
                        turns.append({
                            'role': 'user' if role == 'user' else 'assistant',
                            'text': text.strip()
                        })
            if turns:
                return {'title': title, 'turns': turns}
        except Exception:
            pass
            
    return None


def parse_claude_json(html):
    """Extract conversation from Claude __NEXT_DATA__ SSR JSON."""
    soup = BeautifulSoup(html, 'lxml')
    script = soup.find('script', id='__NEXT_DATA__')
    if not script or not script.string:
        return None
    try:
        data = json.loads(script.string)
        page_props = data.get('props', {}).get('pageProps', {})
        chat = page_props.get('chat') or page_props.get('sharedConversation') or {}
        title = chat.get('name') or chat.get('title') or page_props.get('title') or 'Claude 对话'
        raw_msgs = chat.get('chat_messages') or chat.get('transcript') or []
        turns = []
        for msg in raw_msgs:
            sender = msg.get('sender') or msg.get('role')
            text = msg.get('text') or ''
            if not text and 'content' in msg:
                for c in msg['content']:
                    if isinstance(c, dict) and c.get('type') == 'text':
                        text += c.get('text', '') + '\n'
            if sender in ('human', 'user') and text.strip():
                turns.append({'role': 'user', 'text': _clean_chat_tokens(text.strip())})
            elif sender in ('assistant', 'claude') and text.strip():
                turns.append({'role': 'assistant', 'text': _clean_chat_tokens(text.strip())})
        if turns:
            return {'title': title, 'turns': turns}
    except Exception:
        pass
    return None


def parse_gemini_dom_or_json(html):
    """Extract conversation from Google Gemini DOM or SSR payload (supports window.WIZ_global_data)."""
    soup = BeautifulSoup(html, 'lxml')
    title = ''
    if soup.title and soup.title.string:
        title = soup.title.string.replace(' - Gemini', '').replace('Gemini - ', '').replace('‎Gemini - ', '').strip()
    if not title or title.lower() in ('gemini', 'live content', '直接体验 google ai 黑科技'):
        title = 'Gemini 对话'

    turns = []

    # 策略 1: 解析 Google WIZ_global_data SSR 状态数据 (无需弹窗，极速且 100% 完整)
    target_script = None
    for s in soup.find_all("script"):
        st = s.string or s.text or ""
        if "window.WIZ_global_data" in st:
            target_script = st
            break

    if target_script:
        start = target_script.find("{")
        end = target_script.rfind("}")
        if start != -1 and end != -1:
            try:
                wiz = json.loads(target_script[start:end+1])
                for k, v in wiz.items():
                    if isinstance(v, str) and "∞" in v and len(v) > 50:
                        # 可能包含通过 ∰ 分隔的多轮或多个示例
                        sections = v.split("∰")
                        for sec in sections:
                            sec = sec.strip()
                            if not sec or "∞" not in sec:
                                continue
                            parts = [p.strip() for p in sec.split("∞") if p.strip()]
                            user_parts = []
                            image_parts = []
                            response_parts = []
                            for p in parts:
                                if p.startswith("http://") or p.startswith("https://"):
                                    image_parts.append(f"![Gemini Image]({p})")
                                elif not response_parts and not image_parts:
                                    user_parts.append(p)
                                else:
                                    response_parts.append(p)

                            user_text = "\n\n".join(user_parts).strip()
                            response_text = "\n\n".join(response_parts).strip()
                            if image_parts:
                                response_text = "\n\n".join(image_parts) + ("\n\n" + response_text if response_text else "")

                            if user_text and response_text:
                                if title == 'Gemini 对话':
                                    first_line = user_text.split('\n')[0].strip()
                                    if first_line and len(first_line) < 60:
                                        title = first_line
                                turns.append({'role': 'user', 'text': _clean_chat_tokens(user_text)})
                                turns.append({'role': 'assistant', 'text': _clean_chat_tokens(response_text)})
                        if turns:
                            return {'title': title, 'turns': turns}
            except Exception:
                pass

    # 策略 2: 扫描具体对话节点
    query_and_response_nodes = soup.find_all(['user-query', 'model-response'])
    if query_and_response_nodes:
        for node in query_and_response_nodes:
            role = 'user' if node.name == 'user-query' else 'assistant'
            md_content = _html_to_clean_md(_clean_html_node(node))
            if md_content:
                turns.append({'role': role, 'text': _clean_chat_tokens(md_content)})
                
    # 策略 3: 扫描外层容器
    if not turns:
        all_turns = soup.find_all(['div', 'article', 'section'], class_=re.compile(r'user-query-container|response-container|query-content|response-content|user-turn|model-turn|chat-turn|conversation-turn', re.I))
        for node in all_turns:
            role = _detect_node_role(node)
            md_content = _html_to_clean_md(_clean_html_node(node))
            if md_content and len(md_content) > 1:
                if not turns or turns[-1]['text'] != md_content:
                    turns.append({'role': role, 'text': _clean_chat_tokens(md_content)})

    # 策略 4: 扫描具有 data-message-author-role 标识的元素
    if not turns:
        for node in soup.find_all(['div', 'article', 'section'], attrs={'data-message-author-role': True}):
            role = node.get('data-message-author-role', '').lower()
            md = _html_to_clean_md(_clean_html_node(node))
            if md:
                turns.append({'role': 'user' if role == 'user' else 'assistant', 'text': _clean_chat_tokens(md)})

    if turns:
        return {'title': title, 'turns': turns}
    return None


def parse_generic_ai_chat_dom(html, url=''):
    """Generic AI chat DOM turns extractor."""
    soup = BeautifulSoup(html, 'lxml')
    title = soup.title.string.strip() if soup.title and soup.title.string else 'AI 对话记录'
    title = re.sub(r'\s*[-_|]\s*(Gemini|ChatGPT|Claude|DeepSeek|Kimi|Perplexity|豆包|通义千问).*$', '', title, flags=re.I).strip()
    
    turns = []
    
    # 1. 查找具体角色节点
    role_nodes = soup.find_all(lambda el: el.has_attr('data-message-author-role') or
                                          el.has_attr('data-role') or
                                          el.has_attr('data-testid') and ('message' in el['data-testid'] or 'turn' in el['data-testid']))
    for node in role_nodes:
        role = _detect_node_role(node)
        md = _html_to_clean_md(_clean_html_node(node))
        if md:
            turns.append({'role': role, 'text': _clean_chat_tokens(md)})
            
    # 2. 查找常见对话类名模式
    if not turns:
        candidates = soup.find_all(['div', 'section', 'article'], class_=re.compile(r'chat-message|message-item|conversation-item|chat-bubble|dialog-item|chat-turn', re.I))
        for c in candidates:
            role = _detect_node_role(c)
            md = _html_to_clean_md(_clean_html_node(c))
            if md:
                turns.append({'role': role, 'text': _clean_chat_tokens(md)})

    if turns:
        return {'title': title, 'turns': turns}
    return None


def format_ai_chat_markdown(chat_data, url=''):
    """Format structured AI chat data into professional Markdown document."""
    if not chat_data or not chat_data.get('turns'):
        return None
    
    plat_id, info = detect_ai_platform(url)
    plat_name = info['name'] if info else 'AI 对话'
    assistant_name = info['role_assistant'] if info else 'AI 助手'
    assistant_icon = info['icon'] if info else '🤖'
    
    title = chat_data.get('title') or f"{plat_name} 对话分享"
    turns = chat_data.get('turns', [])
    
    lines = [
        f"# {title}",
        "",
        f"> **平台**：{assistant_icon} {plat_name}  ",
        f"> **来源链接**：[{url}]({url})  " if url else "",
        f"> **对话轮数**：共 {len(turns)} 轮交互",
        "",
        "---",
        ""
    ]
    lines = [l for l in lines if l != ""]
    lines.append("")
    
    for i, turn in enumerate(turns, 1):
        role = turn.get('role', 'user')
        text = turn.get('text', '').strip()
        if not text:
            continue
            
        if role == 'user':
            lines.append(f"### 👤 用户 (User)")
            lines.append("")
            lines.append(text)
            lines.append("")
            lines.append("---")
            lines.append("")
        else:
            lines.append(f"### {assistant_icon} {assistant_name}")
            lines.append("")
            lines.append(text)
            lines.append("")
            lines.append("---")
            lines.append("")
            
    return '\n'.join(lines).strip() + '\n'


def try_parse_ai_chat(url, html):
    """Main entry point to parse any AI chat link into Markdown."""
    if not html:
        return None
        
    plat_id, _ = detect_ai_platform(url)
    
    chat_data = None
    if plat_id == 'chatgpt':
        chat_data = parse_chatgpt_json(html) or parse_generic_ai_chat_dom(html, url)
    elif plat_id == 'claude':
        chat_data = parse_claude_json(html) or parse_generic_ai_chat_dom(html, url)
    elif plat_id == 'gemini':
        chat_data = parse_gemini_dom_or_json(html) or parse_generic_ai_chat_dom(html, url)
    else:
        chat_data = (
            parse_chatgpt_json(html) or
            parse_claude_json(html) or
            parse_gemini_dom_or_json(html) or
            parse_generic_ai_chat_dom(html, url)
        )
        
    if chat_data and chat_data.get('turns'):
        return {
            'ok': True,
            'title': chat_data.get('title'),
            'turns_count': len(chat_data['turns']),
            'markdown': format_ai_chat_markdown(chat_data, url),
            'platform': plat_id
        }
        
    return None
