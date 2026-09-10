# ReadMD v2.3.8 全功能实机操作演示与视频录制资产库

本目录沉淀了 ReadMD v2.3.8 在 **插件中心沙箱**、**RapidOCR/RapidTable 离线识别**、**2D 力导向知识图谱与双链网络**、**学术 LaTeX 论文零依赖解析与 KaTeX 渲染**、**万物转 MD 批量工作台** 以及 **多模态音视频 Whisper 转写** 的全流程真实操作录屏视频与高清 Retina 截图资产。

---

## 目录结构

```
showcase/v238_capabilities/
├── README.md                               # 资产库说明与复现指南 (本文档)
├── samples/                                # 真实测试文件样本
│   ├── sample_paper.tex                    # 真实学术 QED 论文 LaTeX 源码 (含 align/tabular/proof/cite)
│   ├── sample_doc.docx                     # 真实复杂格式 Word 技术规范 (含三线表/多级大纲)
│   ├── dataset.csv                         # 科学测量实验对比指标数据表
│   ├── speech_demo.wav                     # 真实 3 秒 44.1kHz 16-bit PCM 语音样本
│   └── knowledge_base/                     # 真实双链知识库
│       ├── index.md                        # 知识库主页（含四向双链与死链示例）
│       ├── Quantum Physics.md              # 量子力学导论文档
│       ├── Electrodynamics.md              # 电动力学规范场论文档
│       └── Mathematical Methods.md         # 数学物理方法文档
├── snapshots/                              # 13 张高分辨率 Retina 截图 (2880×1800 物理像素)
│   ├── 01-plugin-center-overview.png       # 插件中心卡片式视图与已安装/可用状态
│   ├── 02-plugin-center-interactive.png    # 插件快速开关切换交互
│   ├── 03-ocr-table-formula-result.png     # 离线 OCR 表格还原与多行公式识别
│   ├── 04-wikilink-reader.png              # 双链笔记渲染与反向链接高亮
│   ├── 05-backlinks-drawer.png             # 反向链接与出链抽屉面板
│   ├── 06-knowledge-graph-modal.png        # 2D 力导向全网知识图谱弹窗 (全景)
│   ├── 07-graph-interactive-node.png       # 知识图谱节点拖拽、力学响应与连线高亮
│   ├── 08-latex-dark-math.png              # 暗色主题学术论文 LaTeX 编译渲染 (多行公式)
│   ├── 09-latex-table-and-theorems.png     # 学术论文三线表、定理证明块与参考文献
│   ├── 10-latex-light-theme.png            # 经典白昼/浅色模式学术论文优雅排版
│   ├── 11-batch-convert-complete.png       # 万物转 MD 批量工作台完成清单
│   ├── 12-av-transcribe-result.png         # 音视频多模态 Whisper 逐秒分段转写成果
│   └── 13-transcribe-fallback-guide.png    # 缺少外部依赖时的优雅渐进式降级指引
└── videos/                                 # 5 部 60fps 广播级 H.264 MP4 演示视频
    ├── 01-plugin-center-and-ocr.mp4        # 场景一：插件中心与复杂 OCR/表格识别
    ├── 02-knowledge-graph-and-bidirectional-links.mp4 # 场景二：双向链接与力导向关系网络图谱
    ├── 03-academic-latex-to-markdown.mp4   # 场景三：学术 LaTeX 纯 Python 解析与 KaTeX 渲染
    ├── 04-batch-convert-and-av-transcribe.mp4 # 场景四：万物转 MD 批量转换与音视频转写
    └── 05-full-suite-walkthrough.mp4       # 场景五：v2.3.8 全能特性综合贯通漫游
```

---

## 视频规格与转码参数

所有视频均通过自动化无头引擎在 1440×900 逻辑视口 (Retina 2x 设备像素比，等效 2880×1800 渲染精度) 下进行真实浏览器渲染录制，并使用 FFmpeg 工业级参数进行高质量无损降噪转码：

- **视频格式**: MPEG-4 (.mp4)
- **视频编码器**: H.264 (AVC High Profile, `libx264`)
- **像素格式**: `yuv420p` (保证全平台浏览器、移动端与剪辑软件完美兼容)
- **质量因子**: `CRF 18` (近视觉无损母带级画质)
- **编码预设**: `-preset slow` (最高码率利用率与平滑帧运动估计)
- **帧率**: 60 fps (丝滑平移、缩放与力导向粒子模拟)

---

## 演示场景详情

### 场景 1: 插件中心与复杂 OCR / 表格识别 (`01-plugin-center-and-ocr.mp4`)
1. 打开应用并呼出「更多菜单」→ 点击「插件中心」。
2. 展示插件中心对已缓存模型（如 RapidOCR、RapidTable、PyLaTeXEnc）的模型就绪角标及体积说明。
3. 演示插件一键启用/禁用切换与非阻塞模态框交互。
4. 渲染包含复杂三线表格与二阶偏微分薛定谔方程的学术扫描版提取文本，展现多栏 XY-Cut 阅读序重排效果。

### 场景 2: 双向链接与力导向知识图谱 (`02-knowledge-graph-and-bidirectional-links.mp4`)
1. 加载包含双向 WikiLink (`[[Target]]`, `[[Target|Alias]]`, `[[Target#Heading]]`) 的知识库文档。
2. 打开反向链接（Backlinks）抽屉，查看入链、出链及悬空死链（Deadlink）智能警示。
3. 启动 2D Canvas 极速力导向知识图谱（零第三方三维库，纯原生 Canvas 2D 60fps 实时模拟）。
4. 演示滚轮平滑缩放（Zoom In/Out）、画布抓手拖动（Pan）、节点弹性拖拽与连线引力唤醒。

### 场景 3: 学术 LaTeX 转换与数学公式渲染 (`03-academic-latex-to-markdown.mp4`)
1. 加载真实 QED 量子电动力学论文 LaTeX 源码转换而成的 Markdown。
2. 呈现文章元数据 Frontmatter（标题、作者、日期）、摘要引用块。
3. 展现多行对齐方程组（`aligned`）、规范协变导数、对易关系与物理常量表格。
4. 演示昼夜模式实时切换（Dark Mode / Light Mode），确保学术公式在深浅主题下均保持最佳对比度。

### 场景 4: 万物转 MD 批量工作台与音视频转写 (`04-batch-convert-and-av-transcribe.mp4`)
1. 启动「万物转 MD」工作台，批量投递 `.tex` 学术论文、`.csv` 数据表、`.wav` 研讨会录音。
2. 呈现批量转换处理进度与同名 `.md` 自动存盘确认。
3. 预览音频多模态 Whisper 逐秒分段转写成果（含标准 YAML frontmatter、时间戳高亮）。
4. 演示在未安装 Whisper/FFmpeg 时的智能友好降级排障指引。

### 场景 5: 全功能贯通综合演示大片 (`05-full-suite-walkthrough.mp4`)
- 全流程无缝漫游：学术阅读 → 公式推导 → 呼出知识图谱探索网络关联 → 调出万物转 MD 与插件中心 → 音视频多模态成果回放，展现从文档输入到知识网络沉淀的完整闭环。

---

## 重新录制与复现指南

若需重新录制或在 CI/CD 中回归测试，运行以下命令即可全自动一键生成：

```bash
# 确保系统已安装 Node.js 与 FFmpeg
node ui-tests/record_full_capabilities.js
```

录制完成后，视频与截图将自动同步至对话 Artifacts 与 `showcase/v238_capabilities/` 目录。
