# ReadMD HarmonyOS/OpenHarmony 源码预览（未支持）

本目录仅保留实验性 ArkTS 外壳，**不属于 ReadMD v2.3.7 正式支持矩阵**，也不代表核心功能、安装包或兼容性承诺。HarmonyOS/OpenHarmony 将在有真实设备和完整原生验收证据后另行评估。

## 架构说明

- **视窗载体**：ArkUI 原生 `<Web src="$rawfile('index.html')"/>` 容器；
- **排版与渲染核心**：完全复用 ReadMD 标准离线 Web 资源（Marked + KaTeX + 46 语种 i18n + LaTeX PRO）；
- **系统级桥接 (ReadMDBridge)**：
  - `@ohos.pasteboard`：系统剪贴板双向交互；
  - `@ohos.file.picker` & `@ohos.file.fs`：原生文件读写与另存为；
  - `@ohos.i18n`：系统语言自适应检测；
  - `@ohos.ai.OCR`：鸿蒙系统原生离线文字识别。

## 编译与打包

1. 在本目录执行 `npm install`，再执行 `npm run sync:web` 打包离线 Web 资源；
2. 使用 **DevEco Studio NEXT** 打开本目录；
3. 选择 `entry` 模块并点击 **Build -> Build Hap(s) / APP(s) -> Build Hap(s)**；
4. 生成产物位于 `entry/build/default/outputs/default/entry-default-unsigned.hap`。

当前工程提供可构建的 ArkUI/ArkWeb 外壳与资源同步脚本；完整桌面后端能力仍需继续接入原生文件、剪贴板和 OCR 桥接。请勿将其作为发布资产或“全平台支持”证据。
