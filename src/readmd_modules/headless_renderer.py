# -*- coding: utf-8 -*-
"""ReadMD Out-of-Process Headless Web Renderer.

Runs in an isolated subprocess to safely render dynamic SPAs (Gemini, ChatGPT, DeepSeek, etc.)
using the native system WebView2 without thread collisions with the main application UI.
"""

from __future__ import absolute_import

import json
import os
import sys
import time


def render_url_isolated(url, timeout_sec=25):
    """Render a URL in an isolated process using native WebView and return JSON."""
    try:
        import webview
    except ImportError:
        return {'ok': False, 'error': 'pywebview not installed in environment'}

    result_container = {'data': None}

    def on_loaded(window):
        # 轮询等待客户端 JS 加载完成
        start_time = time.time()
        last_turns_count = 0
        stable_count = 0

        while time.time() - start_time < timeout_sec:
            time.sleep(0.8)
            try:
                # 滚动到底部触发懒加载
                window.evaluate_js("window.scrollTo(0, document.body ? document.body.scrollHeight : 0);")
                
                check = window.evaluate_js("""
                    (() => {
                        const turns = [];
                        const nodes = document.querySelectorAll('user-query, model-response, div[class*="user-query"], div[class*="model-response"], div[class*="query-content"], div[class*="response-content"], div[data-test-id="conversation-turn"], .chat-history > *');
                        
                        nodes.forEach(node => {
                            const tag = node.tagName.toLowerCase();
                            const cls = (node.className || '').toLowerCase();
                            const isUser = tag.includes('user') || cls.includes('user') || cls.includes('query');
                            const text = node.innerText || node.textContent || '';
                            if (text.trim().length > 1) {
                                const last = turns[turns.length - 1];
                                if (!last || last.text !== text.trim()) {
                                    turns.push({
                                        role: isUser ? 'user' : 'assistant',
                                        text: text.trim()
                                    });
                                }
                            }
                        });

                        return {
                            ready: document.readyState,
                            title: document.title,
                            turns_count: turns.length,
                            text_len: document.body ? document.body.innerText.length : 0,
                            turns: turns
                        };
                    })()
                """)
                
                if check and isinstance(check, dict):
                    turns_count = check.get('turns_count', 0)
                    if turns_count > 0 and turns_count == last_turns_count:
                        stable_count += 1
                        if stable_count >= 2:
                            # 已经稳定获取全部轮次
                            break
                    else:
                        stable_count = 0
                    last_turns_count = turns_count
            except Exception:
                pass

        # 获取最终完整的 HTML 与对话结构
        try:
            final_data = window.evaluate_js("""
                (() => {
                    const turns = [];
                    const nodes = document.querySelectorAll('user-query, model-response, div[class*="user-query"], div[class*="model-response"], div[class*="query-content"], div[class*="response-content"], div[data-test-id="conversation-turn"]');
                    
                    nodes.forEach(node => {
                        const tag = node.tagName.toLowerCase();
                        const cls = (node.className || '').toLowerCase();
                        const isUser = tag.includes('user') || cls.includes('user') || cls.includes('query');
                        const text = node.innerText || node.textContent || '';
                        if (text.trim().length > 0) {
                            const last = turns[turns.length - 1];
                            if (!last || last.text !== text.trim()) {
                                turns.push({
                                    role: isUser ? 'user' : 'assistant',
                                    text: text.trim()
                                });
                            }
                        }
                    });

                    return {
                        ok: true,
                        title: document.title || '',
                        final_url: location.href,
                        html: document.documentElement.outerHTML,
                        turns: turns,
                        turns_count: turns.length
                    };
                })()
            """)
            result_container['data'] = final_data
        except Exception as exc:
            result_container['data'] = {'ok': False, 'error': str(exc)}
        finally:
            try:
                window.destroy()
            except Exception:
                pass

    try:
        window = webview.create_window(
            'ReadMD Headless Renderer',
            url,
            hidden=True,
            width=1280,
            height=900
        )
        webview.start(on_loaded, window, private_mode=False)
    except Exception as exc:
        return {'ok': False, 'error': str(exc)}

    return result_container['data'] or {'ok': False, 'error': '渲染超时或无响应'}


if __name__ == '__main__':
    if len(sys.argv) < 2:
        sys.stderr.write("Usage: python -m src.readmd_modules.headless_renderer <url> [timeout]\n")
        sys.exit(1)
    target_url = sys.argv[1]
    timeout = int(sys.argv[2]) if len(sys.argv) > 2 else 25
    res = render_url_isolated(target_url, timeout_sec=timeout)
    print(json.dumps(res, ensure_ascii=False))
