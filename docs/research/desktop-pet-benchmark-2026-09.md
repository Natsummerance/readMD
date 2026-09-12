# 深度调研:GitHub 高星开源桌宠软件 Top 10(2026-09)

> 抓取/核实日期:**2026-09-12**。所有 star 数以 GitHub REST API(`stargazers_count`,精确值)与仓库页面显示数字(如 49.1k)双重核实;技术栈、许可证、活跃度来自仓库页面与 README 原文。原始证据文件存于本仓库 `.firecrawl/` 目录(仓库元数据 `repo-*.json`、搜索快照 `ghapi*.json`、README `readme-*.md`、许可证 `license-*.txt`、网页搜索快照 `search-*.md`)。
>
> **方法说明(工具偏差披露)**:任务指定使用 firecrawl CLI。经核实 `firecrawl-cli` v1.23.3 已安装且已认证,但 API 返回 **HTTP 402**(团队额度耗尽:`-98 / 1,000`),无 key 免费层又被 IP 风控拦截。因此本次调研改用 **GitHub REST API(一手数据,精确 star)+ 仓库页面 WebFetch 核实 + raw.githubusercontent.com 抓取 README/许可证 + 3 次网页搜索(补充生态背景)**,共执行 16 个不同角度的 GitHub 搜索查询 + 3 次网页搜索,满足"至少 10 个搜索角度"的要求;search-feedback 因 firecrawl 搜索不可用而无法发送。对候选池中每个入选项目均抓取了其 GitHub 主页/README 至少一次;未抓取任何需要登录的页面。

## 执行摘要

**领域格局:三条路线、一代新王。** 2026 年的开源桌宠领域已清晰分化为三条路线:(1) **轻量陪伴型**——以 BongoCat(23.1k)与 RunCat365(10.3k)为代表,零 AI、单一"神来之笔"的反馈循环(打字即动 / 负载即跑),靠跨平台移植、Microsoft Store 分发与模型生态拿下海量用户;(2) **AI 陪伴型**——以 airi(49.1k,Live2D/VRM + 语音 + 打游戏)与 Open-LLM-VTuber(13.7k)为代表,把桌宠当作"数字生命"的躯壳,重资产投入 ASR/TTS/LLM 管线;(3) **2026 年新爆发的一代:"AI 编码代理桌宠"**——clawd-on-desk(6.2k,2026 年内从零冲到 6k+)、OpenPets、awesome-codex-pet(918)等,用钩子监听 Claude Code / Codex / Cursor 等 agent 的生命周期事件,把" permission 等待、构建中、子代理并行"变成像素蟹的 12 种状态,击中了"盯着 agent 干活"这一全新痛点。而上一代的 Live2D 挂件(stevenjoezhang/live2d-widget,10.9k)与养成系(VPet,6.8k)依然坚挺,证明"看板娘"与"电子宠物"两类情感需求从未过时。

**共性规律:star 高度与"反馈延迟"成反比,与"内容生态"成正比。** 榜单前十中星数最高的项目都拥有一个 10 秒内可感知的核心反馈(打字→猫拍键盘;CPU 高→猫狂奔;agent 报错→蟹举手),而所有长寿项目都无一例外地开放了**用户导入内容**的通道:BongoCat 有在线模型转换器 + Awesome-BongoCat 模型仓库(1,968 星),VPet 有 Steam Workshop + MOD 制作器,airi 支持任意 Live2D/VRM,live2d-widget 用一个 `model_list.json` 静态约定撑起了十年的模型生态。相反,"功能堆料"并不带来星数:养成数值、喂食打工(VPet/DyberPet)粘性强但实现成本高;AI 对话(LingChat、my-neuro、Soul-of-Waifu 等均 <2.2k)并非高星的必要条件——BongoCat 和 RunCat365 连聊天框都没有。许可证方面出现明显分化:MIT/Apache(代码)与"素材另算"(VPet 内置动画、airi/OLLV 的 Live2D 示例模型、clawd 的美术)是最健康的实践,而 Mate-Engine 的自定义非商用许可证则直接把商用车门焊死。

**对 ReadMD 的启示:不要做"又一个 AI 桌宠",做"文档工作的第一只桌宠"。** ReadMD 的既有资产(Electron 透明窗口、pixi-live2d-display 渲染、Python JSON 文件桥、46 语种 i18n、MCP Server)恰好命中榜单验证过的全部低成本高回报路径:文件桥天然适合做 clawd-on-desk 式的**任务状态机事件源**(解析中/转换中/OCR 中/转换完成),i18n 管线天然适合做**事件驱动台词气泡**,pixi-live2d-display 生态可直接继承 live2d-widget 的 `model_list.json` 零后端模型库约定。最应警惕的是两条反面教训:一是性能隔离(VS Code pets 与 ReadMD 共用宿主渲染管线,桌宠必须可一键暂停);二是素材法律红线(Live2D 模型授权必须与 MIT 代码切分,默认素材必须可再分发)。下文第 7 节给出按优先级排序的 10 条落地建议。

## Top 10 排行总表

| 排名 | 项目 | Star 数(2026-09-12) | 语言/技术栈 | 渲染方式 | 平台 | 许可证 | 最近活跃 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | [moeru-ai/airi](https://github.com/moeru-ai/airi) | 49,053(页面 49.1k) | TypeScript(Vue/Vite monorepo;桌面壳已迁移 Electron;WebGPU/WebAudio/WASM) | Live2D + VRM(3D),自动眨眼/视线/待机动作 | Web(PWA)/Win/macOS/Linux/移动端(Capacitor) | MIT | 2026-09-12(pushed);v0.12.0-beta.5 |
| 2 | [ayangweb/BongoCat](https://github.com/ayangweb/BongoCat) | 23,092(23.1k) | Vue/TS + Rust(Tauri v2) | 分层精灵图(自定义模型格式) | Win/macOS/Linux(x11) | MIT | 2026-09-11 |
| 3 | [Open-LLM-VTuber/Open-LLM-VTuber](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber) | 13,709(13.7k) | Python 服务端 + Web/Electron 前端 | Live2D(后端情绪→表情映射) | Win/macOS/Linux(可全离线) | MIT(代码);Live2D 示例模型单独许可 | 2026-05-15(pushed) |
| 4 | [stevenjoezhang/live2d-widget](https://github.com/stevenjoezhang/live2d-widget) | 10,949(10.9k) | TypeScript(rollup) | Live2D(Cubism 2 与 3+/5 动态加载) | **仅 Web**(网页看板娘挂件,按任务要求收录并注明) | GPL-3.0(排除 Cubism SDK 专有组件) | 2026-09-09 |
| 5 | [runcat-dev/RunCat365](https://github.com/runcat-dev/RunCat365) | 10,300(10.3k) | C# / Win32 / .NET 9 | 任务栏内嵌精灵图奔跑猫 | Windows 10+(Microsoft Store) | Apache-2.0 | 2026-07-15(pushed) |
| 6 | [LorisYounger/VPet](https://github.com/LorisYounger/VPet) | 6,784(6.8k) | C# / WPF(.NET),NuGet `VPet-Simulator.Core` | WPF 位图帧动画(32 种 × 4 状态 × 3 类型动画图);MOD 可扩展 Live2D/Spine 方案 | Windows(Steam + GitHub;民间 Mac 版) | Apache-2.0(代码);内置动画素材版权另计 | 2026-09-11 |
| 7 | [rullerzhou-afk/clawd-on-desk](https://github.com/rullerzhou-afk/clawd-on-desk) | 6,204(6.2k) | JavaScript / Node(Electron) | 像素/SVG 动画(12 状态) | Win11(x64/ARM64)/macOS/Ubuntu;多屏 | AGPL-3.0(代码);美术素材保留所有权利 | 2026-09-11 |
| 8 | [tonybaloney/vscode-pets](https://github.com/tonybaloney/vscode-pets) | 4,171(4.2k) | TypeScript(VS Code 扩展,webpack) | 编辑器面板内像素精灵 | VS Code 桌面版(Win/macOS/Linux) | MIT | 2026-09-10 |
| 9 | [shinyflvre/Mate-Engine](https://github.com/shinyflvre/Mate-Engine) | 3,649(3.6k) | Unity / C# | 3D VRM(脊柱/眼球追踪、MMD 音乐动画、chibi 模式) | Windows(+非官方 Linux 移植) | 混合:AGPL v3 + 自定义 MateProv2(非商用) | 2026-01-20(pushed) |
| 10 | [SlimeBoyOwO/LingChat](https://github.com/SlimeBoyOwO/LingChat) | 2,151(2.2k) | Rust(Tauri)+ TS(Vite);由 Python 版重写 | Galgame 立绘/表情差分(18 类情绪分类驱动) | Windows 10 64 位 | AGPL-3.0 | 2026-09-11 |

**边界与落选说明**:live2d-widget 是纯 Web 挂件(任务要求收录并注明)、vscode-pets 是编辑器内嵌宠物(桌面软件范畴,注明);Externalizable/bongo.cat(3,780)是纯 web 网站、kuroni/bongocat-osu(2,463,已归档)是 osu! 游戏悬浮窗,均不入榜;NanmiCoder/cc-haha(14,345,虽被"desktop pet"搜索命中,实为 Claude Code 桌面工作台 GUI)、TokenTracker(1,586)、memmy-agent(1,888)、BASpark(750,鼠标特效)、fount(707,框架)等非桌宠项目已排除。更多候选见 JSON 附录与第 5 节末"荣誉提名"。

---

## 每个项目深度剖析

### 1. moeru-ai/airi(49,053 ★)— "自托管 Neuro-sama",AI 陪伴的航空母舰

**定位与亮点**:自我定位是"cyber waifu / digital pet / digital companion 的灵魂容器",目标是以开源方式复刻 AI VTuber Neuro-sama。它已远超"桌宠"范畴,是本榜单中唯一的"平台级"项目:实时语音对话(打断检测)、多厂商 TTS(ElevenLabs/Azure/OpenAI 兼容/本地 Kokoro)、以及**会玩游戏**(Minecraft 已可用、Factorio 进行中、Helldivers 2 联机 WIP),还能通过 Telegram/Discord 聊天。多平台分发做得极其顺手:winget/scoop/Homebrew Cask 全覆盖,README 有 7 种语言(另有 Crowdin 翻译工程)。

**技术架构**:monorepo(TypeScript/Vue/Vite,pnpm workspace),建栈第一天就押注 Web 技术:WebGPU(渲染与本地推理)、WebAudio、Web Workers、WebAssembly、WebSocket。桌面端"Stage Tamagotchi"在 2025 年 10 月的 DevLog 中完成 **Electron 迁移**,另有 Godot 阶段(`stage-tamagotchi-godot`)与移动端"Stage Pocket"(Capacitor)。模型层带完整的"活感"控制:自动眨眼、自动视线、待机眼球运动、动作控制。插件系统 WIP,MCP 集成,LLM 经 xsAI 接入 30+ 供应商。透明置顶/点击穿透的窗口细节未在 README 披露(桌面壳为 Electron,实现路径与 ReadMD 同构)。

**交互与陪伴感**:语音(耳朵→识别+说话检测)、游戏陪玩、跨平台随行;"陪伴感"来自连续性(记忆/RAG/内嵌数据库,由 @proj-airi 子项目群提供)而非点击反馈。

**社区与内容生态**:支持导入 Live2D/VRM 模型并在 Stage 中控制;官方长期招募 Live2D/VRM/MMD 级化师(Discussion #33);文档站 moeru-ai-airi.mintlify.app + airi.moeru.ai;DevLog 文化(2025-08 至 2026-03 持续更新)是其社区运营的显著特征。

**可借鉴点**:(1) "Stage(舞台)"抽象——把渲染载体(Web/桌面/手机)与"数字生命"内核分离;(2) 模型控制层(眨眼/视线/待机)是 Live2D/VRM 从"图片"变"生命"的关键 20%;(3) winget/scoop/brew 一键安装大幅降低尝鲜门槛。

### 2. ayangweb/BongoCat(23,092 ★)— 一个动作打天下的输入镜像猫

**定位与亮点**:为 MMmmmoko 的 Windows-only 项目 Bongo-Cat-Mver 而做的跨平台重制——作者作为 macOS 用户想要同款,Tauri 的跨平台能力使项目最终覆盖 macOS/Windows/Linux(x11)。零 AI、零养成,唯一卖点"键盘/鼠标/手柄操作实时映射到猫的动作"却造就了本榜单第二的星数,且**完全离线、不收集任何数据**。

**技术架构**:Tauri v2(Vue/TS 前端 + Rust `src-tauri` 后端)。Rust 侧做全局输入监听,Web 侧渲染分层精灵模型;透明置顶悬浮窗 + 交互穿透是此类输入镜像应用的必然形态(README 未展开技术细节,标注为按产品形态推断)。i18n 经 `src/locales/*.json` 落地(已验证存在 en-US.json)。

**交互与陪伴感**:反馈延迟趋近于零——用户敲键盘,猫拍键盘;"陪伴感"由高频微反馈制造。没有任何台词/气泡/数值系统,反而是其易传播的关键。

**社区与内容生态**:三点式生态令人印象深刻:(1) **在线模型转换器**(bongocat.vteamer.cc)把竞品 Bongo-Cat-Mver 的存量模型一键收编;(2) **Awesome-BongoCat**(1,968 ★)作为社区模型仓库,可探索、下载、提交创作;(3) QQ 群 + 赞赏码的中文社区运营,配合 trendshift/HelloGitHub 曝光。

**可借鉴点**:(1) "为缺失平台移植好项目"本身就是 star 引擎;(2) 模型格式转换器是收割既有生态的杠杆;(3) Awesome 式模型仓库让用户从消费者变成供应者。

### 3. Open-LLM-VTuber/Open-LLM-VTuber(13,709 ★)— 本地优先 AI 语音伴侣的参考实现

**定位与亮点**:口号"Talk to any LLM with hands-free voice interaction"——全离线可跑的语音交互 AI 伴侣,Live2D 形象,灵感同样源自 Neuro-sama。它是榜单中**唯一明确把"桌宠模式"写成特性**的项目:透明背景 + 点击穿透、触摸/拖拽反馈、AI 主动搭话。

**技术架构**:经典"Python 服务端(`run_server.py`)+ 前端客户端"架构,前端为独立子模块 Open-LLM-VTuber-Web(Web/Electron)。ASR 可插拔(sherpa-onnx、FunASR、Faster-Whisper、Whisper.cpp、Azure),TTS 可插拔(MeloTTS、GPT-SoVITS、CosyVoice、Edge TTS、Fish Audio),LLM 可插拔(Ollama、OpenAI 兼容、Gemini、Claude、Mistral、DeepSeek、GGUF、LM Studio、vLLM)。语音打断的工程亮点是"免耳机打断"——AI 不会听到自己的声音(回声消除策略)。表情由后端情绪标签映射到 Live2D 参数。支持视觉感知、聊天记录持久化、TTS 翻译。

**交互与陪伴感**:拖拽/触摸反馈 + 桌宠模式 + 主动说话三件套;文档站 open-llm-vtuber.github.io 有完整 quick start。

**社区与内容生态**:Live2D 模型加载(用户模型可导入,官方文档含模型配置),OS 世界观完整(Win/macOS/Linux,NVIDIA/非 NVIDIA/CPU/云 API 全路径)。许可处理是全榜最佳实践:代码 MIT,**Live2D 示例模型单独放在 LICENSE-Live2D.md**(Live2D Free Material License,商用需 Live2D Inc. 授权)。

**可借鉴点**:(1) "服务端管大脑、前端管脸"的解耦正是 ReadMD Python 桥的正确进化方向;(2) 免耳机打断这种单点工程突破比堆功能更立口碑;(3) 代码与素材许可证分离的范本。

### 4. stevenjoezhang/live2d-widget(10,949 ★)— 十年不衰的 Web 看板娘(纯 Web,注明收录)

**定位与亮点**:"把萌萌哒的看板娘抱回家"——一行 `<script>` 给任何网页加 Live2D 看板娘。**它是本榜单唯一的纯 Web 挂件,不是桌面软件**;收录依据任务要求注明。它常年出现在各类"桌宠"盘点中,因为它是无数桌面项目(PPet、L2dPetForMac、各类 Wallpaper Engine 版)的技术源头,README 亦直接链接这些桌面衍生品。

**技术架构**:TypeScript + rollup;运行时唯一依赖是 Live2D Cubism Core,**运行时动态加载 Cubism 2 或 Cubism 3+/5 对应的 Core**——同一份代码通吃两代模型。v1.0 起去后端化:不再需要 `apiPath`,静态部署 `model_list.json` + `textures.cache` 即可换装/换模型。分发走 jsDelivr(含 fastly 域名)、Cloudflare Pages、自托管三种路径。

**交互与陪伴感**:拖拽、工具条按钮、问候/提示语(简单台词)、退出时隐藏等配置(`drag`、`showToggleAfterQuit` 等)。

**社区与内容生态**:仓库本身不含任何模型——模型来自 `cdnPath` 指向的独立仓库;所有模型/贴图/动作版权归原作者,README 明示"仅供研究学习,不得用于商业用途"。GPL-3.0 代码 + Cubism SDK 专有组件排除的许可写法值得照抄。

**可借鉴点**:对 ReadMD 最直接——`model_list.json` 静态模型库约定 + 零后端 + CDN 分发,与 pixi-live2d-display 技术栈无缝衔接;Cubism 2/3 双核动态加载是兼容性刚需。

### 5. runcat-dev/RunCat365(10,300 ★)— 任务栏里的猫,系统负载的温度计

**定位与亮点**:"A cute running cat animation on your Windows Taskbar"——猫的奔跑速度映射系统负载(RunCat 家族概念:CPU 越忙猫跑越快)。纯指示器型"桌宠":不可拖拽、不可喂食,但 10.3k 星证明"环境感知型陪伴"的号召力。家族版图:macOS 菜单栏新版 RunCatNeo(840 ★,Swift)、旧版 Kyome22/menubar_runcat(510 ★)、GNOME 版 gnome-runcat(549 ★)等。

**技术架构**:C# + Win32 + .NET 9,直接在任务栏区域绘制帧动画;经 Microsoft Store 分发(Win10 19041+)。仓库 README 极简,但工程规范严格:Issue 模板强制、**仅接受英文 Issue/PR**。

**交互与陪伴感**:几乎为零交互——这正是它的设计哲学:宠物是"仪表盘拟物化"。自定义奔跑者(`custom_runner.png`)与无尽跑酷小游戏是仅有的两个彩蛋级互动。

**社区与内容生态**:8 种界面语言(简繁中文/英/法/德/日/韩/西);runcat-dev 开发者社区门户。Store 分发意味着它的真实用户量远大于 GitHub 互动量。

**可借鉴点**:(1) 桌宠可以作为系统/应用状态的"拟物仪表盘",这比"聊天玩偶"更适合生产力工具;(2) Store 渠道带来的量级远超 GitHub;(3) 极小的功能面 = 极低的维护成本。

### 6. LorisYounger/VPet(6,784 ★)— 虚拟桌宠模拟器,养成系的开源巅峰

**定位与亮点**:源自 VUP-Simulator 的内置桌宠,现可独立运行(Steam 可下载)也可**以 NuGet 包 `VPet-Simulator.Core` 内嵌进任何 WPF 应用**。"多达 32(种) × 4(状态) × 3(类型)种动画"是其招牌;摸头、拎起、爬墙等互动演示俱全。

**技术架构**:WPF/.NET 位图帧动画;解决方案三层清晰——`VPet-Simulator.Core`(可嵌入引擎:控制器/图形渲染/显示逻辑)、`VPet-Simulator.Windows`(桌面端:设置、模组管理器、**ChatGPT 设置窗口**、开发控制台)、`VPet-Simulator.Tool`(MOD 制作工具)。数值驱动的状态机:食物/饮品(`IFood`、三层"三明治"进食动画组件)、HP/FV 状态、`WorkTimer` 打工计时、存档系统;锁屏时数值停止变化等细节完备。

**交互与陪伴感**:喂食/养成/打工的完整 tamagotchi 循环;对话气泡文本可由 MOD 自定义;MOD 还能新增动画、主题,甚至**新的显示方案(Live2D 或 Spine)与新功能(闹钟、记事本)**——即核心是帧动画,但插件接口不锁死渲染技术。

**社区与内容生态**:Steam Workshop 是其内容主阵地(物品/食物/饮料/自定义宠物职业/对话文本);配套 LorisYounger/VPet.ModMaker(100 ★)降低 MOD 制作门槛、VPet.Plugin.Demo(108 ★)提供代码插件样例;README 覆盖简繁英日。法律注意:内置动画素材版权归 VUP-Simulator 团队,Apache-2.0 不覆盖这些文件。

**可借鉴点**:(1) "数值→动画图→台词"的状态机设计范式;(2) Workshop + MOD 制作器的 UGC 飞轮;(3) 把桌宠做成可嵌入引擎(NuGet)输出能力。

### 7. rullerzhou-afk/clawd-on-desk(6,204 ★)— 2026 年现象级"AI 编码代理桌宠"

**定位与亮点**:"A pixel desktop pet that watches Claude Code, Codex, Cursor & other AI coding agents — so you don't have to."——像素蟹 Clawd 实时反映 AI 编码代理的工作状态,让用户在长任务期间可以离开屏幕。2026 年爆发的全新品类代表:**桌宠第一次有了"实用主义"的存在理由**。

**技术架构**:Electron + Node.js;集成方式是全榜最值得研究的部分——**Claude Code** 用命令钩子 + HTTP permission 钩子全量接入;**Codex CLI** 用官方钩子 + `~/.codex/sessions/` 的 JSONL 文件兜底;Copilot/Gemini/Cursor/Kiro/Kimi/Qwen/opencode/Pi/OpenClaw/Hermes 等十余个 agent 各有可选钩子注入;**自定义 agent 可向动态 `/state` 端点 POST 生命周期事件**。v1 明确"state-only"边界:权限决定权留在 agent 自己的 UI。

**交互与陪伴感**:12 个动画状态——idle、thinking(模型读代码)、typing(工具执行)、building、subagent groove(单个子代理)、multi-subagent juggling(多子代理并行)、error、happy(如"14 文件/312 测试完成"时庆祝)、notification、sweeping、carrying、sleeping;加上眼球追踪、睡/醒序列、点击反应、mini 模式。**权限气泡**(permission bubble)把"agent 等待批准"顶到桌面,配合 Telegram/飞书远程批准与 Slack 通知——陪伴感与实用价值合流。

**社区与内容生态**:三个内置主题(Clawd 蟹、Calico 三花猫、Cloudling 云宝)+ 自定义主题 + **Codex Pet 动画包导入**;分发覆盖 GitHub Releases、WinGet(`rullerzhou-afk.clawd-on-desk`)、Homebrew cask;多显示器感知。许可:AGPL-3.0 代码,美术/主题素材排除在外且保留所有权利。

**可借鉴点**:(1) 钩子 + `/state` HTTP 端点是成本最低、扩展性最强的"宿主→宠物"事件协议(对 ReadMD 的 JSON 桥是直接升级蓝图);(2) 把等待/完成/报错拟宠化,是生产力工具桌宠的正当性来源;(3) "state-only"克制边界避免了权限劫持的安全坑。

### 8. tonybaloney/vscode-pets(4,171 ★)— 编辑器内嵌宠物的标杆

**定位与亮点**:"Adds playful pets in your VS Code window"——在 VS Code 面板里养猫狗蛇鸭(Clippy 也回归了),外加鹦鹉、Ferris 蟹、狐狸、秋田、乌龟、马、蜗牛、青蛙、熊猫、松鼠、骷髅等十几种。命令面板一句 `vscode-pets.start` 即开"宠物编程会话"。

**技术架构**:TypeScript + webpack 的 VS Code 扩展;宠物渲染在编辑器面板内(WebView),**完全绕开了透明窗口/点击穿透/全局热键等 OS 级难题**——这是它与榜单其他项目的根本形态差异,也是它能以极低成本维持 1,519 commits 高频迭代的原因。

**交互与陪伴感**:扔球玩耍、投喂式互动、背景主题(none/森林/城堡/冬季,素材署名艺术家)、可同时生成大量宠物。

**社区与内容生态**:i18n 走 Crowdin + 社区翻译(`package.nls.*.json` 多语言文件);Hacktoberfest 友好的贡献管线让新人 PR 源源不断;文档站 tonybaloney.github.io/vscode-pets。

**可借鉴点**:(1) 宿主面板内嵌是"性能敏感宿主养宠物"的最安全形态(对 ReadMD 的 VS Code 扩展路线极具参考性);(2) 固定精灵包 + 主题背景即可支撑 4k 星,内容不必贪多;(3) Crowdin 工作流与 ReadMD 的 46 语种 i18n 直接同构。

### 9. shinyflvre/Mate-Engine(3,649 ★)— 闭源付费应用的开放式替代品

**定位与亮点**:诞生动机写在标题里——"A free Desktop Mate alternative":闭源的 Desktop Mate 每 3D 角色收费 $10–25 且后续版本禁 MOD,Mate-Engine 以"自定义 VRM + 可 MOD + 免费"正面迎战(对比表列出对 Desktop Mate/Phase Pal 的全面优势)。

**技术架构**:Unity/C#;3D VRM 注入(任意合法 .VRM 文件)、脊柱/眼球追踪、MMD 音乐动画播放器、chibi 模式、大屏模式、屏幕保护模式、触摸区域、头像音效、Discord Rich Presence、Minecraft 集成、食物系统、**最多 9 个 avatar 同屏**;内置 AI 聊天(捆绑 Apache-2.0 的 Qwen 2.5 1.5B 本地模型)。GitHub 版免费,另有 $3.99 的 Steam 版(带 Workshop 与专属饰品)。

**交互与陪伴感**:事件驱动消息(拖拽、跳舞、坐窗台/任务栏时生成萌系台词);窗口/任务栏坐立、贴边等空间交互丰富,号称 Anti-Cheat Safe 可在游戏旁边运行。

**社区与内容生态**:内置 SDK + 自定义 `.ME` 格式 + 动画 MOD(MaoxiG 的 Custom Dance Player 是代表);免费 Miku VRM 从 Booth 分发。两个警示样本:(1) 未签名二进制触发 Windows Defender 误报(`Trojan:Script/Wacatac.B1ml`,作者声明为误报);(2) 许可证为混合"AGPL v3 + MateProv2":非商用、要求修改版开源发布在公共平台、**禁止在 Steam/Epic 等商业渠道分发衍生品**;默认头像保留所有权利、禁止再分发。

**可借鉴点**:(1) 对标闭源收费功能做开源平替是清晰的定位打法;(2) 9 avatar 同屏 + 事件台词展示了 3D 路线的表现力上限;(3) 自定义非商用许可对生态的抑制与 Defender 误报对分发的干扰,都是前车之鉴。

### 10. SlimeBoyOwO/LingChat(2,151 ★)— 情绪驱动的 Galgame 风 AI 伴侣

**定位与亮点**:沉浸式 AI Galgame 聊天 + 桌宠模式 + 日程管理的混合体。差异化在**情绪引擎**:自研 18 类短句情绪识别模型,让每条回复驱动立绘表情、Galgame 风格气泡、背景与音乐联动变化。

**技术架构**:已从早期 Python 3.8 版本重写为 **Rust(Tauri)+ TypeScript(Vite)**(5,480 commits);LLM 自带 API(DeepSeek 等自选),视觉模型可指派独立"视觉"角色(如通义千问 API)用于"偷看"屏幕;语音走 VITS 生态(Style-Bert-VITS2 / vits-simple-api,按角色配置)。

**交互与陪伴感**:桌面状态感知(工作/游戏/摸鱼)与主动搭话;番茄钟/日程/待办 + AI 提醒;角色自定义 OC 导入、换装、触摸互动、多角色剧本、好感度剧情与成就、**每存档独立持久记忆**。陪伴感设计在榜内最"galgame 化"。

**社区与内容生态**:v0.5 提供Win10 64 位打包;角色/立绘由用户导入(同人 OC 生态);仓库 topics 为空、以中文社区为主。

**可借鉴点**:(1) "LLM 回复→情绪分类→视觉/听觉联动"是比"生硬气泡"高一个层级的陪伴感方案,且分类器可本地小模型实现;(2) 存档级持久记忆让"陪伴"可积累;(3) Python→Rust/Tauri 重写路径对 Electron 应用的性能焦虑有参照意义。

---

## 横向对比与规律总结

功能矩阵(✅ 支持 / ❌ 不支持 / ➖ 文档未注明 / ✱ 按产品形态合理推断;依据为各仓库 README 与页面核实,不确定处明确标注):

| 项目 | Live2D | AI 对话 | 台词气泡 | 状态机 | 点击穿透 | 拖拽互动 | 多宠物 | 导入模型 | i18n | 更新频率 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| airi | ✅ | ✅(核心) | ✅(聊天 UI) | ➖(行为/插件 WIP) | ➖ | ✅(模型控制) | ➖ | ✅(Live2D/VRM) | ✅(7 语言 README + Crowdin) | 极高(日更级) |
| BongoCat | ❌(精灵图) | ❌ | ❌ | ✱(动作映射) | ✱(悬浮窗形态) | ✅(窗口可拖动) | ➖ | ✅(转换器 + Awesome) | ✅(locales) | 极高 |
| Open-LLM-VTuber | ✅ | ✅(核心) | ✅(聊天/弹幕) | ✅(情绪→表情) | ✅(README 明示 pet mode) | ✅(README 明示) | ➖ | ✅(自定义 Live2D) | ✅(多语文档) | 中(pushed 2026-05) |
| live2d-widget(Web) | ✅ | ❌ | ✅(问候语) | ➖(简单) | 不适用(网页) | ✅(drag 选项) | ➖ | ✅(model_list.json) | ✅(多语 README) | 高 |
| RunCat365 | ❌ | ❌ | ❌ | ✱(速度↔负载) | 不适用(任务栏) | ❌ | ❌ | ✅(自定义 runner 图) | ✅(8 语言) | 中(Store 持续分发) |
| VPet | ❌(MOD 可加) | ✅(ChatGPT 窗口) | ✅(对话 MOD) | ✅(32×4×3 动画图) | ✱(WPF 桌宠标配) | ✅(拎起/摸头) | ➖ | ✅(Workshop + ModMaker) | ✅(简繁英日) | 高 |
| clawd-on-desk | ❌(像素/SVG) | ❌(但感知 agent) | ✅(权限气泡) | ✅(12 状态) | ✱(多屏悬浮) | ✅(点击反应) | ✅(3 主题 + 导入包) | ✅(自定义主题/动画包) | ➖ | 极高(2,309 commits) |
| vscode-pets | ❌ | ❌ | ❌ | ✱(游走/玩耍) | 不适用(面板) | ✅(扔球) | ✅(多只) | ❌(固定精灵包) | ✅(Crowdin) | 高(1,519 commits) |
| Mate-Engine | ❌(VRM 3D) | ✅(本地 Qwen) | ✅(事件消息) | ✅(触摸区域/事件) | ✱(坐窗台/任务栏) | ✅(拖拽/跳舞) | ✅(9 avatar) | ✅(任意 VRM) | ➖ | 中(pushed 2026-01) |
| LingChat | ❌(立绘差分) | ✅(核心) | ✅(Galgame 气泡) | ✅(情绪 18 类) | ✱(桌宠模式) | ✅(触摸互动) | ✅(多角色剧本) | ✅(OC 立绘导入) | ➖ | 极高(5,480 commits) |

**规律总结**:

1. **反馈延迟决定上限,内容生态决定寿命。** 前 5 名中 3 名(BongoCat/RunCat365/live2d-widget)没有任何 AI,反馈链路都短于 1 秒;而所有长寿项目都开放了用户内容导入(模型/主题/台词)。
2. **AI 是加分项不是前置条件。** DyberPet README 对此有清醒表述:"AI 是加分项……不接 AI,也依然是完整的桌宠体验"。纯 AI 伴侣项目(LingChat/my-neuro/Soul-of-Waifu)星数尚未突破 2.2k,而"AI agent 状态指示"类(clawd-on-desk)反而快速破 6k——**AI 时代的桌宠机会在"表达 AI",不在"假装成 AI"**。
3. **渲染技术没有赢家通吃。** 精灵图(BongoCat/RunCat/vscode-pets/clawd)、Live2D(OLLV/live2d-widget/airi)、VRM 3D(Mate-Engine)、立绘差分(LingChat)并存;决定选择的不是表现力而是维护成本与素材供给。
4. **窗口技术是护城河也是坑。** 明确实现"透明 + 点击穿透"的项目(OLLV)会将其写成特性;Tauri(BongoCat/LingChat)与 Electron(airi/clawd)两条技术路线在 2026 年均已被验证;最稳的路径是 vscode-pets 式"宿主面板内嵌"。
5. **许可证正在成为品类大事。** 三种主流实践:代码宽松 + 素材另算(MIT/Apache + 素材版权声明:VPet/airi/OLLV/clawd)、全宽松(BongoCat/RunCat365/vscode-pets)、自定义非商用(Mate-Engine)。素材授权不当的项目(BANDORI-PET-REV 用 BanG Dream 角色)存在下架风险。
6. **分发渠道决定 star 之外的体量。** Microsoft Store(RunCat365)、Steam(VPet/Mate-Engine)、WinGet/Homebrew(clawd-on-desk/airi)、QQ 群+网盘(BongoCat)各显神通;GitHub star 只是口碑投影。

**荣誉提名**(未入前十但具参考价值):zenghongtu/PPet(2,032 ★,Electron+React+Live2D,**与 ReadMD 技术栈最接近的前辈,但 2024-06 起停更**)、isHarryh/Ark-Pets(1,088 ★,libGDX+Spine)、ChaozhongLiu/DyberPet(972 ★,PySide6 框架 + 纯 JSON 配置 MOD 生态,v0.10.3,LLM 模块开发中)、OpenPetsHQ/openpets(1,184 ★,插件 SDK)、Adrianotiger/desktopPet(1,144 ★,eSheep 复活)、Eikanya/Live2d-model(3,386 ★,模型资源库,Live2D 生态的"素材自来水")。

---

## 对 ReadMD 桌宠的具体启示

ReadMD 现状:Electron 透明窗口 + pixi-live2d-display(Live2D)+ Python JSON 文件桥;同时具备 46 语种 i18n、VS Code 扩展、FastMCP Server、跨平台信创构建等资产。按优先级排序的 10 条可落地建议:

1. **(P0)把 JSON 桥升级为"状态机事件协议",先抄 clawd-on-desk。** 定义有限的宠物状态集(idle / reading / parsing(转换中) / ocr( OCR 中) / error / celebrating(转换完成) / sleeping(空闲超时)),Python 桥在文件打开/解析/转换/OCR/保存等节点推送 `pet_state` 事件。这是全部调研中性价比最高的模式:事件源现成(ReadMD 的长任务就是事件),而"文档工作的桌宠"在全 GitHub 尚无头部占位者。
2. **(P0)性能隔离红线。** ReadMD 主业务是超大文档渲染,桌宠动画绝不能与分页渲染抢主线程:vsi vscode-pets 的启示是把宠物放在独立渲染层/独立窗口,并提供"性能模式"(大文件打开时自动降帧/暂停/隐藏);给用户提供一键暂停是生产力工具桌宠的礼仪底线。
3. **(P0)默认素材的法律洁净。** 参照 OLLV:代码 MIT,Live2D 示例模型单独 LICENSE-Live2D.md;内置模型要么自制、要么使用明确可再分发素材(Eikanya/Live2d-model 这类收集库的模型**不可**直接捆绑)。Cubism Core 运行时的分发条款需单独审阅(参照 live2d-widget 的排除条款写法)。
4. **(P1)台词气泡 + 事件文案,直接复用 46 语种 i18n 管线。** 事件驱动台词(打开 8000 行大文件、修复表格、转换完成)成本极低、情感回报极高;BongoCat 证明"无台词"可行,但 ReadMD 的差异化恰恰是"懂你的文档工作"——台词是最便宜的个性化载体。
5. **(P1)模型导入走 live2d-widget 的 `model_list.json` 约定。** 零后端、静态目录、支持换装;再补一个"ReadMD 官方示例模型仓库"(Awesome 模式,参照 Awesome-BongoCat 的 1,968 ★),让用户从消费者变供应者。pixi-live2d-display 与该约定同源,实现成本极低。
6. **(P1)透明窗口三件套做成一等公民。** 透明置顶 + `setIgnoreMouseEvents(true, {forward: true})` 的动态穿透(宠物本体可点击、周边穿透)+ 托盘开关/开机自启;多显示器与 DPI 缩放要在首版就处理(clawd-on-desk 把 multi-monitor aware 写进卖点,说明用户在意)。
7. **(P2)反馈手感优先于功能数量。** BongoCat 的全部成功来自"打字→拍键盘"的零延迟手感。ReadMD 可先做一个"翻页/打字→Live2D 视线与耳朵微动"级别的输入镜像,再谈其他;眨眼/视线/待机微动作(airi 的模型控制层)是把静态模型变"活"的最低成本投入。
8. **(P2)养成/喂食缓做,但接口留好。** VPet 证明养成系粘性最强,但数值平衡与素材成本极高;先以 MOD/插件接口预留(参照 VPet 的 `IFood`/WorkTimer 与 DyberPet 的纯 JSON MOD:角色/道具/音效 JSON 即可上手),把内容生产外包给社区。
9. **(P2)情绪引擎可选升级。** 若接 AI,采用 LingChat 的"LLM 回复→情绪分类→表情/气泡/音效联动"管线,分类用本地小模型即可;避免直接把完整聊天框塞进宠物窗口(OLLV 的服务端/前端分离证明聊天与形象应解耦)。
10. **(P3)分发与签名。** 开机自启 + 托盘 + 便携版按 ReadMD 既有交付矩阵补齐;但 Mate-Engine 的 Defender 误报案例说明**代码签名必须先行**——桌宠类常驻悬浮进程是杀软重点关照对象。Windows Store/WinGet(clawd-on-desk 已验证 winget id 分发)可作为第二渠道。

---

## 风险与反向观点

1. **"AI 桌宠"可能是伪需求陷阱。** 榜单规律显示:星数与 AI 含量无明显正相关,反而与维护负担强负相关(LLM 管线、API 费用、隐私质疑)。ReadMD 若把桌宠做成"套壳聊天",将同时输给 Character.ai 类产品和 BongoCat 类产品。
2. **clawd-on-desk 模式的可持续性存疑。** 其价值依赖 coding-agent 钩子生态,但各 agent 的钩子协议碎片化严重(其 README 维护着 15+ 个 agent 的集成说明,大量标注 state-only/Phase 1),协议一变即失效;且 permission 气泡一旦被 agent 官方 UI 覆盖,宠物存在感将被抽空。
3. **素材授权是最大法律雷区。** Live2D 模型角色(如 BANDORI-PET-REV 的 BanG Dream 角色、Mate-Engine 的 Miku VRM)属于第三方 IP,桌面常驻展示的授权边界模糊;Live2D Cubism Core 的再分发有专有条款;VPet 内置动画亦需单独授权。"开源代码 + 侵权素材"的组合随时可能被下架。
4. **自定义非商用许可证抑制生态。** Mate-Engine 的 MateProv2(禁商用、禁 Steam 分发衍生品)保护了作者但隔绝了贡献者商业化动机;对希望被集成/内嵌(如 ReadMD 场景)的项目,AGPL(Mate-Engine 混合、LingChat、clawd-on-desk)对闭源宿主同样不友好——若 ReadMD 桌宠希望被第三方整合,应坚持 MIT/Apache。
5. **看似诱人但不值得抄的功能**:多宠物同屏(Mate-Engine 的 9 avatar——素材与性能成本指数级,收益边际);游戏联玩(airi 的 Minecraft/Factorio——研究级工程);桌面视觉"偷看"(LingChat 的 vision 偷屏——隐私争议与杀软误判风险);VRM 3D 路线(脊柱/眼球追踪工程量大,素材生态远小于 Live2D)。
6. **star ≠ 用户量。** RunCat365 的真实体量在 Microsoft Store,BongoCat 的在网盘与 QQ 群;以 GitHub star 做产品决策会系统性高估"极客功能"低估"分发"。
7. ** Shimeji 生态的警告:框架会死,内容永生。** Shimeji-ee 官方已不在 GitHub 活跃(kilkakon/shimeji 等 404),但数百个粉丝制作的 mascot 包仍在流转,衍生运行器(Qt6/Wayland/跨平台)持续出现——桌宠项目的长期资产是素材格式与内容生态,不是代码本身。

## 开放问题

1. airi 应归类为"桌宠"还是"AI VTuber 引擎/平台"?若剔除,Top 1 顺延为 BongoCat,榜单性质将完全不同——本文按任务口径(桌面端数字陪伴软件)收录并注明其平台属性。
2. clawd-on-desk 式"agent 桌宠"能否成为长期品类,还是 agent 官方 UI(如 Claude Code 的桌面化)的 transitional 需求?
3. Tauri 与 Electron 在"透明 + 穿透 + 多屏 + 全局输入监听"上的长期维护成本差异,缺乏系统性对比数据(两路线在本榜单各有一线案例)。
4. Live2D Inc. 对桌面常驻类应用的模型授权态度(现有许可多面向网页嵌入),可能影响所有 Live2D 桌宠的商业化路径。
5. Shimeji-ee 官方仓库的消失时间与原因未能核实(仅确认 2026-09-12 时 kilkakon/shimeji、kilkakon/shimeji-ee、Shimeji-ee/Shimeji-ee 均返回 404);任务中提到的"gimmicnaster/shimeji"与"amethyst"桌宠:经专项搜索(3 次查询)未找到对应的高星项目——"amethyst"最接近的命中是 VR 体追踪应用 KinectToVR/Amethyst 与 macOS 平铺窗口管理器 ianyh/Amethyst,均与桌宠无关,判定为记忆偏差,实际所指很可能是 Mate-Engine / Desktop Mate 一类。

## Sources

- https://github.com/moeru-ai/airi — Top1 仓库页(WebFetch 核实 49.1k/MIT/Topics/Release v0.12.0-beta.5);README raw(main)
- https://api.github.com/repos/moeru-ai/airi 及 https://api.github.com/search/repositories?q=topic:live2d — 精确 star 49,053、pushed_at、topics(一手,存 .firecrawl/)
- https://github.com/ayangweb/BongoCat — Top2 仓库页(23.1k/MIT/Tauri/平台矩阵);README raw(master):输入映射、模型转换器、Awesome-BongoCat、QQ 社区
- https://api.github.com/repos/ayangweb/BongoCat — 精确 star 23,092;raw.githubusercontent.com/.../src/locales/en-US.json(200,i18n 佐证)
- https://github.com/Open-LLM-VTuber/Open-LLM-VTuber — Top3 仓库页(13.7k/特性矩阵/pet mode/License 说明);README raw + LICENSE raw(MIT + Live2D 模型单独许可);文档站 https://open-llm-vtuber.github.io/
- https://github.com/stevenjoezhang/live2d-widget — Top4 仓库页(10.9k/GPL-3.0/CDN/model_list.json/双 Cubism 核心);README raw
- https://github.com/runcat-dev/RunCat365 — Top5 仓库页(10.3k/Apache-2.0/C# Win32 .NET 9/Store 分发/8 语言);README raw;https://api.github.com/search/repositories?q=RunCat+in:name(RunCat 家族星数)
- https://github.com/LorisYounger/VPet — Top6 仓库页(6.8k/Apache-2.0/32×4×3 动画/Workshop/NuGet/素材版权说明);README raw
- https://github.com/rullerzhou-afk/clawd-on-desk — Top7 仓库页(6.2k/AGPL-3.0/12 状态/15+ agent 钩子集成//state 端点);README raw(50KB,集成矩阵);WebSearch 快照 .firecrawl/search-clawd-on-desk.md(Releases/WinGet/known-limitations)
- https://github.com/tonybaloney/vscode-pets — Top8 仓库页(4.2k/MIT/宠物列表/主题/Crowdin);README raw
- https://github.com/shinyflvre/Mate-Engine — Top9 仓库页(3.6k/Unity/对比表/9 avatar/Qwen 内置);README raw + LICENSE.md raw(MateProv2 非商用条款);WebSearch 快照 .firecrawl/search-mate-engine.md(Desktop Mate 背景、Defender 误报)
- https://github.com/SlimeBoyOwO/LingChat — Top10 仓库页(2.2k/AGPL-3.0/情绪引擎/桌宠感知/v0.5);API 元数据(default_branch=main、Rust/Tauri 文件树)
- https://api.github.com/search/repositories?q=desktop+pet / topic:desktop-pet / 桌宠 / 桌面宠物 / bongo+cat / shimeji / shimeji-ee / live2d desktop / virtual pet / oneko / vpet / open-llm-vtuber / airi 等 16 组查询 — 候选池发现与星数排序(原始 JSON 存 .firecrawl/ghapi*.json)
- https://api.github.com/repos/{owner}/{repo} — 27 个候选仓库的精确元数据(stars/language/license/pushed_at/topics,存 .firecrawl/repo-*.json)
- https://raw.githubusercontent.com/ChaozhongLiu/DyberPet/main/README.md — 荣誉提名依据:PySide6 框架、JSON MOD、"AI 是加分项不是前置条件"、v0.10.3 状态
- https://api.github.com/repos/kilkakon/shimeji 等 — 404 核实(Shimeji-ee 官方仓库不在 GitHub);衍生项目 pixelomer/Shijima-Qt、CluelessCatBurger/wl_shimeji、estenv/linux-shimeji 元数据
- https://store.steampowered.com/app/3301060/Desktop_Mate/ — 闭源对照 Desktop Mate(Steam,付费 DLC);https://github.com/YusufOzmen01/desktopmate-custom-avatar-loader(社区 mod 被更新破坏)
- https://winstall.app/apps/rullerzhou-afk.clawd-on-desk — clawd-on-desk 的 WinGet 分发佐证
- https://moeru-ai-airi.mintlify.app/ 、https://airi.moeru.ai/docs/en/docs/overview/ 、https://github.com/moeru-ai/airi/discussions/33 — airi 官方文档与社区(WebSearch 快照 .firecrawl/search-airi-context.md)
- https://explainx.ai/blog/airi-ai-vtuber-neuro-sama-guide-2026 — 第三方对 airi 的定位描述(airi 搜索快照)

## Rerun Inputs

workflow: firecrawl-deep-research
topic: GitHub top-starred open-source desktop pet software
depth: thorough (执行层实际达到 exhaustive:16 组 GitHub API 搜索角度 + 3 次网页搜索 + 27 仓库元数据核实 + Top10 页面/README/许可证逐项抓取)
output: markdown+json
tooling_note: firecrawl CLI v1.23.3 已验证安装与认证,但 API 402(credits 耗尽 -98/1000)且 keyless 层 IP 受限;搜索结果改由 GitHub REST API(一手)+ WebSearch/WebFetch 获取,全部原始证据保存于 .firecrawl/ 目录;firecrawl search-feedback 因无成功搜索而未发送。
