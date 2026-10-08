# WSD、D2、ditaa 基础离线图表

[返回总清单](../readmd-ui-function-inventory-2026-10-02.md)。本页更新于 2026-10-03。

这些代码围栏原本已有前端识别、渲染和重试入口，本轮把三个固定“不可用”分支接成 Rust 离线 SVG 渲染。界面保留源码/渲染切换与错误反馈；不需要下载 Java、Python、D2 命令行或访问图表服务。它们提供下表所列基础语法，不能视为完整第三方引擎。

| 类型 | 可操作入口与实际内容 | Rust 支持 | 限制与反馈 |
| --- | --- | --- | --- |
| WSD | 文档中的 `wsd` 代码围栏；查看图形/源码、失败重试 | `title`、`participant`、引号名称加 `as` 别名、自动参与者、正反方向消息、虚线消息、自己发给自己的消息、单行注释 | 不支持分组、激活区、条件分支、多行注释等完整 WSD 语法；无法解析的指令返回 `diagram_render_failed`。基础注释统一显示在参与者上方区域。 |
| D2 | 文档中的 `d2` 代码围栏；图形/源码和重试 | 命名节点与标签、链式连线 `->` / `<-` / `<->` / `--`、连线文字、上下左右布局、rectangle/square/circle/oval/diamond；闭环与自环保持有限布局 | 不支持嵌套映射、容器、样式块、变量、主题与复杂布局；大括号/方括号等复杂语法明确报错。循环图可以渲染，不等于完整 D2 布局质量。 |
| ditaa | 文档中的 `ditaa` 代码围栏；图形/源码和重试 | 等宽 ASCII 网格，`+ - = \| : / \\` 转为 SVG 线段，其他字符作为转义文字保留 | 提供基础网格矢量化；不是上游 ditaa 的闭合形状、色彩标签和特殊图形完整实现。没有把二进制图资源或未经转义源码作为图片返回。 |

WSD 基础示例：

```wsd
title Document conversion
participant "Reader" as R
R->Kernel: Convert file
Kernel-->R: Markdown
note over R: Ready
```

D2 基础示例：

```d2
direction: right
source: Source document
output: Markdown
source -> output: Convert
output.shape: diamond
```

ditaa 基础示例：

```ditaa
+----------+       +----------+
| Document |------>| Markdown |
+----------+       +----------+
```

源码上限为 2 MB / 2,000 行，图节点不超过 100，消息/连线不超过 1,000；ASCII 图不超过 240 行与 240 列。标签转义为 XML 文字，生成内容没有脚本、网络引用或动态执行。多行标签最多显示 12 行。超限与不支持语法都返回失败，保留原围栏供编辑。

前端路径为 `assets/js/reader/render.js` → `/api/diagram/render`；Rust 路径为 `parity_diagram.rs` / `batch2.rs` → `native_diagrams.rs`。`/api/diagram/capabilities` 标记这三个引擎为 `available: true`、`renderer: rust`、`syntax: basic`、离线可用；机器接口仍保留类型和错误码。

语法参考：[WSD 官方示例](https://www.websequencediagrams.com/examples.html)、[D2 官方连线文档](https://d2lang.com/tour/connections)、[ditaa 项目文档](https://ditaa.sourceforge.net/)。本仓库支持范围由上表和 Rust 实现确定。

自动验证包括 Rust 的 SVG/转义/拒绝复杂语法测试，以及 `ui-tests/function-completion.spec.js` 中三个真实代码围栏的浏览器渲染测试。具体结果见 [实现与验收记录](../readmd-ui-completion-2026-10-02.md)。
