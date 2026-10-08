const fs=require('fs'),path=require('path'),assert=require('assert/strict'),{chromium}=require('playwright'),{start}=require('./lib/serve-static.cjs');
async function until(page,fn){const end=Date.now()+20000;while(Date.now()<end){if(await page.evaluate(fn))return;await page.waitForTimeout(80);}throw Error('Website state timeout');}
(async()=>{const local=await start(path.resolve(__dirname,'../dist')),browser=await chromium.launch({headless:true,channel:'msedge'}),report={passed:false,cases:[],errors:[]};
try{
 for(const prefix of ['','zh-cn/','zh-tw/','ja/'])for(const scheme of ['light','dark']){
  const page=await browser.newPage({viewport:{width:1280,height:800},colorScheme:scheme});page.on('pageerror',e=>report.errors.push(e.message));page.on('response',r=>{if(r.status()>=400)report.errors.push(r.status()+' '+new URL(r.url()).pathname);});
  await page.goto(local.url+'/'+prefix+'ai-autocomplete/');await until(page,()=>document.querySelector('main h1').classList.contains('motion-item'));
  assert.ok((await page.locator('body').textContent()).includes('V0.0.5'));
  await page.locator('video').scrollIntoViewIfNeeded();await page.locator('video').evaluate(v=>{v.preload='auto';v.load();});
  await until(page,()=>document.querySelector('video').readyState>=2);
  const media=await page.locator('video').evaluate(v=>({width:v.videoWidth,height:v.videoHeight,duration:v.duration}));assert.equal(media.width,3840);assert.equal(media.height,2160);assert.ok(media.duration>5);
  await page.locator('video').evaluate(async v=>{v.muted=true;await v.play();});await page.waitForTimeout(150);assert.ok(await page.locator('video').evaluate(v=>v.currentTime>0));
  await page.setViewportSize({width:390,height:844});await page.emulateMedia({reducedMotion:'reduce'});assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  assert.equal(await page.locator('article').count(),2);report.cases.push({prefix,scheme,...media,mobileOverflow:false});await page.close();
 }
 assert.deepEqual(report.errors,[]);report.passed=true;console.log(JSON.stringify(report));
 if(process.env.READMD_AI_SITE_REPORT)fs.writeFileSync(process.env.READMD_AI_SITE_REPORT,JSON.stringify(report,null,2));
}finally{await browser.close();await new Promise(r=>local.server.close(r));}
})().catch(e=>{console.error(e);process.exitCode=1;});
