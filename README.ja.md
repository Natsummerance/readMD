<p align="center">
  <b>Languages</b>:
  <a href="README.md">简体中文</a> |
  <a href="README.zh-TW.md">繁體中文</a> |
  <a href="README.en.md">English</a> |
  <b>日本語</b>
</p>

<div align="center">
  <img src="assets/icon-256.png" width="88" alt="ReadMD logo">

  # ReadMD

  **巨大な Markdown ファイルもローカルのまま。元ファイルは書き換えません。**

  ReadMD はローカルファーストの Markdown リーダーおよびエディターです。ダブルクリックで即時表示し、巨大ドキュメントもストレスなく閲覧できます。一般的な構文エラーは表示時にのみ修復し、Office/PDF/Web からの変換、オフライン OCR、LaTeX 学術支援、MCP 連携に対応します。

  [![platform](https://img.shields.io/badge/Windows%20%7C%20macOS%20%7C%20Linux%20%7C%20KylinOS%20%7C%20UOS-blue)](#プラットフォーム別ダウンロード-release-assets)
  [![i18n](https://img.shields.io/badge/languages-46-orange)](docs/i18n-language-reference.md)
  [![license](https://img.shields.io/badge/license-MIT-green)](LICENSE)
  [![release](https://img.shields.io/github/v/release/Natsummerance/readMD)](https://github.com/Natsummerance/readMD/releases/latest)
  [![website](https://img.shields.io/badge/site-readmd.asia-black)](https://readmd.asia)
</div>

## こんな用途に向いています

| 状況 | ReadMD の動作 |
| --- | --- |
| 8,000 行または 500KB を超える文件 | セマンティックページ分割で目次と Ctrl+F 検索を使いやすく保ちます |
| 表の区切り線や数式が壊れている場合 | メモリ上の表示だけを修復し、元ファイルは変更しません |
| DOCX / PPTX / XLSX / PDF / HTML がある | Markdown への変換で手作業を減らします |
| 画像内の文字が必要 | 対応環境ではシステム OCR をオフラインで利用できます |
| 論文や技術資料を書く場合 | BibTeX カード、定理・証明ブロック、LaTeX エクスポートを支援します |
| AI ワークフローに接したい | VS Code 拡張と FastMCP stdio サーバーを提供します |

## 直接ダウンロード

[Windows インストーラー](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMDSetup-v2.3.7-beta.5.exe) ·
[Windows ポータブル版](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-portable-v2.3.7-beta.5.exe) ·
[macOS Apple Silicon](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-macos-arm64-v2.3.7-beta.5.zip) ·
[macOS Intel](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-macos-x64-v2.3.7-beta.5.zip) ·
[Linux AppImage](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-linux-x86_64-v2.3.7-beta.5.AppImage) ·
[Linux ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-linux-aarch64-v2.3.7-beta.5.AppImage) ·
[UOS / 麒麟 Deb](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd_2.3.7-beta.5_amd64.deb) ·
[麒麟 V10 ARM64](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd_2.3.7-beta.5_arm64.deb) ·
[SHA-256](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/SHA256SUMS.txt)

## 3 ステップで開始

1. 環境に合うインストーラーまたはポータブル版を入手します。
2. 実際に大きいファイルを開き、目次・検索・表表示を確認します。
3. 編集は保存したいときだけ行います。表示修復は勝手に元ファイルへ書き込みません。

## AI アシスタント引用リソース

- [製品インデックス](https://readmd.asia/llms.txt): バージョン、対応環境、プライバシー境界、主要な事実。
- [完全引用コーパス](https://readmd.asia/llms-full.txt): 長文の改ページ、非破壊修復、変換、よくある質問への直接回答。

## Star が役に立つ理由

ReadMD は、長期間保管する資料で起こりやすい問題に取り組みます。大きなファイルも読み続けられ、取り込んだ資料の整理作業が減り、機密性の高い草稿はローカルに残せます。Windows、macOS、Linux、中国 OS の間でも同じ操作感を維持できます。

もし整理時間の短縮に役立ったら、[リポジトリに Star](https://github.com/Natsummerance/readMD) を付けて他の執筆者にも見つけてもらいましょう。


## 🚀 プラットフォーム別ダウンロード (Release Assets)

| プラットフォーム | アーキテクチャ / 形式 | 直接ダウンロードリンク (GitHub Release) | 概要 |
| :--- | :--- | :--- | :--- |
| 🪟 **Windows** | x64 (インストーラー) | [⬇️ **ReadMDSetup-v2.3.7-beta.5.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMDSetup-v2.3.7-beta.5.exe) | `.md` 関連付けを自動登録するセットアップ版 |
| 💼 **Windows** | x64 (ポータブル版) | [⬇️ **ReadMD-portable-v2.3.7-beta.5.exe**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-portable-v2.3.7-beta.5.exe) | インストール不要の単一実行ファイル |
| 🍏 **macOS** | Apple Silicon (M1〜M4) | [⬇️ **ReadMD-macos-arm64-v2.3.7-beta.5.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-macos-arm64-v2.3.7-beta.5.zip) | Apple Silicon Mac 向けネイティブビルド (Vision OCR 内蔵) |
| 💻 **macOS** | Intel x86_64 | [⬇️ **ReadMD-macos-x64-v2.3.7-beta.5.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-macos-x64-v2.3.7-beta.5.zip) | Intel Mac 向けネイティブビルド (Vision OCR 内蔵) |
| 🐧 **Linux** | x86_64 (AppImage) | [⬇️ **ReadMD-linux-x86_64-v2.3.7-beta.5.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-linux-x86_64-v2.3.7-beta.5.AppImage) | インストール不要の Linux AppImage パッケージ |
| 🐧 **Linux** | ARM64 (AppImage) | [⬇️ **ReadMD-linux-aarch64-v2.3.7-beta.5.AppImage**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/ReadMD-linux-aarch64-v2.3.7-beta.5.AppImage) | Phytium / Kunpeng など ARM64 端末向けパッケージ |
| 🇨🇳 **Linux / 国産 OS** | Debian / Ubuntu / UOS / 麒麟 | [⬇️ **readmd_2.3.7-beta.5_amd64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd_2.3.7-beta.5_amd64.deb) | Deb ネイティブインストールパッケージ |
| 🖥️ **麒麟 V10 / 飛騰** | ARM64 (aarch64) | [⬇️ **readmd_2.3.7-beta.5_arm64.deb**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd_2.3.7-beta.5_arm64.deb) | ARM64 ネイティブ Deb パッケージ |
| 📱 **HarmonyOS** | ソースプロジェクト (DevEco ビルド) | [🧩 **packages/harmonyos-app**](https://github.com/Natsummerance/readMD/tree/main/packages/harmonyos-app) | ArkTS + ArkUI + ArkWeb のソースプロジェクト。ビルド済み HAP は未提供 |
| 🧩 **VSCode 拡張** | VSIX パッケージ | [⬇️ **readmd-vscode-2.3.7-beta.5.vsix**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd-vscode-2.3.7-beta.5.vsix) | オフラインインストール用 VSIX 拡張機能 |
| 🤖 **MCP Server** | FastMCP stdio パッケージ | [⬇️ **readmd-mcp-server-2.3.7-beta.5.zip**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/readmd-mcp-server-2.3.7-beta.5.zip) | Claude Desktop / Cursor 連携用 FastMCP パッケージ |
| 🔐 **チェックサム** | SHA-256 リスト | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/download/v2.3.7-beta.5/SHA256SUMS.txt) | 配布ファイルの整合性検証用チェックサム |

---

<div align="center">

**ReadMD** · 完全ローカル優先、全プラットフォーム対応 Markdown ツール。

</div>
