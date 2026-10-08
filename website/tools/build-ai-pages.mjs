import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../public');
const origin='https://rust.readmd.asia';
const copies=[
 ['','en','Your next thought. One Tab away.','AI autocomplete in V0.0.5','Suggestions stream beside your cursor. Tab accepts the visible words; Esc dismisses, and undo restores your draft.','Opt in from the editor view menu. Advanced AI settings can choose a fast model automatically or use your own completion model. Only nearby context goes to your saved connection; nothing is saved automatically.','One Rust engine, two editors','The desktop app and VS Code share context limits, filtering and saved credentials. MCP exposes the same completion engine.','Platform support, with evidence','Windows is tested locally. macOS and Linux use native hosts. UOS, Kylin, Deepin, openEuler and Anolis need native acceptance; HarmonyOS is a source-integrated candidate awaiting SDK and device verification.','Release candidate · not yet published','Back to ReadMD'],
 ['zh-cn/','zh-CN','下一句灵感，按 Tab 接上。','V0.0.5 · AI 自动补全','灰色候选在光标旁流式出现。Tab 接受眼前的文字，Esc 忽略；不满意，一次撤销回到原稿。','在编辑视图菜单主动开启。AI 高级参数可自动优先选择快模型，也可独立指定补全模型。仅将附近内容发送给已保存的连接，不自动写入文件。','同一个 Rust 核心，两种编辑环境','桌面与 VS Code 共用上下文限制、输出过滤和已保存凭据；MCP 也可调用同一个补全引擎。','平台支持，以验证为准','Windows 已在本机实测；macOS 与 Linux 采用原生宿主。UOS、麒麟、Deepin、openEuler 和龙蜥仍需原生验收；鸿蒙已接入源码，等待 SDK 编译与设备验证。','发布候选 · 尚未正式发布','回到 ReadMD'],
 ['zh-tw/','zh-TW','下一句靈感，按 Tab 接上。','V0.0.5 · AI 自動補全','灰色候選在游標旁串流出現。Tab 接受眼前的文字，Esc 忽略；不滿意，一次復原回到原稿。','在編輯檢視選單主動開啟。AI 進階參數可自動優先選擇快模型，也可獨立指定補全模型。僅將附近內容傳送給已儲存的連線，不自動寫入檔案。','同一個 Rust 核心，兩種編輯環境','桌面與 VS Code 共用上下文限制、輸出過濾與已儲存憑證；MCP 也能呼叫相同補全引擎。','平台支援，以驗證為準','Windows 已在本機實測；macOS 與 Linux 採用原生宿主。國產 Linux 發行版仍需原生驗收；鴻蒙已接入原始碼，等待 SDK 編譯與裝置驗證。','發布候選 · 尚未正式發布','回到 ReadMD'],
 ['ja/','ja','次のひらめきを、Tab でつなぐ。','V0.0.5 · AI 自動補完','カーソル横に候補が順次表示されます。Tab で見えている文字を確定、Esc で破棄。元に戻す操作で原稿を復元できます。','編集表示メニューで有効にします。AI の詳細設定で高速モデルの自動選択や補完専用モデルを指定できます。周辺の文章のみを保存済み接続に送信し、自動保存はしません。','同じ Rust エンジンを二つのエディターで','デスクトップと VS Code はコンテキスト制限と出力フィルターを共有。MCP も同じ補完エンジンを呼び出します。','検証に基づくプラットフォーム対応','Windows はローカル実測済み。macOS と Linux はネイティブホストを使用。中国の Linux 環境は実機検証待ち。HarmonyOS はソース統合済みで SDK と端末検証が必要です。','リリース候補 · 未公開','ReadMD に戻る']
];
const esc=s=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('"','&quot;');
const player='<video class="w-full rounded-3xl border border-line" controls playsinline preload="none" poster="/media/v005-ai-autocomplete.webp"><source src="/media/v005-ai-autocomplete.mp4" type="video/mp4"></video>';
for(const [prefix,lang,title,label,lead,privacy,shared,sharedCopy,platform,platformCopy,candidate,back]of copies){
 const url=origin+'/'+prefix+'ai-autocomplete/';
 const alternates=copies.map(c=>'<link rel="alternate" hreflang="'+c[1]+'" href="'+origin+'/'+c[0]+'ai-autocomplete/">').join('');
 const html='<!doctype html><html lang="'+lang+'"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>'+esc(label)+' — ReadMD</title><meta name="description" content="'+esc(lead)+'"><link rel="canonical" href="'+url+'">'+alternates+'<link rel="alternate" hreflang="x-default" href="'+origin+'/ai-autocomplete/"><link rel="stylesheet" href="/assets/site.css"><meta property="og:type" content="website"><meta property="og:title" content="'+esc(label)+'"><meta property="og:description" content="'+esc(lead)+'"><meta property="og:url" content="'+url+'"><meta property="og:image" content="'+origin+'/media/overview-reader.png"></head><body class="text-ink"><header class="border-b border-line"><nav class="mx-auto max-w-6xl px-5 py-5"><a class="flex items-center gap-3 font-semibold" href="/'+prefix+'"><img src="/assets/icon-256.png" width="32" height="32" alt="">ReadMD</a></nav></header><main class="mx-auto max-w-6xl px-5 py-16 md:py-24"><p class="text-sm text-muted">'+label+' · '+candidate+'</p><h1 class="mt-6 max-w-4xl text-4xl md:text-6xl font-bold tracking-tight">'+title+'</h1><p class="mt-6 text-xl text-muted max-w-3xl">'+lead+'</p><figure class="mt-12">'+player+'<figcaption class="mt-3 text-sm text-muted">'+({'zh-CN':'真实流式录屏，未加速模型等待；响应速度因模型与网络而异。','zh-TW':'真實串流錄影，未加速模型等待；回應速度因模型與網路而異。','ja':'実際のストリーミング録画です。待ち時間は短縮していません。応答速度はモデルとネットワークによって異なります。'}[lang]||'Real streaming capture, without speeding up model waiting. Response time varies by model and network.')+'</figcaption></figure><p class="mt-6 text-muted max-w-3xl">'+privacy+'</p><section class="grid md:grid-cols-2 gap-6 mt-16"><article class="rounded-3xl border border-line bg-card p-8"><h2 class="text-xl font-semibold">'+shared+'</h2><p class="mt-4 text-muted leading-relaxed">'+sharedCopy+'</p></article><article class="rounded-3xl border border-line bg-card p-8"><h2 class="text-xl font-semibold">'+platform+'</h2><p class="mt-4 text-muted leading-relaxed">'+platformCopy+'</p></article></section></main><footer class="mx-auto max-w-6xl px-5 py-8 border-t border-line"><a href="/'+prefix+'">'+back+'</a></footer><script src="/assets/site.js" defer></script></body></html>\n';
 const dir=path.join(root,prefix,'ai-autocomplete');fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,'index.html'),html);
 const home=path.join(root,prefix,'index.html');let s=fs.readFileSync(home,'utf8');
 const section='<section id="ai-autocomplete" class="mx-auto max-w-6xl px-5 py-16 md:py-24"><p class="text-sm text-muted">'+label+' · '+candidate+'</p><h2 class="mt-4 text-3xl md:text-5xl font-bold tracking-tight">'+title+'</h2><p class="mt-6 text-xl text-muted max-w-3xl">'+lead+'</p><a class="apple-pill-secondary inline-flex mt-6" href="/'+prefix+'ai-autocomplete/">'+({'zh-CN':'Tab · Esc · 撤销','zh-TW':'Tab · Esc · 復原','ja':'Tab · Esc · 元に戻す'}[lang]||'Tab · Esc · Undo')+' →</a></section>';
 if(s.includes('id="ai-autocomplete"'))s=s.replace(/<section id="ai-autocomplete"[\s\S]*?<\/section>/,section);else s=s.replace('</main>',section+'</main>');
 fs.writeFileSync(home,s);
}
const sitemap=path.join(root,'sitemap.xml');let xml=fs.readFileSync(sitemap,'utf8');
for(const c of copies){const url=origin+'/'+c[0]+'ai-autocomplete/';if(xml.includes('<loc>'+url+'</loc>'))continue;xml=xml.replace('</urlset>','<url><loc>'+url+'</loc>'+copies.map(x=>'<xhtml:link rel="alternate" hreflang="'+x[1]+'" href="'+origin+'/'+x[0]+'ai-autocomplete/"/>').join('')+'<xhtml:link rel="alternate" hreflang="x-default" href="'+origin+'/ai-autocomplete/"/><lastmod>2026-10-07</lastmod></url>\n</urlset>');}
fs.writeFileSync(sitemap,xml);
console.log('Generated four localized AI introduction pages.');

const feedFile=path.join(root,"feed.xml");let feed=fs.readFileSync(feedFile,"utf8");
for(const [prefix,lang,title,label,lead]of copies){
 const url=origin+"/"+prefix+"ai-autocomplete/";
 const entry='<entry><id>'+url+'</id><title>'+esc(label)+'</title><link rel="alternate" href="'+url+'"/><updated>2026-10-07T00:00:00Z</updated><summary>'+esc(lead)+'</summary></entry>';
 const existing=[...feed.matchAll(/<entry>[\s\S]*?<\/entry>/g)].find(m=>m[0].includes('<id>'+url+'</id>'));
 feed=existing?feed.replace(existing[0],entry):feed.replace('</feed>',entry+'\n</feed>');
 const llms=path.join(root,prefix,"llms.txt");let text=fs.readFileSync(llms,"utf8");
 if(lang==="en")text=text.replace("Current candidate version: V0.0.4 (formal release pending evidence)","Published stable version: V0.0.4; candidate V0.0.5 is not yet published");
 const line='- ['+label+']('+url+'): '+lead;
 const previous=text.split('\n').find(s=>s.includes(']('+url+')'));
 text=previous?text.replace(previous,line):text+'\n'+line+'\n';fs.writeFileSync(llms,text);
}
fs.writeFileSync(feedFile,feed);
