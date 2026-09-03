# assets/vendor/diagrams — 离线图表引擎清单（v2.3.8 Batch 0）

本目录是 ReadMD 的离线图表渲染引擎库，版本与来源严格对齐
markdown-preview-enhanced（MPE）/ crossnote 上游（"照搬"原则）。
参照源：`third_party/mpe-ref/crossnote/`（depth-1 克隆，.gitignore 排除，仅供对照）。

**铁律**（来自 crossnote/agents.md）：
- mermaid 必须保持与 jsDelivr 官方 dist 字节一致，禁止自行 re-bundle。
- 所有引擎渲染产物（SVG/HTML）在插入文档前必须经过转义/清洗。

## 引擎清单

| 引擎 | 版本 | 文件 | 来源 | 校验 |
|---|---|---|---|---|
| mermaid | 11.17.2 | mermaid/mermaid.min.js (3,576,297 B) | 字节级复制自 crossnote/dependencies/mermaid/mermaid.min.js（其上游为 jsDelivr mermaid@11.17.2/dist/mermaid.min.js） | 与上游字节一致 |
| vega | 5.25.0 | vega/vega.min.js (511,115 B) | 字节级复制自 crossnote/dependencies/vega/vega.min.js | 与上游字节一致 |
| vega-lite | 5.16.1 | vega-lite/vega-lite.min.js (249,066 B) | 字节级复制自 crossnote/dependencies/vega-lite/vega-lite.min.js（含上游两处 hack：structuredClone→globalThis.structuredClone；require("vega")→require("../vega/vega.min.js")） | 与上游字节一致 |
| vega-embed | 6.23.0 | vega-embed/vega-embed.min.js (67,368 B) | 字节级复制自 crossnote/dependencies/vega-embed/vega-embed.min.js | 与上游字节一致 |
| wavedrom | 3.3.0 | wavedrom/wavedrom.min.js (54,590 B) + wavedrom/skins/default.js (43,492 B) + wavedrom/skins/narrow.js (42,834 B) | 字节级复制自 crossnote/dependencies/wavedrom/；加载顺序必须是 skins/default.js → skins/narrow.js → wavedrom.min.js | 与上游字节一致 |
| bit-field | 1.9.0 | bitfield/bitfield.min.js (18,038 B) + LICENSE | npm `bit-field@1.9.0` 官方包内 `build/bitfield.js`（UMD，暴露 window.bitfield）；crossnote 侧经 `bit-field/lib/render.js` + JSON5 + onml.stringify 调用，本目录的 UMD 已内置等价链路 | tarball 原文件未改动 |
| @viz-js/viz | 3.30.0（@3 解析值） | viz/viz-standalone.js (1,404,869 B) | https://cdn.jsdelivr.net/npm/@viz-js/viz@3/lib/viz-standalone.js | 官方 standalone，未改动 |
| chart.js | 4.5.1 | chart/chart.umd.js (208,518 B) | https://cdn.jsdelivr.net/npm/chart.js@4/dist/chart.umd.js | 内嵌版本字符串 4.5.1 已验证 |
| TikZjax | v1 | tikzjax/tikzjax.js (458,758 B) + tikzjax/fonts.css (10,238 B) + tikzjax/3f69afb974a1e83f66a36f7618f88a38c254034b.wasm (612,530 B) + tikzjax/b565ab0b474e8e557d954694b7379a57db669ac9.gz (9,785,967 B) | tikzjax.com/v1/（js、css）与 s3.us-east-2.amazonaws.com/tikzjax.com/（wasm、字体 gz，即 upstream 硬编码的默认资源地址） | wasm 魔数 \0asm 已验证；gz 可完整解压（163,840,000 B 字体数据） |

## 与批准计划的偏差

1. **mermaid 版本**：计划写 10.9.x，实际采用 11.17.2——MPE/crossnote 上游当前锁定的版本，
   "照搬 MPE"优先级更高。
2. **chart.js**：当前 MPE/crossnote 源码已无 chart.js 集成（grep 证实），但批准计划明确
   包含 7 引擎全套，故仍予以 vendor，供 ```` ```chart ```` 代码块沿用。
3. **TikZjax 离线适配**：上游 `tikzjax.js` 保持逐字节不改（其资源基址仍是
   `https://s3.us-east-2.amazonaws.com/tikzjax.com`）。ReadMD 的懒加载 dispatcher
   在脚本执行期间将这两个固定资源请求映射到本目录的同名文件，再恢复原生
   `fetch`，因此运行时不访问外网，也不需要修改上游代码。注意：`tex.wasm` 不是真实
   资产名，真实资产就是上表两个哈希文件名。
4. **plantuml**：不离线 vendor；沿用现有在线渲染（`src/readmd_modules/diagrams.py`），
   可选激活 `has_local_plantuml()` 本机 jar 探测（计划既定）。

## 懒加载约定（Batch B 实施时遵守）

- 全部引擎按需加载，不进首屏。
- 渲染分发器落点：`assets/js/reader/render.js`（diagramLangs + 正式 dispatcher）。
- 各引擎的调用约定必须与 crossnote `src/render-enhancers/fenced-diagrams.ts`
  及 `src/renderers/*.ts` 保持一致：
  - mermaid → `<div class="mermaid" attrs>escaped</div>` 客户端渲染
  - wavedrom → `<div attrs><script type="WaveDrom">code</script></div>`
  - bitfield → JSON5.parse → window.bitfield 渲染 → SVG；错误输出转义 `<pre>`
  - viz → `Viz.instance().then(i => i.renderString(code, {engine: attrs.engine || 'dot', format:'svg'}))`，出错后重置 instance
  - vega / vega-lite → 首字符 `{` 判 JSON 否则 YAML.parse；vega-lite 先 compile 再走 vega View(renderer:'none').toSVG()
  - tikz → tikzjax.js 客户端 `<script type="text/tikz">`；`window.tikzjaxRender` trick 见 fenced-diagrams.ts

## 外部与非捆绑引擎声明（D2、WSD、Ditaa、PlantUML）

1. **D2 / WSD / Ditaa**：
   - 当前版本未捆绑数十兆的独立 Go/Java 编译程序；
   - 遵循用户数据隐私与本地优先原则，系统绝不在未授权时将源码上传至商业第三方云端；
   - 遇到此类代码块时，系统自动执行**诚实优雅降级（Graceful Fallback）**，以高对比度、等宽代码卡片形式安全呈现源码与明确提示。
2. **Vega / Vega-Lite**：
   - 采用独立 Node.js 本地侧端进程离线解析渲染，既保证离线可用与高保真 SVG 产出，又严格遵循桌面 WebView CSP 零 `unsafe-eval` 安全策略。
3. **PlantUML**：
   - 优先探测本机 Java 及 `plantuml.jar`（或 `plantuml` CLI）；
   - 本地无 Java 环境时透明走官方 SVG 代理，前端图表卡片明确标注 `(Online Proxy)` 标识以保证网络透明度。
