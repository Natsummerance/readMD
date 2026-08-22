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

完整构建会先生成 12 种叙事框架 × 5 个标题公式共 60 个实验组合；每个组合都有稳定 `variant_id` 和 `copy_frame`，语义评分、置信门控后的钩子/标题/叙事帧表现、近期疲劳和跨发布原创性共同决定最终稿。任一维度不足 2 次发布或 1000 次曝光时只做探索，不给历史加分。每个变体都会计算整稿哈希、开头指纹、结尾指纹和三元组相似度：整文哈希和 ≥85% 相似度对照全历史，开头/结尾只在最近 8 次发布内冷却，冷却期外的框架可随新事实安全轮换。选择证据写入 `variants.json`。随后生成 `wechat/readmd-wechat.html`。该 HTML 只用行内样式，不包含脚本、外链、class、id 或图片；`wechat/wechat-qa.json` 也必须为 `{"ok":true}` 才允许进入发布队列。

构建还会把 `pattern-library.json` 里的 10 条热门机制变成机器检查，结果写入 `pattern-audit.json`；封面钩子、痛点移除、第二张完整主界面、单一主功能、具体场景提问、UI 区域契约和 DOM 设计审计任一失败都会阻止发布。随后生成 `review-dashboard.html`，把 QA 门禁、语义评分、选中稿加前四名挑战者、框架库存、反馈账本和最终文案汇总成一个自包含审查面板；60 个候选不会全部堆到页面上。`build_package.py --finalize` 会先写入本轮预检 QA，再生成面板并最后聚合所有门禁；任一面板失败都会把 `qa.json` 置红。

语义 QA 现在包含 AI 指纹与共鸣审计：检查空泛形容词、AI 腔、句长节奏、具体产物、读者痛点和行动问题；未通过会写入 `copy-review.json.style` 并阻止发布。

`qa.json`、`copy-review.json`、`variants.json`、`pattern-audit.json` 和 `dashboard-qa.json` 必须同时通过才允许进入小红书发布队列。新包还必须让 `metadata.copy_frame`、选择报告和选中排名三方一致；watcher 发布前会重复这项检查。`--draft` 可让 watcher 只填充小红书表单，不点击发布：

```powershell
python showcase/scripts/watch_and_publish.py --once --draft
```

真实全自动发布使用已登录 Edge 和 CDP proxy：

```powershell
python showcase/scripts/watch_and_publish.py
```

watcher 会把 CI 产出的 `content-package.zip` 解到 `showcase/publish-work/`，重写图片路径后调用 `xhs-publish`。状态保存在 `showcase/publish-state.json`；同一 release 只会发布一次，失败最多自动重试两次。

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
