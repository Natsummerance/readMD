# ReadMD V2.3.7 开发交接与技术架构全景文档 (Developer Handoff)

## 📌 1. 项目基础信息

- **项目名称**：ReadMD (高性能本地 Markdown 智能阅读与排版套件)
- **当前候选版本**：`v2.3.7`（本地 RC；正式发布仍受原生平台证据、签名和启动门禁约束）
- **代码仓库结构**：
  ```
  readmd/
  ├── src/
  │   ├── readmd_core/          # 核心调度与基础架构 (配置/自愈引擎/TOC/SourceMap/CSS注入)
  │   │   ├── config.py         # 全局配置与版本号定义 (VERSION = '2.3.4')
  │   │   ├── readmd_fix.py     # 格式自愈与纠错管道
  │   │   ├── toc_engine.py     # [TOC] 目录树解析与原地自愈
  │   │   ├── source_map.py     # AST 行号映射与双锚点插值同步
  │   │   ├── style_injector.py # custom.css 与 head.html 样式注入
  │   │   └── ...
  │   └── readmd_modules/       # 进阶功能模块矩阵
  │       ├── import_processor.py      # @import 模块化处理器 (CSV/TSV/切片/PDF/LESS/TikZ)
  │       ├── diagrams.py              # 全景图表与绘图 (WaveDrom/Bitfield/Viz.js/PUML/TikZ)
  │       ├── code_chunk_runner.py     # 多语言安全 Code Chunk 执行器
  │       ├── mdexport/                # 排版级导出引擎
  │       │   ├── epub_render.py       # 原生 EPUB 3.0 电子书打包引擎 (纯标准库)
  │       │   ├── presentation_render.py # Reveal.js 演说模式脱机导出
  │       │   └── ...
  │       ├── convert.py               # 多格式文档转 Markdown (Word/PDF/PPT/Excel/TeX/HTML)
  │       ├── latex2omml.py            # LaTeX 转 Word 原生 OMML 节点
  │       ├── web.py                   # 网页智能抓取与降噪转 Markdown
  │       ├── ocr.py                   # 本地多引擎 OCR (WinRT/Vision/Paddle)
  │       ├── bibtex.py                # BibTeX 参考文献解析与卡片
  │       └── ...
  ├── packages/
  │   ├── mcp-server/           # MCP (Model Context Protocol) 15 项标准工具服务
  │   │   ├── readmd_mcp_server.py
  │   │   └── mcp_config_templates.json
  │   ├── vscode-extension/     # ReadMD VSCode 官方扩展源码
  │   │   ├── src/
  │   │   │   ├── extension.ts       # 命令注册与 Webview 同步渲染
  │   │   │   ├── bridge.ts          # VSCode ⇄ Python/MCP 通信网桥
  │   │   │   ├── sidebarProvider.ts # 侧边栏快捷工具箱
  │   │   │   └── pythonFinder.ts    # 虚拟环境与 Python 自动侦测
  │   │   └── package.json       # 扩展清单 (v2.3.7)
  │   └── harmonyos-app/        # HarmonyOS NEXT 纯血鸿蒙应用源码
  ├── assets/
  │   ├── i18n/                 # 46 国多语言本地化字典 (100.0% 词条对齐)
  │   └── vendor/               # 离线前端核心库 (Marked, KaTeX, Reveal.js 等)
  ├── tests/                    # 全仓 312+ 项自动化测试套件
  │   ├── test_extreme_stress_scenarios.py  # 6 大极限压力与多语言测试
  │   ├── test_integration_full_pipeline.py # 全链路复合文档端到端测试
  │   ├── test_mcp_server.py                # 15 项 MCP 工具调度测试
  │   ├── test_epub_export.py               # EPUB 3.0 打包测试
  │   ├── test_tikz_rendering.py            # TikZ 绘图测试
  │   ├── test_polyglot_code_chunk.py       # 多语言代码块测试
  │   └── ...
  └── dist/                     # 打包发布物输出目录
  ```

---

## 🚀 2. 核心研发与构建命令指南

### 2.1 运行全量测试套件 (CI/CD Quality Gate)
```bash
# 执行全仓 312+ 项单元测试与集成测试
python -m unittest discover -s tests -p "test_*.py"

# 执行核心功能与环境自检
python readmd.py --selftest

# 执行极限压力测试 (1000章节/万行表格/递归防御/并发沙箱/i18n审计)
python -m unittest tests/test_extreme_stress_scenarios.py

# 执行代码与资产隐私安全扫描
python tools/privacy_scan.py
```

### 2.2 构建与打包 VSCode 插件 (`.vsix`)
```bash
cd packages/vscode-extension
npm run compile
npx @vscode/vsce package --no-dependencies
# 生成文件位置：packages/vscode-extension/readmd-vscode-2.3.4.vsix
```

### 2.3 运行桌面客户端与 MCP 服务
```bash
# 启动桌面 GUI 客户端
python readmd.py

# 启动 MCP Server (stdio 模式，供智能体客户端接入)
python packages/mcp-server/readmd_mcp_server.py
```

---

## 💡 3. 关键模块技术决策与维护说明

1. **`@import` 安全架构**：
   - 必须通过 `abs_curr = os.path.abspath(target_path)` 与 `visited` 集合进行循环引用防御；
   - 强制最大嵌套深度 `MAX_IMPORT_DEPTH = 8`，防止恶意文件构造无限递归爆栈。
2. **EPUB 3.0 原生打包**：
   - 遵守 IDPF OCF 规范：`mimetype` 文件必须作为 ZIP 内的第一个条目，且必须以 `zipfile.ZIP_STORED`（无压缩）方式写入；
   - 依赖项保持纯 Python 标准库（`zipfile`, `html`, `uuid`, `datetime`），坚决不引入外部重量级 Pandoc/Calibre 依赖。
3. **AST 像素级双向同步**：
   - 行号由 `annotate_markdown_source_lines` 与 `inject_source_line_attributes_to_html` 预注入；
   - 滚动监听计算基于当前视口上方与下方的两个最近 `data-source-line` 进行线性插值（`Lerp`），确保行内折叠公式不产生累积误差。
4. **多语言 Code Chunk 执行**：
   - 子进程必须设置 `PYTHONIOENCODING=utf-8` 与 `PYTHONUTF8=1`，防止 Windows 平台非 ASCII 编码乱码；
   - 严格启用 10s 默认超时控制并在 `finally` 块中销毁进程。
