# ReadMD v2.3.8 更新清单（小红书与多平台发布版）

**主标题**：没装Office，也能批量把文档转成MD  
**副标题**：统一批量工作台 · 原生 OOXML / 纯 Python OLE2 解析 · 零云端依赖  
**话题标签**：#Markdown #生产力工具 #知识管理 #开源软件 #Obsidian #科研日常 #程序员日常 #效率神器 #文档转换 #本地优先  

---

## 核心版本特性与证据链

1. **统一批量工作台 (Batch Workbench)**
   - 支持批量任务取消与逐行进度追溯，中途安全退出不锁死进程，已完成文件不丢失。
   - 证据：`src/readmd_modules/converter.py`、`assets/js/features/batch.js`。

2. **脱离 Office COM 依赖 (Native Engine)**
   - PPTX / XLSX 改用原生 OOXML 读取器；旧版 .doc 引入纯 Python OLE2 解析器。
   - 彻底摆脱 Windows Office COM 进程冲突与后台卡死，跨系统（Win/Mac/Linux）原生秒转。
   - 证据：`src/readmd_modules/converters/office_native.py`。

3. **科学图表沙箱与离线化 (Diagram Sandbox)**
   - Chart.js 纯离线渲染；Vega 与 Vega-Lite 图表由独立服务端 Sidecar 沙箱隔离渲染，杜绝 eval 与外链数据窃取；PlantUML 远程渲染前显式确认。
   - 证据：`src/readmd_modules/diagrams/vega_sidecar.py`。

4. **VS Code 扩展与 MCP 生态 (Ecosystem Integration)**
   - VS Code 扩展补齐至 22 项原生命令，支持动态技能流式取消与核心连接状态可视化；MCP Server 深度接入本地大模型。
   - 证据：`packages/vscode-extension/`、`packages/mcp-server/`。

5. **Zip 解压沙箱与边界安全 (Security & Zip Sandbox)**
   - 全面拦截路径穿越、UNC 伪造与 Zip 炸弹；解压条目与体积设上限，大型二进制解压需显式授权，失败信息自动脱敏。
   - 证据：`src/readmd_modules/security/archive.py`。

6. **AI 生成选区保护与无障碍体验 (Interaction & AI Protection)**
   - AI 流式生成期间严格锁定用户文本选区与派生文档保存授权；分层 Escape 退出；修复 41 种语言“请先选中文本”原生提示。
   - 证据：`assets/js/features/ai.js`、`assets/i18n/`。

7. **46 种语言完整收敛 (Localization)**
   - 系统托盘、技能导入、图表消息、EPUB/LaTeX 导出配置全量本地化，杜绝生硬的英文回退。
   - 证据：`assets/i18n/`。
