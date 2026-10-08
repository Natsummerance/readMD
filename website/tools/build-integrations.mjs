// Shared facts and native copy; keep the existing Apple-style site and CSS.
import fs from 'node:fs';
const release=JSON.parse(fs.readFileSync(new URL('../release.json',import.meta.url),'utf8'));
const site=new URL('../public/',import.meta.url), origin='https://rust.readmd.asia';
const locales={
 en:{prefix:'',name:'English',title:'Markdown in VS Code and AI workflows',lead:'One local Rust engine. Your documents stay in your workflow.',description:'Use ReadMD in VS Code and MCP clients: offline Markdown preview, document conversion and export, with version-protected local editing in V0.0.5.',candidate:'V0.0.5 release candidate · not published yet',stable:'Download the published release',back:'ReadMD home',cards:[
 ['VS Code, without another workspace','Live preview, math, diagrams and slides. V0.0.5 adds link diagnostics to the native Problems panel, clickable wiki links and local workspace search.'],
 ['AI edits you can review','V0.0.5 MCP reads a document revision, previews literal edits and writes only after explicit confirmation. Conflicting file versions stop the edit; recovery history stays in application data.'],
 ['Local files, current results','Search phrases, titles, paths and YAML tags directly in the workspace. Limits and skipped files are reported. URLs are never fetched during link inspection.']],
 flow:'Read → preview → confirm → recover',code:'Use the same executable installed with ReadMD. The MCP kit contains documentation and client configuration, not a second server binary.',docs:'Integration documentation',limits:'Bounded by design: 2 MiB per inspected/edited document, 5,000 searched Markdown files, 64 MiB per search, 8 concurrent MCP tools. Native runtimes and OS services are required for some formats.',more:'V0.0.5 adds 8 tools to the 22-tool stable core. Availability follows the installed desktop version.'},
 'zh-CN':{prefix:'zh-cn/',name:'简体中文',title:'让 Markdown 接入你的编辑器与 AI',lead:'同一套本地 Rust 内核，沿用你熟悉的工作方式。',description:'ReadMD 的 VS Code 与 MCP 集成：离线预览、格式转换与导出，V0.0.5 增加文档检查、工作区检索和可预览、可恢复的文件编辑。',candidate:'V0.0.5 发布候选 · 尚未正式发布',stable:'下载当前正式版',back:'ReadMD 首页',cards:[
 ['在 VS Code 中，保持专注','预览、公式、图表与演讲继续复用本地内核。V0.0.5 将链接检查放进原生“问题”面板，支持点击双链和搜索 Markdown 工作区。'],
 ['让 AI 编辑先经过审阅','V0.0.5 MCP 读取文件版本，默认只预览文字替换；明确确认后才写入。文件被其他操作修改时停止覆盖，恢复记录保存在应用数据区。'],
 ['直接检索当前文件','组合短语、标题、路径与 YAML 标签，不等待旧索引更新。检查范围、跳过文件与处理上限明确返回；链接检查不联网。']],
 flow:'读取 → 预览 → 确认 → 恢复',code:'使用已安装的 ReadMD 可执行程序启动 MCP。连接包提供文档与客户端配置，不重复附带服务端内核。',docs:'查看集成文档',limits:'明确边界：检查及编辑单文档上限 2 MiB；单次搜索最多 5,000 个 Markdown 文件、64 MiB 内容；MCP 最多并发 8 个工具。部分格式依赖本机运行时或系统服务。',more:'V0.0.5 在正式版 22 个工具基础上增加 8 个工具，具体能力以已安装的桌面版本为准。'},
 'zh-TW':{prefix:'zh-tw/',name:'繁體中文',title:'讓 Markdown 接入你的編輯器與 AI',lead:'同一套本機 Rust 核心，沿用你熟悉的工作方式。',description:'ReadMD 的 VS Code 與 MCP 整合：離線預覽、格式轉換與匯出，V0.0.5 增加文件檢查、工作區搜尋和可預覽、可復原的檔案編輯。',candidate:'V0.0.5 發布候選 · 尚未正式發布',stable:'下載目前正式版',back:'ReadMD 首頁',cards:[
 ['在 VS Code 中，保持專注','預覽、公式、圖表與簡報繼續共用本機核心。V0.0.5 將連結檢查放進原生「問題」面板，支援點擊雙向連結與搜尋 Markdown 工作區。'],
 ['讓 AI 編輯先經過審閱','V0.0.5 MCP 讀取檔案版本，預設只預覽文字取代；明確確認後才寫入。檔案被其他操作修改時停止覆寫，復原記錄保存在應用程式資料區。'],
 ['直接搜尋目前檔案','組合片語、標題、路徑與 YAML 標籤，不必等待舊索引更新。檢查範圍、略過檔案與處理上限明確回傳；連結檢查不連網。']],
 flow:'讀取 → 預覽 → 確認 → 復原',code:'使用已安裝的 ReadMD 執行檔啟動 MCP。連線包提供文件與用戶端設定，不重複附帶伺服器核心。',docs:'查看整合文件',limits:'明確邊界：檢查及編輯單份文件上限 2 MiB；單次搜尋最多 5,000 個 Markdown 檔案、64 MiB 內容；MCP 最多並行 8 個工具。部分格式依賴本機執行環境或系統服務。',more:'V0.0.5 在正式版 22 個工具基礎上增加 6 個工具，實際能力以已安裝的桌面版本為準。'},
 ja:{prefix:'ja/',name:'日本語',title:'Markdown をエディターと AI につなぐ',lead:'同じローカル Rust エンジンを、いつもの作業環境で。',description:'ReadMD の VS Code と MCP 連携。オフラインプレビュー、変換と書き出しに加え、V0.0.5 はリンク検査、ワークスペース検索と確認付きファイル編集を提供します。',candidate:'V0.0.5 リリース候補 · 未公開',stable:'公開済みの安定版をダウンロード',back:'ReadMD ホーム',cards:[
 ['VS Code で集中する','数式、図、スライドをローカルで表示。V0.0.5 は標準の問題パネルにリンク診断を表示し、Wiki リンクと Markdown ワークスペース検索に対応します。'],
 ['AI の編集を確認する','V0.0.5 MCP はファイルの版を読み取り、置換をまずプレビューします。明示的な確認後に保存し、競合時は停止。復元履歴はアプリのデータ領域に保存します。'],
 ['現在のファイルを検索','フレーズ、タイトル、パスと YAML タグを組み合わせます。処理上限とスキップを報告し、リンク検査では URL に接続しません。']],
 flow:'読み取り → プレビュー → 確認 → 復元',code:'インストール済み ReadMD の実行ファイルを使用します。MCP パッケージは文書と接続設定を提供し、サーバー本体は含みません。',docs:'連携ドキュメント',limits:'検査・編集は文書ごとに 2 MiB。検索は最大 5,000 ファイル、合計 64 MiB。MCP ツールは最大 8 件を並行実行。一部形式にはローカル実行環境や OS サービスが必要です。',more:'V0.0.5 は安定版の 22 ツールに 8 ツールを追加します。利用可能な機能はインストールした版によります。'}
};
// Localized copy is authored against this template version; future candidates
// update the shared release manifest instead of hand-editing four pages.
for (const c of Object.values(locales)) {
 for (const key of ['description','candidate','more']) c[key]=c[key].replaceAll('V0.0.5','V'+release.candidate);
 c.cards=c.cards.map(card=>card.map(text=>text.replaceAll('V0.0.5','V'+release.candidate)));
}
const esc=s=>s.replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
for(const [lang,c] of Object.entries(locales)){
 const url=origin+'/'+c.prefix+'integrations/';
 const docsRef=release.candidate_ref||'main';
 const alternate=Object.entries(locales).map(([l,v])=>'<link rel="alternate" hreflang="'+l+'" href="'+origin+'/'+v.prefix+'integrations/">').join('\n');
 const schema={'@context':'https://schema.org','@type':'WebPage',name:c.title,description:c.description,url,inLanguage:lang,dateModified:'2026-10-07',
   isPartOf:{'@type':'WebSite',name:'ReadMD',url:origin+'/'}};
 const html='<!doctype html>\n<html lang="'+lang+'"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">'+
 '<title>'+esc(c.title)+' | ReadMD</title><meta name="description" content="'+esc(c.description)+'"><meta name="robots" content="index,follow,max-image-preview:large,max-snippet:-1,max-video-preview:-1">'+
 '<link rel="canonical" href="'+url+'">'+alternate+'<link rel="alternate" hreflang="x-default" href="'+origin+'/integrations/">'+
 '<meta property="og:type" content="website"><meta property="og:title" content="'+esc(c.title)+'"><meta property="og:description" content="'+esc(c.description)+'"><meta property="og:url" content="'+url+'">'+
 '<meta property="og:image" content="'+origin+'/media/overview-reader.png"><meta name="twitter:card" content="summary_large_image"><meta name="twitter:image" content="'+origin+'/media/overview-reader.png">'+
 '<link rel="icon" href="/assets/icon-256.png"><link rel="manifest" href="/site.webmanifest"><link rel="alternate" type="application/atom+xml" href="/feed.xml"><link rel="stylesheet" href="/assets/site.css"><script type="application/ld+json">'+JSON.stringify(schema)+'</script></head>'+
 '<body class="text-ink"><header class="border-b border-line"><nav aria-label="Breadcrumb" class="mx-auto max-w-6xl px-5 py-5 flex items-center justify-between"><a class="flex items-center gap-3 font-semibold" href="/'+c.prefix+'"><img src="/assets/icon-256.png" width="32" height="32" alt="">ReadMD</a><a class="apple-pill-secondary" href="/'+c.prefix+'download/">'+esc(c.stable)+'</a></nav></header>'+
 '<main class="mx-auto max-w-6xl px-5 py-16 md:py-24"><p class="text-sm text-muted">'+esc(c.candidate)+'</p><h1 class="mt-6 max-w-4xl text-4xl md:text-6xl font-bold tracking-tight text-balance">'+esc(c.title)+'</h1>'+
 '<p class="mt-6 text-xl text-muted max-w-3xl">'+esc(c.lead)+'</p><section class="grid md:grid-cols-3 gap-6 mt-16">'+c.cards.map(([h,p])=>'<article class="rounded-3xl border border-line bg-card p-8"><h2 class="text-xl font-semibold">'+esc(h)+'</h2><p class="mt-4 text-muted leading-relaxed">'+esc(p)+'</p></article>').join('')+'</section>'+
 '<section class="mt-16 rounded-3xl border border-line p-8 md:p-12"><h2 class="text-2xl font-semibold">'+esc(c.flow)+'</h2><p class="mt-4 max-w-3xl text-muted">'+esc(c.code)+'</p><pre class="mt-6 p-6 rounded-2xl bg-surface overflow-x-auto"><code>readmd --mcp</code></pre>'+
 '<p class="mt-5 text-muted">'+esc(c.more)+'</p><a class="apple-pill-primary inline-flex mt-6" href="https://github.com/'+release.repository+'/tree/'+docsRef+'/packages/mcp-server">'+esc(c.docs)+'</a></section>'+
 '<p class="mt-8 text-sm text-muted leading-relaxed">'+esc(c.limits)+'</p></main><footer class="mx-auto max-w-6xl px-5 py-8 border-t border-line"><a href="/'+c.prefix+'">'+esc(c.back)+'</a><nav class="mt-4 flex flex-wrap gap-5" aria-label="Language">'+Object.values(locales).map(v=>'<a href="/'+v.prefix+'integrations/">'+v.name+'</a>').join('')+'</nav></footer><script src="/assets/site.js" defer></script></body></html>\n';
 const dir=new URL(c.prefix+'integrations/',site);fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(new URL('index.html',dir),html);
 const home=new URL(c.prefix+'index.html',site);let text=fs.readFileSync(home,'utf8');
 if(!text.includes('href="/'+c.prefix+'integrations/"')) text=text.replace('</footer>','<p class="text-center py-6"><a class="link-action" href="/'+c.prefix+'integrations/">'+esc(c.title)+'</a></p></footer>');
 fs.writeFileSync(home,text);
}
const sitemap=new URL('sitemap.xml',site);let xml=fs.readFileSync(sitemap,'utf8');
for(const c of Object.values(locales)){
 const url=origin+'/'+c.prefix+'integrations/';
 const alternates=Object.entries(locales).map(([l,v])=>'<xhtml:link rel="alternate" hreflang="'+l+'" href="'+origin+'/'+v.prefix+'integrations/"/>').join('')
   +'<xhtml:link rel="alternate" hreflang="x-default" href="'+origin+'/integrations/"/>';
 const entry='  <url><loc>'+url+'</loc>'+alternates+'<lastmod>2026-10-07</lastmod></url>';
 const existing='<url><loc>'+url+'</loc>';
 if(xml.includes(existing)){
   const start=xml.indexOf(existing),end=xml.indexOf('</url>',start)+6;xml=xml.slice(0,start)+entry.trim()+xml.slice(end);
 }else xml=xml.replace('</urlset>',entry+'\n</urlset>');
}
fs.writeFileSync(sitemap,xml);
console.log('Generated four localized integration pages with published/candidate distinction.');
