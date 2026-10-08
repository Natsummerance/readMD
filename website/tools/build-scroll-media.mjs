// Offline derivatives of existing recordings. Only ignored dist/ receives media.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const site=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const output=path.join(site,'dist/motion-clips');fs.mkdirSync(output,{recursive:true});
const html=fs.readFileSync(path.join(site,'public/index.html'),'utf8');
const ids=[...new Set([...html.matchAll(/data-video="\/showcase\/videos\/(F\d{3})\.mp4"/g)].map(m=>m[1]))];
if(ids.length!==6)throw Error('Build the six localized scroll chapters before media');
try{execFileSync('ffmpeg',['-version'],{stdio:'ignore',windowsHide:true});}catch{
 if(process.argv.includes('--require'))throw Error('The existing FFmpeg tool is required; this build never installs or downloads tools.');
 console.log('FFmpeg is not prepared: using the existing MP4 sources and poster fallback.');process.exit(0);
}
const report=[];
const recipe='vp9-crf20-g15-v1';
let cached;try{cached=JSON.parse(fs.readFileSync(path.join(output,'manifest.json'),'utf8'));}catch{}
for(const id of ids){
 const source=path.join(site,'public/showcase/videos',id+'.mp4'),target=path.join(output,id+'.webm');
 const sourceSha256=crypto.createHash('sha256').update(fs.readFileSync(source)).digest('hex');
 const previous=cached?.recipe===recipe&&cached.derivatives?.find(v=>v.id===id&&v.sourceSha256===sourceSha256);
 const reusable=previous&&fs.existsSync(target)&&crypto.createHash('sha256').update(fs.readFileSync(target)).digest('hex')===previous.sha256;
 if(!reusable)execFileSync('ffmpeg',['-nostdin','-hide_banner','-loglevel','error','-y','-i',source,'-map','0:v:0','-an',
  '-vf','scale=in_range=auto:out_range=tv,format=yuv420p','-color_range','tv','-c:v','libvpx-vp9','-b:v','0','-crf','20',
  '-g','15','-keyint_min','15','-auto-alt-ref','0','-row-mt','1','-cpu-used','4','-threads','2',target],{windowsHide:true});
 report.push({id,sourceSha256,sha256:crypto.createHash('sha256').update(fs.readFileSync(target)).digest('hex'),bytes:fs.statSync(target).size});
}
fs.writeFileSync(path.join(output,'manifest.json'),JSON.stringify({source:'existing genuine showcase recordings',recipe,codec:'VP9',derivatives:report},null,2)+'\n');
for(const prefix of ['','zh-cn/','zh-tw/','ja/']){
 const file=path.join(site,'dist',prefix,'index.html');
 const html=fs.readFileSync(file,'utf8').replace(/data-video="\/showcase\/videos\/(F\d{3})\.mp4"(?: data-webm="[^"]+")?/g,
  (_,id)=>'data-video="/showcase/videos/'+id+'.mp4" data-webm="/motion-clips/'+id+'.webm"');
 fs.writeFileSync(file,html);
}
console.log('Built six seekable WebM clips offline ('+report.reduce((n,r)=>n+r.bytes,0)+' bytes); original recordings unchanged.');
