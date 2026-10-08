'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http');
exports.start=async root=>{
 root=path.resolve(root);
 const types={'.html':'text/html','.css':'text/css','.js':'text/javascript','.json':'application/json','.xml':'application/xml','.webp':'image/webp','.png':'image/png','.mp4':'video/mp4','.webm':'video/webm','.vtt':'text/vtt'};
 const server=http.createServer((request,response)=>{
  let file;
  try{file=path.resolve(root,'.'+decodeURIComponent(new URL(request.url,'http://local').pathname));}catch{response.writeHead(400).end();return;}
  if(file!==root&&!file.startsWith(root+path.sep)){response.writeHead(403).end();return;}
  if(fs.statSync(file,{throwIfNoEntry:false})?.isDirectory())file=path.join(file,'index.html');
  const stat=fs.statSync(file,{throwIfNoEntry:false});
  if(!stat?.isFile()){response.writeHead(404).end();return;}
  let start=0,end=stat.size-1,status=200;
  const headers={'Content-Type':types[path.extname(file)]||'application/octet-stream','Accept-Ranges':'bytes',
   'Content-Security-Policy':"default-src 'self'; img-src 'self'; style-src 'self'; script-src 'self'; object-src 'none'; base-uri 'self'"};
  if(request.headers.range){
   const match=request.headers.range.match(/^bytes=(\d*)-(\d*)$/);
   if(!match||(!match[1]&&!match[2])){response.writeHead(416).end();return;}
   if(match[1]){start=Number(match[1]);if(match[2])end=Math.min(end,Number(match[2]));}
   else start=Math.max(0,stat.size-Number(match[2]));
   if(start>end||start>=stat.size){response.writeHead(416,{'Content-Range':'bytes */'+stat.size}).end();return;}
   status=206;headers['Content-Range']='bytes '+start+'-'+end+'/'+stat.size;
  }
  headers['Content-Length']=Math.max(0,end-start+1);response.writeHead(status,headers);
  if(request.method==='HEAD'||!stat.size){response.end();return;}
  const stream=fs.createReadStream(file,{start,end});stream.on('error',()=>response.destroy());stream.pipe(response);
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 return {server,url:'http://127.0.0.1:'+server.address().port};
};
