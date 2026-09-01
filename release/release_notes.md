# ReadMD v2.3.8-preview.1 更新说明

ReadMD 是本地优先的 Markdown 阅读、编辑与格式转换工具。本版本聚焦 AI Skills、Provider 配置、跨端 Core Service、文档转换与启动体验。

## 正式支持矩阵与发布资产

| 系统 | 架构 | 交付物 | 资产文件名 |
| --- | --- | --- | --- |
| Windows 10/11 | x64、ARM64 | 候选安装版、便携版 | 需绑定当前提交的原生构建与签名证据 |
| macOS 13+ | Intel x64、Apple Silicon ARM64 | 候选原生压缩包 | 需绑定当前提交的签名、公证与冷启动证据 |
| Ubuntu 22.04/24.04、Debian 12 | x64、ARM64 | 候选 AppImage、Deb | `ReadMD-linux-x86_64-v2.3.8-preview.1.AppImage` / `ReadMD-linux-aarch64-v2.3.8-preview.1.AppImage`；需绑定对应系统的原生安装与功能证据 |
| 统信 UOS 20、银河麒麟 V10、Deepin 23 | x64、ARM64 | 目标 Deb | `readmd_2.3.8-preview.1_amd64.deb` / `readmd_2.3.8-preview.1_arm64.deb`；真实系统证据完成前不构成正式支持承诺 |
| VS Code | Extension Host 支持的桌面架构 | VSIX | `readmd-vscode-2.3.8-preview.1.vsix` |
| MCP 客户端 | Python 3.11+ / stdio | MCP ZIP | `readmd-mcp-server-2.3.8-preview.1.zip` |
| 校验清单 | 全架构 | SHA-256 | `SHA256SUMS.txt` |

HarmonyOS/OpenHarmony、Windows 7/8、LoongArch、MIPS、SW64、RISC-V、Alpine、AUR、Flatpak 和 Linglong 在本版本不属于正式支持范围。`packages/harmonyos-app` 仅保留为未支持的源码预览，不提供功能或兼容性承诺。

## 主要变化

- AI Provider 配置使用 schema v3 和 `credential_id`；密钥不进入 URL、日志、历史或导出文件。
- Provider 与 Skills 使用固定的离线来源快照；来源查看器只按清单 ID 读取，不接受任意路径或联网依赖。
- AI 系统指令统一从 ReadMD Skills 解析，普通用户 Skill 不执行脚本；AI 生成内容先以禁用草稿校验和试跑，再由用户明确发布。
- 桌面、MCP 和 VS Code 共享 Core Service、Provider、Credential、Skill、History 和 SSE 数据模型。
- Windows ARM64、macOS 双架构、Linux x64/ARM64 分别在目标系统构建；交付前执行安装、冷启动、离线、导出、OCR、AI 和隐私检查。
- 启动传输预算基线为 880,000 bytes；同时要求 `ready < 900ms`、FCP `< 400ms`、请求数 `≤31`，并以至少五次冷启动的最差值和 P95 验收。

## 离线来源与许可证

上游原文随包存放在 `assets/upstream/`，由 `assets/upstream/manifest.json` 固定逐文件 SHA-256。ReadMD 适配层位于 `assets/skills/` 和 `assets/providers/`，与原文严格分离。许可证和归属文件随快照保留；Research Paper Writing 为 ReadMD 独立重写，不复制无许可证来源内容。

## 发布前提

本文件描述发布合同，不代表未经验证的链接已经可下载。只有所有目标平台的原生构建、签名/公证、SBOM、许可证包、校验和、隐私扫描、全量测试和本地 RC 验收均通过，并收到候选哈希对应的用户确认后，才可创建 `v2.3.7` tag 或 GitHub Release。
