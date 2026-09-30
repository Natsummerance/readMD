# ReadMD Product Showcase

这条管线只发布真实运行截图，不生成或重绘产品 UI。
每个镜头可声明独立视口和素材文档：阅读/编辑保留工作台纵屏，放映使用 16:9，欢迎页与弹层使用无空白的桌面比例。发布前会按截图元数据复核尺寸，并用像素审计拦截长空带、近空白和比例错配。Reveal.js、Markdown、高亮、备注、KaTeX 及字体已本地化到 `assets/vendor`，放映页在 CSP 内离线渲染。

## V2.3.7 大版本更新海报

`update-v237/` 是 V2.3.6 → V2.3.7 的独立、可复现更新清单与海报源。它以 14 张互不重复的真实截图覆盖阅读、编辑、转换、学术、图表、代码、放映、共享、AI、Skills、Provider、GitHub 导入、语言和 MCP；每页的文案、功能来源、截图 SHA-256 与最终 PNG SHA-256 都由同一份清单约束。

```powershell
npm run test:v237-update --prefix showcase
npm run capture --prefix showcase
npm run build:v237-update --prefix showcase
npm run verify:v237-update --prefix showcase
```

成品位于 `showcase/output/v237-update/`：`index.html` 是本地浏览页，`update-list.zh-CN.md` 是可直接使用的中文大版本更新清单，`deck-evidence.json` 记录完整证据链。英文版可用 `node showcase/update-v237/render-deck.mjs --locale en --output showcase/output/v237-update-en` 生成。历史 beta 社媒包不受这组正式版视觉资产影响。

## 历史营销流程

小红书/微信的文案生产、海报审稿和自动发布流程依赖 Python 脚本与第三方平台账号，已随 ReadMD 移除 Python 依赖一起下线；已生成的海报、报告和发布账本仍保留在本目录中作为存档。截图录制（`npm run capture`、`npm run record`）与更新海报（`update-v23*`）使用 Node，不受影响。
