# VS Code 扩展 8 项核心缺陷治理实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 彻底修复 VS Code 扩展中 Astra 诊断出的 8 项核心缺陷（BUG-001 ~ BUG-008），全面覆盖数据完整性、配置容错、底层通信编码、异步看门狗超时、多语言执行及 Webview 渲染体验。

**Architecture:** 桥接通信层通过 `StringDecoder` 与双层超时看门狗加固；命令层通过快照版本校验与零依赖 JSONC 状态机清洗杜绝静默覆盖；Webview 预览层全面重塑为“单骨架加载 + postMessage 增量通信 + 200ms 防抖 + 本地资源 URI 转换”架构，彻底消除 script 截断注入漏洞与高频刷屏丢滚动问题。

**Tech Stack:** TypeScript, Node.js (`string_decoder`, `child_process`, `fs`), VS Code Extension API, node:test.

## Global Constraints

- **零运行时外部依赖约束**：遵循 `vsce package --no-dependencies`，严禁在 `package.json` 的 `dependencies` 引入新 npm 包。
- **无破坏性写入原则**：配置解析失败或文档版本不匹配时，坚决熔断并提示用户，严禁静默覆盖或清空配置。
- **完全向后兼容**：不破坏既有 29 项扩展测试与 Python 后端 85 项核心测试。

---

### Task 1: 桥接层多字节 UTF-8 跨 Chunk 拼包 (BUG-004)
### Task 2: 桥接层双超时流式看门狗与主动取消 (BUG-005)
### Task 3: openAiWorkbench 选区与文档版本快照防覆盖 (BUG-001)
### Task 4: setupMcpServer 零依赖 JSONC 清洗与熔断保护 (BUG-002)
### Task 5: runCodeChunk 多语言围栏提取与语言透传 (BUG-006)
### Task 6: Webview 现代消息架构、防抖与资源安全转换 (BUG-003, BUG-007, BUG-008)
### Task 7: 全套测试与生产打包验证
