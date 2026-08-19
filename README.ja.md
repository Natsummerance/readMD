<p align="center">
  🌐 <b>Languages / 言語の切り替え</b>: 
  <a href="README.md">简体中文</a> | 
  <a href="README.zh-TW.md">繁體中文</a> | 
  <a href="README.en.md">English</a> | 
  <b>日本語</b>
</p>

<div align="center">

<img src="assets/icon-256.png" width="96" alt="ReadMD logo">

# 📖 ReadMD · 軽量 Markdown ビューアー＆エディター

**完全ローカル · 爆速起動 · オフライン対応** の Windows / macOS 向け Markdown リーダー＆エディター。

`.md` ファイルをダブルクリックするだけですぐに読める。レンダリング前によくある Markdown の構文エラー（テーブル崩れ、太字閉じ忘れ、LaTeX 数式エラー、見出しスペース不足など）を自動修復します。**元のファイルを書き換えることは一切なく、プレビュー表示のみを最適化します**。内蔵 AI アシスタント、Office/PDF ファイルの一括変換、システムネイティブ OCR、Web ページ抽出、直接編集、ローカル Wi-Fi 経由でのスマホ共有機能を統合。

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-0078d6)
![version](https://img.shields.io/badge/version-v2.3.0-3b6ef5)
![webview2](https://img.shields.io/badge/runtime-WebView2-4fc08d)
![license](https://img.shields.io/badge/license-MIT-green)
![i18n](https://img.shields.io/badge/i18n-46%20Languages-orange)

<br>

<p align="center">
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_インストーラー-v2.3.0-2563eb?style=for-the-badge&logo=windows&logoColor=white" alt="Windows インストーラーダウンロード">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe">
    <img src="https://img.shields.io/badge/⬇️_Windows_ポータブル版-v2.3.0-0284c7?style=for-the-badge&logo=windows&logoColor=white" alt="Windows ポータブル版ダウンロード">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Apple_Silicon)-v2.3.0-111827?style=for-the-badge&logo=apple&logoColor=white" alt="macOS ARM64 ダウンロード">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip">
    <img src="https://img.shields.io/badge/⬇️_macOS_(Intel)-v2.3.0-4b5563?style=for-the-badge&logo=apple&logoColor=white" alt="macOS Intel ダウンロード">
  </a>
  <a href="https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix">
    <img src="https://img.shields.io/badge/🧩_VSCode_拡張機能-v2.3.0-6366f1?style=for-the-badge&logo=visualstudiocode&logoColor=white" alt="VSCode VSIX ダウンロード">
  </a>
</p>

</div>

---

## ✨ 主な機能

- ⚡ **爆速起動**：インストール版は onedir ディレクトリ配置を採用し、コールドスタート 1.5 秒以内（低スペック機でも 2 秒以内）を実現。システムトレイ常駐時は 0.3 秒以内で瞬時に呼び出し可能。
- 🌍 **世界 46 以上の多言語に対応 (v2.3.0)**：初回起動時に OS のシステム言語を自動検知して初期化。日本語、英語、繁体字、簡体字、ヨーロッパ諸言語、アラビア語・ヘブライ語の双方向テキスト (RTL) を完備。
- 📐 **LaTeX PRO 学術論文スイート (v2.3.0)**：同じフォルダ内にある `.bib` 参考文献ファイルをゼロコンフィグで自動検知。文中の引用バッジにマウスホバーで論文情報・DOI リンク・BibTeX ワンクリックコピーを表示。定理 (Theorem)、補題 (Lemma)、証明 (Proof / Q.E.D. ■ 記号付き)、定義 (Definition) などの学術コールアウトに対応。
- 🧘 **Editor Studio PRO (v2.3.0)**：Zen モード（<kbd>F11</kbd> / <kbd>Esc</kbd> で即座に集中執筆）、10x10 マウス操作テーブルデザイナー、Excel / CSV のコピー＆ペーストによるスマート表変換、リアルタイム文字数・読了目安時間表示。
- 🔌 **VSCode 公式拡張機能 & MCP サーバー (v2.3.0)**：Visual Studio Code 上で ReadMD と完全一致の同期プレビュー＆自動修復を提供する拡張機能と、Claude Desktop や Cursor 等と連携できる標準 FastMCP (stdio) サーバーを同梱。
- 🛡️ **スムーズなアプリ内アップデート**：アップデート適用時のファイルロック問題を解消し、起動時に不要な一時インストーラーを自動クリーンアップ。
- 🎨 **クリーンな UI デザイン**：コンパクトな 44px ツールバー、ライト / ダーク / セピアの 3 つのデザインテーマ、大容量ドキュメント向けスケルトンスクリーン、OS の「視覚効果を減らす」設定に完全追従。
- 🤖 **AI アシスタント機能**：OpenAI、DeepSeek、Kimi、Anthropic、Ollama などの主要プロバイダーに対応。API キーはローカルにのみ暗号化保存。文脈を保ったチャット、文章の推敲・要約・コードレビューが可能。
- 🔄 **多彩な形式から Markdown への変換**：Word (.docx)、PowerPoint (.pptx)、Excel (.xlsx)、PDF、HTML、LaTeX (.tex) を Markdown へワンクリック変換し、新しいタブですぐにプレビュー。
- 🔍 **OS ネイティブのオフライン OCR**：Windows (WinRT) および macOS (Vision) のローカル OCR エンジンを利用し、プライバシーを完全保護。クリップボード画像から瞬時に文字抽出。
- 🌐 **Web ページから Markdown 抽出**：Trafilatura エンジンとヘッドレス WebView（Defuddle / Readability）を組み合わせ、動的な Single Page Application (SPA) でも本文を正確に抽出。
- 📱 **ローカル Wi-Fi スマホ共有**：QR コードを読み取るだけで、同一 Wi-Fi 内のスマートフォンから安全にドキュメントを閲覧・編集（暗号化トークン認証付き）。
- 📤 **高度なエクスポート**：PDF、Word (DOCX)、スタンドアロン HTML、コンパイル可能な LaTeX 論文ソースコード (.tex) の出力に対応。

---

## 🚀 直接ダウンロード一覧

| プラットフォーム / 種別 | ダウンロードリンク | 概要 |
| :--- | :--- | :--- |
| 🪟 **Windows インストーラー** | [⬇️ **ReadMDSetup-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMDSetup-v2.3.0.exe) | `.md` 関連付けを自動設定するセットアップウィザード付き |
| 💼 **Windows ポータブル版** | [⬇️ **ReadMD-portable-v2.3.0.exe**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-portable-v2.3.0.exe) | インストール不要のスタンドアロン実行ファイル |
| 🍏 **macOS Apple Silicon** | [⬇️ **ReadMD-macos-arm64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-arm64-v2.3.0.zip) | M1 / M2 / M3 / M4 チップ搭載 Mac 向けネイティブビルド |
| 💻 **macOS Intel** | [⬇️ **ReadMD-macos-x64-v2.3.0.zip**](https://github.com/Natsummerance/readMD/releases/latest/download/ReadMD-macos-x64-v2.3.0.zip) | Intel プロセッサー搭載 Mac 向けネイティブビルド |
| 🧩 **VSCode 拡張機能** | [⬇️ **readmd-vscode-2.3.0.vsix**](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) | Visual Studio Code 用オフライン拡張パッケージ |
| 🔐 **SHA-256 検証リスト** | [⬇️ **SHA256SUMS.txt**](https://github.com/Natsummerance/readMD/releases/latest/download/SHA256SUMS.txt) | 配布ファイルの整合性確認用チェックサム |

---

## 🧩 VSCode 拡張機能のインストール手順

ReadMD 公式の VSCode 拡張機能を使用すると、Visual Studio Code 内で ReadMD と同一のプレビュー表示および Markdown 自動修復機能を利用できます。

### 方法 1：VSCode 画面から直接インストール（推奨）
1. [`readmd-vscode-2.3.0.vsix`](https://github.com/Natsummerance/readMD/releases/latest/download/readmd-vscode-2.3.0.vsix) をダウンロードします。
2. VSCode で <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>（Mac では <kbd>Cmd</kbd>+<kbd>Shift</kbd>+<kbd>X</kbd>）を押して拡張機能パネルを開きます。
3. 拡張機能パネル右上にある **`...` (その他の操作)** メニューをクリックします。
4. **`VSIX からのインストール... (Install from VSIX...)`** を選択します。
5. ダウンロードした `.vsix` ファイルを選択すると、数秒でインストールが完了します！

### 方法 2：コマンドラインから一括インストール
ターミナルで以下のコマンドを実行します：
```bash
code --install-extension readmd-vscode-2.3.0.vsix
```

---

## 🤖 MCP (Model Context Protocol) サーバー設定

ReadMD は標準 FastMCP (stdio) サーバーを内蔵しており、Claude Desktop、Cursor、Antigravity、Cline などの AI 開発環境とシームレスに連携できます。

### 設定例

#### 1. Claude Desktop
`claude_desktop_config.json` に以下を追記します：
```json
{
  "mcpServers": {
    "readmd": {
      "command": "python",
      "args": ["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]
    }
  }
}
```

#### 2. Cursor / Cline
MCP 設定画面で以下を追加します：
- **Name**: `readmd`
- **Command**: `python`
- **Args**: `["/path/to/readmd/packages/mcp-server/readmd_mcp_server.py"]`

#### 🛠️ 提供される主な MCP ツール：
- `readmd_fix_markdown`: Markdown の構文・数式・表の崩れを自動修復
- `readmd_convert_to_markdown`: 各種ドキュメント（Word/PDF/Excel 等）を Markdown に変換
- `readmd_latex_to_md`: LaTeX の論文や数式を Markdown に変換
- `readmd_md_to_latex`: Markdown を学術標準の LaTeX ソースに変換
- `readmd_parse_bibtex`: `.bib` ファイルを解析し構造化データを取得

---

## ⌨️ 主なショートカットキー

| ショートカット | 動作 |
| :--- | :--- |
| <kbd>F11</kbd> / <kbd>Esc</kbd> | Zen モードの切り替え / 全画面解除 |
| <kbd>F2</kbd> | 現在のファイル名を変更（拡張子保護付き） |
| <kbd>Ctrl</kbd> + <kbd>O</kbd> | ファイルを開く |
| <kbd>Ctrl</kbd> + <kbd>E</kbd> | 閲覧モードと編集モードの切り替え |
| <kbd>Ctrl</kbd> + <kbd>S</kbd> | ファイルを保存 |
| <kbd>Ctrl</kbd> + <kbd>F</kbd> | 全文検索と置換 |
| <kbd>Ctrl</kbd> + <kbd>P</kbd> | PDF / DOCX / HTML / LaTeX のエクスポート |
| <kbd>Ctrl</kbd> + <kbd>D</kbd> | テーマの切り替え（ライト / ダーク / セピア） |
| <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>A</kbd> | AI アシスタントパネルを開く |
| <kbd>Ctrl</kbd> + <kbd>=</kbd> / <kbd>-</kbd> | フォントサイズの拡大 / 縮小 |

---

<p align="center">
  <b>ReadMD</b> は完全無料・オープンソース・ローカルファーストのソフトウェアです。<br>
  Developed with ❤️ by <a href="https://github.com/Natsummerance">Natsummerance</a>.
</p>
