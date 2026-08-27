# ReadMD Product Showcase

这条管线只发布真实运行截图，不生成或重绘产品 UI。
每个镜头可声明独立视口和素材文档：阅读/编辑保留工作台纵屏，放映使用 16:9，欢迎页与弹层使用无空白的桌面比例。发布前会按截图元数据复核尺寸，并用像素审计拦截长空带、近空白和比例错配。Reveal.js、Markdown、高亮、备注、KaTeX 及字体已本地化到 `assets/vendor`，放映页在 CSP 内离线渲染。

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

海报视觉默认仍是 `evidence-paper`。要切换风格时，构建阶段写入 `story.poster_style`，合成阶段也可用参数覆盖。CI 使用 `--poster-style auto`：只有发布账本中出现达到置信阈值的最优风格时才切换；证据不足时按最少使用顺序轮换四种新风格，用真实发布结果积累可比样本：

```powershell
python showcase/scripts/build_package.py ... --poster-style photo-relic
node showcase/scripts/compose_cards.js showcase/output/package --style photo-relic
```

当前风格库为 `evidence-paper`、`minimal-zine`、`morandi-cinematic`、`photo-abstract` 和 `photo-relic`。新风格只改变包装语言和版式，不替换真实截图；所有包仍必须通过同一套对比度、最小字号、碰撞、溢出和 UI 面积门禁。临时比较不同风格时，可把成品输出到独立目录，避免改写发布包的 `metadata.json` 和根级 `composition.json`：

```powershell
node showcase/scripts/compose_cards.js showcase/output/package `
  --style minimal-zine `
  --output-dir showcase/output/style-previews/minimal-zine
```

正式包会把选中的 `poster_style` 写进 `story.json`、`metadata.json` 和 schema 3 的 `composition.json`，并把选择模式写入 `variants.json`；watcher 再把它作为不可变身份写入发布账本。因为 CI 看不到私有账本，watcher 发布前会用本机账本复算 `auto` 选择；结论不同时不只是拦截，而是用同一批未改动的真实 PNG 本地重合成目标风格，再重跑语义、像素、热门机制和面板门禁。绩效报告会按风格聚合曝光和加权互动。显式指定风格或省略参数时仍保持固定视觉，不会被动轮换。

## 海报人工确认与一键发布

批量包先转成 PDF 给操作者确认；确认前不允许发布：

确认 PDF 包含批次索引页、每个 ZIP 的分隔页、发布标题、海报页码和 SHA-256；海报本身不会被索引页覆盖。

```powershell
python showcase/scripts/build_poster_review.py `
  --batch showcase/output/release-run/downloads/batch-pending.json `
  --root showcase/output/release-run/downloads `
  --output showcase/output/release-run/downloads/poster-review-beta3-beta4.pdf
```

你确认 PDF 后，执行一条一键发布命令；脚本会生成批准文件并立即发布：

```powershell
python showcase/scripts/publish_approved_batch.py `
  --batch showcase/output/release-run/downloads/batch-pending.json `
  --approval-request showcase/output/release-run/downloads/poster-review-beta3-beta4.approval-request.json `
  --root showcase/output/release-run/downloads `
  --work-dir showcase/output/release-run/work `
  --state showcase/output/release-run/publish-approved-state.json `
  --ledger T:/Programming/Project/codex/creator/readmd/showcase/content/publication-ledger.jsonl
```

批准文件绑定 PDF、批次和每个 ZIP 的 SHA-256；任一文件改动、QA 变红或 release 已在账本中，一键脚本都会停止。已发布的 release 会自动跳过。

发布前可先用无副作用预检复核同一批文件；它不会创建批准文件，也不会调用发布器：

```powershell
showcase/scripts/publish_approved_latest.ps1 `
  --validate-only
```

确认 PDF 后，直接运行 Windows 一键入口即可发布最新待确认批次；脚本会自动定位最新 `approval-request`、生成批准文件、跳过已发布 release，并按顺序提交剩余包：

```powershell
showcase/publish-approved.cmd
```

如需指定私有账本或代理：

```powershell
showcase/publish-approved.cmd `
  -Ledger "T:/Programming/Project/codex/creator/readmd/showcase/content/publication-ledger.jsonl" `
  -PublisherProxy "http://127.0.0.1:3456"
```

完整构建会先生成 12 种叙事框架 × 8 个标题公式共 96 个实验组合；每个组合都有稳定 `variant_id` 和 `copy_frame`，语义评分、置信门控后的钩子/标题/叙事帧表现、近期疲劳和跨发布原创性共同决定最终稿。任一维度不足 2 次发布或 1000 次曝光时只做探索，不给历史加分。每个变体都会计算整稿哈希、开头指纹、结尾指纹和三元组相似度：整文哈希和 ≥85% 相似度对照全历史，开头/结尾只在最近 8 次发布内冷却，冷却期外的框架可随新事实安全轮换。标题哈希与三元组也会对照发布账本，防止同一机制连续换封面却重复旧标题。选择证据写入 `variants.json`。随后生成 `wechat/readmd-wechat.html`。该 HTML 只用行内样式，不包含脚本、外链、class、id 或图片；`wechat/wechat-qa.json` 也必须为 `{"ok":true}` 才允许进入发布队列。

标题张力评分使用公式结构信号加机制具体性，正文焦点评分读取同一份机制档案的 mechanism 词表；不再给“PPT”“上台”等单一场景词额外加分。这样科研图表、LaTeX、代码运行、资料整理和分享机制都能按同一把尺子参与爆帖实验。

热门机制审计里的痛点、读者场景和具体提问也改读当前机制档案；评论共鸣把焦点切到代码、图表或资料整理时，不会再被旧的学术/放映词表误判。

标题反馈使用同一套置信门槛：同一公式不足 2 次发布或 1000 次曝光时，不会成为推荐公式，也不会给下一轮首选标题加分。
每个标题候选都记录 `dbs-xhs-title` 的来源模板和本轮适配规则；QA 与发布账本会复核这两项溯源字段，防止公式编号变成自由发挥的借口。
数字锚定标题中的“N 张”读取最终轮播计划；QA 会再把它与实际合成卡数量比对，避免用 claim 数编造图片规模。

标题、痛点开场、机制解释和提问都来自 `scripts/copy_profiles.py` 的机制档案；Release Intelligence 选出的 `primary_shot` 决定这轮叙事。完整主界面固定作为第二张真实证据图，不会冒充兜底主功能。
不可拍图的底层修复会先翻译成读者能感知的结果；QA 会拦截 CodeMirror、AST、DOM 这类实现名回流到正文。
每个机制档案还提供一条“判断标准”，告诉读者什么情况下值得用这条工作流；QA 会确认这句可收藏规则没有在长度适配时被裁掉。
补齐长度的事实段只会进入场景和下载说明之前；具体互动提问永远保持最后一段，避免读者看到结尾时已经离开行动点。
微信适配层会把这条判断标准渲染成醒目收藏卡，并把三点摘要放在互动提问之前，保持两个渠道的行动节奏一致。
首图钩子也参与标题公式实验：A/B 选中的公式决定封面的心理触发器，审计会拒绝“标题是身份代入、封面却是结果承诺”这类断链。
核心叙事句也来自同一份机制档案；热门机制审计会拒绝「主功能是 A，核心句却讲 B」的立场漂移。
五个话题同样按主机制选择：保留 Markdown 和效率工具等大流量词，同时加入 PPT、LaTeX、科研绘图、PDF 等精准搜索词；语义 QA 会拒绝只有通用标签的机制错配。
每个机制维护两个合规话题组。没有足够发布证据时探索默认组；同一话题组达到置信门槛后会获得历史表现加分，最近连用会受到疲劳惩罚，未充分使用的组会获得轮换奖励。
每个包都会根据精确话题组合生成 `topic_set_id`；QA 拒绝篡改，发布账本把它与 `primary_shot` 和话题列表一起作为不可变归因身份。绩效报告会分别聚合话题组和单个搜索词的加权互动。
封面也读取同一份机制档案：短标题只保留一个可扫读的结果钩子，说明句解释它解决的具体任务；合成器和热门机制审计都会拒绝泛化的「本地文档台」兜底。
功能卡说明不再使用 shot 库里的实现描述，而是强制读取 `card_plan` 中与 claim 对应的读者收益；文案长度和 UI 区域一起进入热门机制审计。
辅助工作流短句同样来自机制档案；主机制确定后，最多保留两条最相关的支撑能力，不会因为 shot 未进入旧映射而被静默丢掉。
匿名评论会先归并为主题和请求/疑问/称赞/担忧意图；达到置信门槛后，下一稿自动生成保留、强化、压缩、删除四项编辑决策，并把焦点能力排进支撑句。该指令随发布账本不可变保存，预检面板会在点击前展示完整证据链。
watcher 发布前会用本地发布账本重算这条共鸣指令；包内指令与账本证据不一致时，即使其他 QA 全绿也不会点击。
选题使用的完整学习账本也会记录 SHA-256 指纹；finalize 会刷新绩效报告。watcher 收到本机更新的私有账本后，会用当前证据重算 96 个候选；只有胜出稿改变或旧稿不再合格时才阻止发布，避免 runner 与本机账本不同步造成误拦。
总结卡同样读取机制档案：标题收束本轮结果，说明句解释为什么值得保存，三个短证据点只保留这条机制最关键的支撑；泛化的「本地 Markdown 工作台」总结会被审计拒绝。

构建还会把 `pattern-library.json` 里的 12 条热门机制变成机器检查，结果写入 `pattern-audit.json`；封面钩子、缩略图排版、痛点移除、第二张完整主界面、单一主功能、可收藏判断标准、具体场景提问、UI 区域契约和设计审计任一失败都会阻止发布。封面标题会在 Playwright 渲染后实测字号和占幅：展示字号不得低于 96px，标题块高度必须达到画布的 6%；当前设计系统使用 102px，保证它在小红书信息流小图里仍能建立层级。随后生成 `review-dashboard.html`，把 QA 门禁、语义评分、选中稿加前四名挑战者、框架库存、反馈账本和最终文案汇总成一个自包含审查面板；96 个候选不会全部堆到页面上。`build_package.py --finalize` 会先写入本轮预检 QA，再生成面板并最后聚合所有门禁；任一面板失败都会把 `qa.json` 置红。

语义 QA 现在包含 AI 指纹与共鸣审计：检查空泛形容词、AI 腔、句长节奏、固定连接词堆叠、“不是 X 而是 Y”高密度、升维套话过拟合、祝福式收尾、具体产物、读者痛点和行动问题；未通过会写入 `copy-review.json.style` 并阻止发布。

`qa.json`、`copy-review.json`、`variants.json`、`pattern-audit.json` 和 `dashboard-qa.json` 必须同时通过才允许进入小红书发布队列。新包还必须让 `metadata.copy_frame`、选择报告和选中排名三方一致；watcher 发布前会重复这项检查。`--draft` 可让 watcher 只填充小红书表单，不点击发布：

```powershell
python showcase/scripts/watch_and_publish.py --once --draft
```

CI 使用 `package_content.py` 打包，压缩包内保留 `images/`、`raw/`、微信适配层和全部审计报告的相对路径；同时携带当次 Release notes 和 diff 快照及 SHA-256 清单。缺失合成图、真实截图、复核报告或证据哈希不匹配时会直接失败。
合成器会把每张 1080×1440 成品卡的 SHA-256 写入 `composition.json`；构建 QA 和 watcher 都会重算这张清单，防止真实截图在合成后被替换或篡改。
CI 还会生成 `content-package.zip.manifest.json` 传输清单，逐个记录包内文件的字节和哈希；watcher 在解包前核对压缩包与全部成员，任何损坏、缺失或多出的文件都会停止发布。

真实全自动发布使用已登录 Edge 和 CDP proxy：

```powershell
python showcase/scripts/watch_and_publish.py
```

多个历史修复包在进入发布队列前，还必须作为一批校验。批次校验会复核 ZIP/manifest 哈希、绿色报告、语义图与原始证据链、版本/标题唯一性和跨包正文相似度，并生成免脚本的人工审查页：

```powershell
python showcase/scripts/validate_repair_batch.py
```

审查页会由 Playwright 做 DOM 布局回归：桌面和移动端都必须无横向溢出、无隐藏文本裁切，并保持三包正文与证据链可读。
补录历史 Release 时，可用 `SHOWCASE_SHOT_OVERLAY` 只放宽某个版本尚不存在的 UI 断言（例如旧版放映窗口没有悬浮工具栏），镜头 ID、输出文件和证据契约保持不变。已确认 Edge CDP 就绪后，追加 `--reuse-edge` 可让同一批发布复用会话，避免每篇都重启浏览器。

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

曝光和互动优先用创作者中心手动导出的「笔记列表明细表」XLSX 回填；导入器只读本地文件，不重启浏览器、不提交表单、不保存原始工作簿：

```powershell
python showcase/scripts/import_feedback_workbook.py `
  --release "v2.3.7-beta.3" `
  --workbook "笔记列表明细表.xlsx" `
  --captured-at "2026-08-25T10:00:00+08:00"
```

评论可从本地公开页采集结果回填。适配器兼容 `{ "comments": [{ "content", "like_count", "sub_comments" }] }` 和旧的 `{ "comments": [{ "text", "likes" }] }`；它会展开子回复、校验身份归属，然后只把匿名哈希、主题和意图写入账本：

```powershell
python showcase/scripts/import_comment_capture.py `
  --release "v2.3.7-beta.3" `
  --capture "note_detail.json" `
  --captured-at "2026-08-25T10:00:00+08:00"
```

指标导入会保留发布时的公式、钩子和 `variant_id`，拒绝身份冲突和旧快照回滚；六个平台计数齐全会置为 `complete`，缺项保持 `pending`，且计数不会随新快照回退。评论导入只保存主题、意图、点赞权重和匿名内容哈希，不保存原文、作者昵称或账号 ID；多次分页快照会按哈希累积，避免漏掉先前证据或重复计数。绩效报告会跨发布聚合出有置信门槛的评论焦点，并把它写入下一版文案的读者场景；评论证据有自己的来源和时间戳，不必等曝光指标补齐才生效。报告会分开列出 `complete` 与 `pending` 记录；`pending` 不参与公式和钩子学习，低置信维度也不会给出推荐。

互动学习使用统一的六信号评分：点赞、收藏、评论、转发和关注分别按 1、2、3、4、6 加权；关注代表跨笔记的长期需求，权重最高。

记录字段包括 release、标题、公式 ID、钩子类型、曝光、赞、藏、评、转发、关注和一句复盘。下一次构建会读取这份资产：优先选择验证过的公式，并对最近连续使用过的公式降权。
