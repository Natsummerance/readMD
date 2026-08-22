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
python showcase/scripts/validate_package.py showcase/output/package
```

完整构建会先生成结果导向、身份代入、机制好奇三种完整稿件；语义评分、历史钩子表现和跨发布原创性共同决定最终稿。每个变体都会计算整稿哈希、开头指纹、结尾指纹和三元组相似度；命中历史正文、开头或结尾，或与历史稿相似度 ≥85% 的变体直接淘汰。选择证据写入 `variants.json`。随后生成 `wechat/readmd-wechat.html`。该 HTML 只用行内样式，不包含脚本、外链、class、id 或图片；`wechat/wechat-qa.json` 也必须为 `{"ok":true}` 才允许进入发布队列。

构建还会生成 `review-dashboard.html`，把 QA 门禁、语义评分、变体排序、反馈账本和最终文案汇总成一个自包含审查面板；`dashboard-qa.json` 必须为绿。

语义 QA 现在包含 AI 指纹与共鸣审计：检查空泛形容词、AI 腔、句长节奏、具体产物、读者痛点和行动问题；未通过会写入 `copy-review.json.style` 并阻止发布。

`qa.json`、`copy-review.json`、`variants.json` 和 `dashboard-qa.json` 必须同时通过才允许进入小红书发布队列。`--draft` 可让 watcher 只填充小红书表单，不点击发布：

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

真实发布后 watcher 会自动在 `showcase/content/publication-ledger.jsonl` 建立零指标待补录记录。拿到平台数据后，把可补录字段写进 JSON 文件并按 release 更新：

```powershell
python showcase/scripts/content_memory.py update --release "v2.3.7-beta.3" --record feedback.json
python showcase/scripts/content_memory.py summary
python showcase/scripts/performance_report.py --output-dir showcase/reports/performance
```

绩效报告会分开列出 `complete` 与 `pending` 记录；`pending` 只作为待补录清单，不参与公式和钩子学习。

记录字段包括 release、标题、公式 ID、钩子类型、曝光、赞、藏、评、转发、关注和一句复盘。下一次构建会读取这份资产：优先选择验证过的公式，并对最近连续使用过的公式降权。
