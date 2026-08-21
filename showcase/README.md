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

完整构建会在语义 QA 通过后生成 `wechat/readmd-wechat.html`。该 HTML 只用行内样式，不包含脚本、外链、class、id 或图片；`wechat/wechat-qa.json` 也必须为 `{"ok":true}` 才允许进入发布队列。

`qa.json` 和 `copy-review.json` 必须同时通过才允许进入小红书发布队列。`--draft` 可让 watcher 只填充小红书表单，不点击发布：

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

发布后把真实数据写进 JSON 文件，再追加到 `showcase/content/publication-ledger.jsonl`：

```powershell
python showcase/scripts/content_memory.py record --record feedback.json
python showcase/scripts/content_memory.py summary
```

记录字段包括 release、标题、公式 ID、钩子类型、曝光、赞、藏、评、转发、关注和一句复盘。下一次构建会读取这份资产：优先选择验证过的公式，并对最近连续使用过的公式降权。
