# 2026-09-09 桌宠可见性修复

Live2D 模型已加载但桌面窗口全透明：已安装扩展的页面样式令 `#root` 占满窗口，Live2D 却把 canvas 追加到 body 最后。实测窗口高度 301px，画布 top 为 301.33px，被 overflow:hidden 完全裁掉。`live2dReady=true` 无法证明画布可见。

修复将画布放入 `#root`，并统一扩展页面的透明、满窗口样式。新增 `ui-tests/pet-live2d-layout.cjs` 使用真实 Electron，断言画布父节点、top=0 及画布底部不超出窗口。

本轮验证：扩展构建成功；新增原生布局回归通过；通过 `dist/ReadMD/ReadMD.exe` 的实际配置界面切换 Live2D，修复后用 Windows 屏幕抓取确认角色已显示在桌面。只验证这次的显示问题，没有重跑旧插件或全量测试。

本机 `%APPDATA%/ReadMD/plugins/pet/hermes-adapter/app/renderer` 已更新。源码构建目录和 `dist/ReadMD` 均提供新的 `ReadMD-Desktop-Pet.zip`。调试改动已撤销；本地备份与截图在 `scratch/pet-debug-2026-09-09/`。

系统安装目录 `Z:/Program Files/ReadMD/ReadMD.exe` 仍为 9 月 3 日的旧版本；本轮实际运行的是项目 `dist/ReadMD/ReadMD.exe`。既有 `assets/js/features/pet-batch.js` 未提交的交互改动保持原样，不纳入本次修复提交。
