# ReadMD Product Showcase

这条管线只发布真实运行截图，不生成或重绘产品 UI。

## 本地流程

```powershell
$env:SHOWCASE_RELEASE = "v2.3.7-beta.3"
$env:SHOWCASE_OUTPUT_DIR = "output/package/raw"
python showcase/scripts/build_package.py `
  --release "v2.3.7-beta.3" `
  --previous-release "v2.3.7-beta.2" `
  --notes release/release_notes.md `
  --output showcase/output/package `
  --skip-compose

npm run capture --prefix showcase
node showcase/scripts/compose_cards.js showcase/output/package
python showcase/scripts/build_package.py --output showcase/output/package --finalize
```

完整构建会先生成 12 种叙事框架 × 6 个标题公式共 72 个实验组合；每个组合都有稳定 `variant_id` 和 `copy_frame`，语义评分、置信门控后的钩子/标题/叙事帧表现、近期疲劳和跨发布原创性共同决定最终稿。任一维度不足 2 次发布或 1000 次曝光时只做探索，不给历史加分。每个变体都会计算整稿哈希、开头指纹、结尾指纹和三元组相似度：整文哈希和 ≥85% 相似度对照全历史，开头/结尾只在最近 8 次发布内冷却，冷却期外的框架可随新事实安全轮换。选择证据写入 `variants.json`。随后生成 `wechat/readmd-wechat.html`。该 HTML 只用行内样式，不包含脚本、外链、class、id 或图片；`wechat/wechat-qa.json` 也必须为 `{"ok":true}` 才允许进入发布队列。

标题反馈使用同一套置信门槛：同一公式不足 2 次发布或 1000 次曝光时，不会成为推荐公式，也不会给下一轮首选标题加分。
数字锚定标题中的“N 张”读取最终轮播计划；QA 会再把它与实际合成卡数量比对，避免用 claim 数编造图片规模。

标题、痛点开场、机制解释和提问都来自 `scripts/copy_profiles.py` 的机制档案；Release Intelligence 选出的 `primary_shot` 决定这轮叙事。完整主界面固定作为第二张真实证据图，不会冒充兜底主功能。
首图钩子也参与标题公式实验：A/B 选中的公式决定封面的心理触发器，审计会拒绝“标题是身份代入、封面却是结果承诺”这类断链。
核心叙事句也来自同一份机制档案；热门机制审计会拒绝「主功能是 A，核心句却讲 B」的立场漂移。
封面也读取同一份机制档案：短标题只保留一个可扫读的结果钩子，说明句解释它解决的具体任务；合成器和热门机制审计都会拒绝泛化的「本地文档台」兜底。
功能卡说明不再使用 shot 库里的实现描述，而是强制读取 `card_plan` 中与 claim 对应的读者收益；文案长度和 UI 区域一起进入热门机制审计。
辅助工作流短句同样来自机制档案；主机制确定后，最多保留两条最相关的支撑能力，不会因为 shot 未进入旧映射而被静默丢掉。
总结卡同样读取机制档案：标题收束本轮结果，说明句解释为什么值得保存，三个短证据点只保留这条机制最关键的支撑；泛化的「本地 Markdown 工作台」总结会被审计拒绝。

构建还会把 `pattern-library.json` 里的 11 条热门机制变成机器检查，结果写入 `pattern-audit.json`；封面钩子、缩略图排版、痛点移除、第二张完整主界面、单一主功能、具体场景提问、UI 区域契约和 DOM 设计审计任一失败都会阻止发布。封面标题会在 Playwright 渲染后实测字号和占幅，保证它在小红书信息流小图里仍能建立层级。随后生成 `review-dashboard.html`，把 QA 门禁、语义评分、选中稿加前四名挑战者、框架库存、反馈账本和最终文案汇总成一个自包含审查面板；72 个候选不会全部堆到页面上。`build_package.py --finalize` 会先写入本轮预检 QA，再生成面板并最后聚合所有门禁；任一面板失败都会把 `qa.json` 置红。

语义 QA 现在包含 AI 指纹与共鸣审计：检查空泛形容词、AI 腔、句长节奏、固定连接词堆叠、“不是 X 而是 Y”高密度、升维套话过拟合、祝福式收尾、具体产物、读者痛点和行动问题；未通过会写入 `copy-review.json.style` 并阻止发布。

`qa.json`、`copy-review.json`、`variants.json`、`pattern-audit.json` 和 `dashboard-qa.json` 必须同时通过才允许进入小红书发布队列。新包还必须让 `metadata.copy_frame`、选择报告和选中排名三方一致；watcher 发布前会重复这项检查。`--draft` 可让 watcher 只填充小红书表单，不点击发布：

```powershell
python showcase/scripts/watch_and_publish.py --once --draft
```

CI 使用 `package_content.py` 打包，压缩包内保留 `images/`、`raw/`、微信适配层和全部审计报告的相对路径；同时携带当次 Release notes 和 diff 快照及 SHA-256 清单。缺失合成图、真实截图、复核报告或证据哈希不匹配时会直接失败。

真实全自动发布使用已登录 Edge 和 CDP proxy：

```powershell
python showcase/scripts/watch_and_publish.py
```

watcher 会把 CI 产出的 `content-package.zip` 解到 `showcase/publish-work/`，重写图片路径后调用 `xhs-publish`。入队前会复核结构 QA、语义评分、变体原创性、热门机制、评审面板和微信适配层。状态保存在 `showcase/publish-state.json`；同一 release 只会发布一次。watcher 会同时检查发布状态和反馈账本，因此即使状态文件丢失，也不会重复发布已记录的 release。失败最多自动重试两次。
真实发布前还会按标题做一次平台预检；平台已接受同名笔记时只补录反馈账本并跳过点击，预检不可用时也不会冒险重发。
发布器还会独立复核 `title.txt`、`body.txt`、`topics.txt` 和本地图片清单必须与 `metadata.json` 完全一致；任何输入漂移都会在点击前失败，避免平台内容和实验归因错位。
如果发布器在点击后超时或返回非零，watcher 会先按标题查询平台笔记；只要平台已接受该笔记，就记录为已发布并写入反馈账本，不会再次点击。
如果发布器返回零但状态查询失败，watcher 仍把这次视为已发布；查询失败只记录为待补证据，反馈账本照常落盘。
如果账本首次写入失败，watcher 下次处理同一包时只补录账本，不会重新发布；若账本已有该 release，则保留已导入指标，不会重放零值。

WeChat 文件只做人工复制/粘贴发布，watcher 不会自动操作公众号。

## 发布反馈资产

真实发布后 watcher 会自动在 `showcase/content/publication-ledger.jsonl` 建立零指标待补录记录。拿到平台数据后，把平台计数写进 JSON 文件，并用带来源和抓取时间的 `metrics` 命令回填：

```powershell
python showcase/scripts/content_memory.py metrics --release "v2.3.7-beta.3" --record feedback.json --source xiaohongshu-web --captured-at "2026-08-23T10:00:00+08:00"
python showcase/scripts/content_memory.py comments --release "v2.3.7-beta.3" --record comments.json --source xiaohongshu-web --captured-at "2026-08-23T10:00:00+08:00"
python showcase/scripts/content_memory.py summary
python showcase/scripts/performance_report.py --output-dir showcase/reports/performance
```

指标导入会保留发布时的公式、钩子和 `variant_id`，拒绝身份冲突和旧快照回滚；六个平台计数齐全会置为 `complete`，缺项保持 `pending`，且计数不会随新快照回退。评论导入只保存主题、意图、点赞权重和匿名内容哈希，不保存原文、作者昵称或账号 ID；多次分页快照会按哈希累积，避免漏掉先前证据或重复计数。绩效报告会跨发布聚合出有置信门槛的评论焦点，并把它写入下一版文案的读者场景。报告会分开列出 `complete` 与 `pending` 记录；`pending` 只作为待补录清单，不参与公式和钩子学习，低置信维度也不会给出推荐。

记录字段包括 release、标题、公式 ID、钩子类型、曝光、赞、藏、评、转发、关注和一句复盘。下一次构建会读取这份资产：优先选择验证过的公式，并对最近连续使用过的公式降权。
