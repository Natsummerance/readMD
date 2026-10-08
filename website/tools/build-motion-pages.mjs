// Restore scroll storytelling around existing, genuine recordings. No media copy.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const site=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../public');
const release=JSON.parse(fs.readFileSync(new URL('../release.json',import.meta.url),'utf8'));
const locales={
 en:{prefix:'',kicker:'SCROLL TO EXPLORE',title:'Your documents.<br>In motion.',watch:'Watch the real operation',
  support:'ReadMD V'+release.stable+' provides Windows x64, macOS Intel and Apple Silicon, and Linux x86_64 packages. VS Code and MCP use the installed Rust engine. UOS and Kylin still require native installation testing.'},
 'zh-CN':{prefix:'zh-cn/',kicker:'滚动，探索每一步',title:'读、写、想。<br>随你心意。',watch:'观看真实操作',
  support:'ReadMD V'+release.stable+' 提供 Windows x64、macOS Intel / Apple Silicon 和 Linux x86_64 安装包。VS Code 与 MCP 共用已安装的 Rust 内核；统信 UOS、银河麒麟仍需原生安装验收。'},
 'zh-TW':{prefix:'zh-tw/',kicker:'捲動，探索每一步',title:'讀、寫、想。<br>隨你心意。',watch:'觀看實際操作',
  support:'ReadMD V'+release.stable+' 提供 Windows x64、macOS Intel / Apple Silicon 與 Linux x86_64 安裝套件。VS Code 與 MCP 共用已安裝的 Rust 核心；統信 UOS、銀河麒麟仍需原生安裝驗收。'},
 ja:{prefix:'ja/',kicker:'スクロールして体験',title:'読む、書く、考える。<br>思いのままに。',watch:'実際の操作を見る',
  support:'ReadMD V'+release.stable+' は Windows x64、macOS Intel / Apple Silicon、Linux x86_64 のパッケージを提供します。VS Code と MCP はインストール済み Rust エンジンを共有。UOS と Kylin はネイティブ環境での検証が必要です。'}
};
for(const c of Object.values(locales)){
 const file=path.join(site,c.prefix,'index.html');let html=fs.readFileSync(file,'utf8');
 const cards=[...html.matchAll(/<a class="demo-card" href="([^\"]+)">([\s\S]*?)<\/a>/g)].map(m=>({
  href:m[1],id:m[1].split('#')[1],title:m[2].match(/<h3>([\s\S]*?)<\/h3>/)?.[1],
  copy:m[2].match(/<p>([\s\S]*?)<\/p>/)?.[1],poster:m[2].match(/<img src="([^\"]+)"/)?.[1]
 }));
 if(cards.length!==6||cards.some(v=>!v.title||!v.copy||!v.poster||!/^F\d{3}$/.test(v.id)))throw Error('Six genuine demo cards are required');
 const captions=cards.map((v,i)=>'<article class="journey-caption'+(i===0?' is-active':'')+'" data-video="/showcase/videos/'+v.id+'.mp4" data-poster="'+v.poster+'"><strong>'+v.title+'</strong><p>'+v.copy+'</p><a class="link-action" href="'+v.href+'">'+c.watch+' <span aria-hidden="true">›</span></a></article>').join('');
 const section='<section id="journey" class="journey" data-readmd-cinema aria-labelledby="journey-title"><div class="journey-sticky"><div class="journey-aura" aria-hidden="true"></div><div class="journey-grid"><div><p class="journey-kicker">'+c.kicker+'</p><h2 id="journey-title" class="journey-heading">'+c.title+'</h2><div class="journey-caption-stage">'+captions+'</div></div><figure class="journey-stage"><img src="'+cards[0].poster+'" width="1280" height="800" loading="lazy" alt="'+cards[0].title+' · ReadMD"><div class="journey-progress" aria-hidden="true"><span></span></div></figure></div></div></section>';
 if(html.includes('data-readmd-cinema'))html=html.replace(/<section id="journey"[\s\S]*?<\/section>/,section);
 else html=html.replace('<section id="capabilities"',section+'\n<section id="capabilities"');
 if(!html.includes('class="home-logo ')) html=html.replace('class="text-lg font-semibold tracking-tight" href="/'+c.prefix+'">ReadMD</a>',
  'class="home-logo text-lg font-semibold tracking-tight" href="/'+c.prefix+'"><img src="/assets/icon-256.png" width="26" height="26" alt="" aria-hidden="true">ReadMD</a>');
 html=html.replace('<img src="/assets/icon-256.png" width="26" height="26" alt="">','<img src="/assets/icon-256.png" width="26" height="26" alt="" aria-hidden="true">');
 for(const match of html.matchAll(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g)){
  const block=JSON.parse(match[1]);
  if(block['@type']==='FAQPage')for(const item of block.mainEntity||[]){
   const previous=item.acceptedAnswer?.text;
   if(previous&&/ReadMD\s+v?2\.3\.9/.test(previous))html=html.replaceAll(previous,c.support);
  }
 }
 fs.writeFileSync(file,html);
}
let count=0;
function walk(dir){for(const entry of fs.readdirSync(dir,{withFileTypes:true})){
 const file=path.join(dir,entry.name);if(entry.isDirectory()){walk(file);continue;}
 if(!file.endsWith('.html'))continue;
 let html=fs.readFileSync(file,'utf8');
 html=html.replace(/("softwareVersion"\s*:\s*")[^"]+(")/g,(_,start,end)=>start+release.stable+end);
 if(!html.includes('/assets/motion.css'))html=html.replace('</head>','<link rel="stylesheet" href="/assets/motion.css">\n</head>');
 if(!html.includes('/assets/motion.js'))html=html.replace('</body>','<script src="/assets/motion.js" defer></script></body>');
 fs.writeFileSync(file,html);count++;
}}
walk(site);console.log('Enhanced '+count+' pages; four scroll-video stories use existing recordings.');
