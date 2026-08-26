# -*- coding: utf-8 -*-
"""Unit tests for ReadMD AI Chat Share Parser."""

import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..'))

from src.readmd_modules import ai_chat_parser as AIChat
from src.readmd_modules import web as WEB


class TestAIChatParser(unittest.TestCase):

    def test_gemini_share_dom_parsing(self):
        html = '''<!DOCTYPE html>
        <html>
        <head><title>Gemini - 算法复杂度分析探讨</title></head>
        <body>
          <div class="chat-container">
            <div class="user-query-container">
              <user-query class="query-content">请解释快速排序算法的时间复杂度，并写出 Python 实现。</user-query>
            </div>
            <div class="response-container">
              <model-response class="response-content">
                <p>快速排序（Quicksort）的平均时间复杂度为 <strong>O(n log n)</strong>，最坏情况下为 O(n²)。</p>
                <pre><code class="language-python">def quicksort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    return quicksort(left) + middle + quicksort(right)
</code></pre>
              </model-response>
            </div>
          </div>
        </body>
        </html>'''
        url = 'https://share.gemini.google/YVjgvJlMjXZj'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'gemini')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('# 算法复杂度分析探讨', md)

    def test_gemini_game_demand_share_parsing(self):
        html = '''<!DOCTYPE html>
        <html>
        <head><title>微信小游戏需求梳理 - Gemini</title></head>
        <body>
          <main class="chat-history">
            <div class="user-query-container">
              <user-query class="query-content">我现在要用Anti Gravity 2去开发一个那种微信小城市的小游戏。现在我们来聊一聊我的需求,就玩我没有完全,你没有完全搞清楚我的需求之前,先不要停下来,先不断追问我,每次只问一个问题,每轮对话,让我详细描述,你详细记录,直到最终我们得出一个结论,一个开发的清单,功能清单,然后一些具体的细节,我后面继续补充。</user-query>
            </div>
            <div class="response-container">
              <model-response class="response-content">
                <p>已进入需求梳理流程。在完全明确所有功能与技术细节之前，将保持单轮单问推进。</p>
                <p><strong>第 1 问：</strong><br>这款小游戏的核心玩法与游戏类型是什么？（例如：放置模拟经营、城镇建造、合成消除、策略规划，或者其他复合机制？玩家在游戏中最核心的操作循环是怎样的？）</p>
              </model-response>
            </div>
          </main>
        </body>
        </html>'''
        url = 'https://share.gemini.google/kqeQNa9EDLTU'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'gemini')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('# 微信小游戏需求梳理', md)
        self.assertIn('用户 (User)', md)
        self.assertIn('Anti Gravity 2去开发一个那种微信小城市的小游戏', md)
        self.assertIn('Gemini', md)
        self.assertIn('已进入需求梳理流程', md)
        self.assertIn('这款小游戏的核心玩法与游戏类型是什么？', md)
        wiz_data = {
            "DnVkpd": "巴塞罗那最好的公园在哪里？\n请详细介绍这座城市最好的公园。∞https://www.gstatic.com/lamda/images/p1.jpg∞巴塞罗那是西班牙著名的旅游城市，以下是最好的公园简介：\n1. 奎尔公园 (Park Güell)\n奎尔公园是高迪的代表作。"
        }
        import json
        html = f'''<!DOCTYPE html>
        <html>
        <head><title>‎Gemini - 直接体验 Google AI 黑科技</title></head>
        <body>
          <script>
            window.WIZ_global_data = {json.dumps(wiz_data)};
          </script>
        </body>
        </html>'''
        url = 'https://share.gemini.google/kqeQNa9EDLTU'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'gemini')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('用户 (User)', md)
        self.assertIn('巴塞罗那最好的公园在哪里？', md)
        self.assertIn('Gemini', md)
        self.assertIn('奎尔公园 (Park Güell)', md)
        self.assertIn('![Gemini Image](https://www.gstatic.com/lamda/images/p1.jpg)', md)

    def test_chatgpt_share_json_parsing(self):
        next_data = {
            "props": {
                "pageProps": {
                    "title": "量子纠缠科普",
                    "serverResponse": {
                        "data": {
                            "title": "量子纠缠科普",
                            "linear_conversation": [
                                {
                                    "message": {
                                        "author": {"role": "user"},
                                        "content": {"parts": ["什么是量子纠缠？"]}
                                    }
                                },
                                {
                                    "message": {
                                        "author": {"role": "assistant"},
                                        "content": {"parts": ["量子纠缠是量子力学中一种奇特的现象，当几个粒子在彼此相互作用后，各个粒子所拥有的特性已综合成为整体性质。"]}
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        }
        import json
        html = f'''<!DOCTYPE html>
        <html>
        <head><title>ChatGPT - 量子纠缠科普</title></head>
        <body>
          <script id="__NEXT_DATA__" type="application/json">{json.dumps(next_data)}</script>
        </body>
        </html>'''
        url = 'https://chatgpt.com/share/66e01234-5678-abcd-ef01-23456789abcd'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'chatgpt')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('# 量子纠缠科普', md)
        self.assertIn('用户 (User)', md)
        self.assertIn('什么是量子纠缠？', md)
        self.assertIn('ChatGPT', md)
        self.assertIn('量子纠缠是量子力学中一种奇特的现象', md)

    def test_claude_share_json_parsing(self):
        next_data = {
            "props": {
                "pageProps": {
                    "chat": {
                        "name": "Rust 异步编程指南",
                        "chat_messages": [
                            {"sender": "human", "text": "Rust 中 async/await 的底层原理是什么？"},
                            {"sender": "assistant", "text": "在 Rust 中，async 函数会被编译器转换为实现 Future trait 的状态机。"}
                        ]
                    }
                }
            }
        }
        import json
        html = f'''<!DOCTYPE html>
        <html>
        <head><title>Claude - Rust 异步编程指南</title></head>
        <body>
          <script id="__NEXT_DATA__" type="application/json">{json.dumps(next_data)}</script>
        </body>
        </html>'''
        url = 'https://claude.ai/share/88ff1234-5678-abcd-ef01-23456789abcd'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'claude')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('# Rust 异步编程指南', md)
        self.assertIn('用户 (User)', md)
        self.assertIn('Claude', md)

    def test_web_extract_integration(self):
        html = '''<!DOCTYPE html>
        <html>
        <head><title>Gemini - 机器学习探讨</title></head>
        <body>
          <div class="chat-turn"><user-query>什么是深度学习？</user-query></div>
          <div class="chat-turn"><model-response>深度学习是机器学习的一个分支，基于人工神经网络。</model-response></div>
        </body>
        </html>'''
        url = 'https://share.gemini.google/test12345'
        result = WEB.extract_html(url, html)
        self.assertTrue(result['ok'])
        self.assertEqual(result['engine'], 'ai-chat-parser')
        self.assertIn('ai-chat-parser', result['engine_chain'])
    def test_chatgpt_remix_streaming_parsing(self):
        # 模拟 Turbo-stream 格式数据
        stream_data = [
            {"_1": 2, "_3": 4},
            "pageTitle",
            "电影艺术探讨",
            "linear_conversation",
            [5, 6],
            {
                "message": {
                    "author": {"role": "user"},
                    "content": {"parts": ["《去年在马里昂巴德》的艺术特色是什么？"]}
                }
            },
            {
                "message": {
                    "author": {"role": "assistant"},
                    "content": {"parts": ["该片是法国左岸派代表作，打破了传统的线性叙事与时空结构。"]}
                }
            }
        ]
        import json
        stream_json = json.dumps(json.dumps(stream_data))
        html = f'''<!DOCTYPE html>
        <html>
        <head><title>ChatGPT - 电影艺术探讨</title></head>
        <body>
          <script>
            window.__reactRouterContext.streamController.enqueue({stream_json});
          </script>
        </body>
        </html>'''
        url = 'https://chatgpt.com/share/6a83e896-9938-83ea-b5e6-01844a200c81'
        res = AIChat.try_parse_ai_chat(url, html)
        self.assertIsNotNone(res)
        self.assertTrue(res['ok'])
        self.assertEqual(res['platform'], 'chatgpt')
        self.assertEqual(res['turns_count'], 2)
        md = res['markdown']
        self.assertIn('# 电影艺术探讨', md)
        self.assertIn('用户 (User)', md)
        self.assertIn('《去年在马里昂巴德》的艺术特色是什么？', md)
        self.assertIn('ChatGPT', md)
        self.assertIn('该片是法国左岸派代表作', md)


if __name__ == '__main__':
    unittest.main()
