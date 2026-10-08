'use strict';
// Real Chromium/WebKit decoding and scroll control. All requests stay local.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const {start}=require('./lib/serve-static.cjs');
function playwright(){for(const name of ['playwright','../../ui-tests/node_modules/playwright']){try{return require(name);}catch{}}throw Error('Use the prepared Playwright installation');}
const {chromium,webkit}=playwright();
const output=process.env.READMD_WEBSITE_MOTION_REPORT&&path.resolve(process.env.READMD_WEBSITE_MOTION_REPORT);
async function until(page,predicate,arg){const end=Date.now()+15000;while(Date.now()<end){if(await page.evaluate(predicate,arg))return;await page.waitForTimeout(80);}const state=await page.evaluate(()=>({progress:document.querySelector('[data-readmd-cinema]')?.dataset.motionProgress,stage:{...document.querySelector('.journey-stage')?.dataset},videos:[...document.querySelectorAll('.journey-stage video')].map(v=>({src:v.currentSrc,time:v.currentTime,ready:v.readyState,seeking:v.seeking,error:v.error?.message}))}));throw Error('Scroll video did not settle: '+JSON.stringify(state));}
const settle=async(page,p)=>{
 await page.evaluate(p=>{const j=document.querySelector('[data-readmd-cinema]');const top=j.getBoundingClientRect().top+scrollY;const range=j.offsetHeight-innerHeight+56;scrollTo({top:top-56+range*p,behavior:'instant'});},p);
 const index=Math.min(5,Math.floor(p*6));
 await until(page,({p,index})=>{
  const j=document.querySelector('[data-readmd-cinema]'),s=j.querySelector('.journey-stage'),v=s.querySelector('video.cinema-visible');
  return Math.abs(Number(j.dataset.motionProgress)-p)<.004&&Number(s.dataset.clipIndex)===index&&s.dataset.mediaReady==='true'&&v?.readyState>=2&&!v.seeking&&Math.abs(v.currentTime-Number(s.dataset.targetTime))<.1;
 },{p,index});
 const state=await page.evaluate(()=>{
  const j=document.querySelector('[data-readmd-cinema]'),s=j.querySelector('.journey-stage'),v=s.querySelector('video.cinema-visible');
  const box=s.getBoundingClientRect(),caption=j.querySelector('.journey-caption.is-active').getBoundingClientRect();
  return {time:v.currentTime,target:Number(s.dataset.targetTime),videos:s.querySelectorAll('video').length,paused:v.paused,muted:v.muted,
   active:j.querySelectorAll('.journey-caption:not([inert])').length,transform:getComputedStyle(s).transform,
   bounded:box.left>=-1&&box.right<=innerWidth+1&&box.top>=54&&box.bottom<=innerHeight+1,
   separate:innerWidth>=1024?caption.right<box.left:caption.bottom<box.top+1,
   viewport:[innerWidth,innerHeight],box:box.toJSON(),caption:caption.toJSON()};
 });
 assert.ok(state.bounded,'Sticky video must stay inside the viewport: '+JSON.stringify(state));assert.ok(state.separate,'Caption must not overlap the recording: '+JSON.stringify(state));
 assert.ok(state.videos<=2);assert.equal(state.active,1);assert.equal(state.paused,true);assert.equal(state.muted,true);assert.notEqual(state.transform,'none');return state;
};
(async()=>{
 const local=await start(path.join(__dirname,'../dist')),report={passed:false,cases:[],pageErrors:[],frameTiming:null};
 try{
  const engines=[['chromium',chromium],...(process.env.READMD_MOTION_WEBKIT==='1'?[['webkit',webkit]]:[])];
  for(const [engineName,engine] of engines){
   const browser=await engine.launch({headless:true,...(engineName==='chromium'&&process.env.READMD_MOTION_CHANNEL?{channel:process.env.READMD_MOTION_CHANNEL}:{})});
   try{
    const page=await browser.newPage({viewport:{width:1440,height:900},colorScheme:'light'});
    page.on('pageerror',e=>report.pageErrors.push(e.message));
    for(const prefix of engineName==='chromium'?['','zh-cn/','zh-tw/','ja/']:['zh-cn/']){
     console.log('Verify scroll story: '+engineName+' /'+prefix);
     await page.goto(local.url+'/'+prefix);await page.waitForSelector('html.motion-ready');
     assert.ok(await page.locator('main h1.motion-item').count());
     assert.equal(await page.locator('main a:not(.motion-item):not(.journey-caption a)').count(),0,'Every ordinary link participates in component motion');
     for(const progress of [.04,.23,.41,.59,.77,.96]){
      const evidence=await settle(page,progress);report.cases.push({engine:engineName,prefix,progress,...evidence});
      if(output&&prefix==='zh-cn/'&&engineName==='chromium'&&progress===.23){fs.mkdirSync(path.dirname(output),{recursive:true});await page.screenshot({path:path.join(path.dirname(output),'scroll-story.png')});}
     }
     if(prefix==='zh-cn/')for(const progress of [.77,.41,1/6,.04]){
      report.cases.push({engine:engineName,prefix,reverse:true,progress,...await settle(page,progress)});
     }
    }
    if(engineName==='chromium'){
     for(const [width,height] of [[320,568],[390,740],[1024,680]])for(const prefix of ['','zh-cn/','zh-tw/','ja/']){
      await page.setViewportSize({width,height});await page.goto(local.url+'/'+prefix);await settle(page,.41);
      assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),'No page overflow');
      report.cases.push({engine:engineName,prefix,width,height,layout:true});
     }
     await page.setViewportSize({width:1440,height:900});await page.goto(local.url+'/zh-cn/');await settle(page,.23);
     await page.locator('#home-theme').click();assert.equal(await page.getAttribute('html','data-site-theme'),'dark');await settle(page,.59);
     if(output)await page.screenshot({path:path.join(path.dirname(output),'scroll-story-dark.png')});
     report.frameTiming=await page.evaluate(()=>new Promise(resolve=>{
      const samples=[];let last=0,n=0;const j=document.querySelector('[data-readmd-cinema]'),top=j.getBoundingClientRect().top+scrollY,range=j.offsetHeight-innerHeight+56;
      const frame=t=>{if(last)samples.push(t-last);last=t;scrollTo({top:top-56+range*(.1+n/140),behavior:'instant'});if(++n<90)requestAnimationFrame(frame);else{samples.sort((a,b)=>a-b);resolve({frames:samples.length,p95ms:samples[Math.floor(samples.length*.95)],maxMs:samples.at(-1)});}};requestAnimationFrame(frame);
     }));
     assert.ok(report.frameTiming.p95ms<150,'Scroll animation must remain responsive under repeated seeking');
    }
    const reduced=await browser.newPage({viewport:{width:390,height:740},reducedMotion:'reduce'});
    let videosRequested=0;reduced.on('request',r=>{if(/\.(mp4|webm)$/.test(r.url()))videosRequested++;});
    await reduced.goto(local.url+'/zh-cn/');await reduced.waitForTimeout(300);
    assert.equal(await reduced.locator('html.motion-ready').count(),0);assert.equal(videosRequested,0);
    assert.equal(await reduced.locator('.journey-caption[inert]').count(),0);report.cases.push({engine:engineName,reducedMotion:true});
    const fallback=await browser.newPage({viewport:{width:390,height:740}});
    await fallback.route(/\.(mp4|webm)$/,route=>route.abort());
    await fallback.goto(local.url+'/zh-cn/');await fallback.evaluate(()=>document.querySelector('[data-readmd-cinema]').scrollIntoView());await fallback.waitForTimeout(700);
    assert.equal(await fallback.locator('.journey-stage video.cinema-decoded').count(),0);
    assert.ok(await fallback.locator('.journey-stage>img').evaluate(img=>img.complete&&img.naturalWidth>0));
    assert.ok(await fallback.locator('.journey-caption.is-active a').isVisible());report.cases.push({engine:engineName,mediaFailureFallback:true});await fallback.close();
    const plain=await browser.newPage({viewport:{width:390,height:740},javaScriptEnabled:false});
    await plain.goto(local.url+'/zh-cn/');assert.equal(await plain.locator('.journey-caption[inert]').count(),0);
    for(const caption of await plain.locator('.journey-caption').all())assert.ok(await caption.isVisible());
    assert.ok(await plain.evaluate(()=>document.documentElement.scrollWidth<=innerWidth));
    report.cases.push({engine:engineName,noJavaScript:true});await plain.close();
    if(process.env.READMD_MOTION_TEST_MP4==='1'&&engineName==='chromium'){
     const mp4=await browser.newPage({viewport:{width:1440,height:900}});await mp4.route(/\.webm$/,r=>r.abort());await mp4.goto(local.url+'/zh-cn/');
     await settle(mp4,.04);assert.ok(await mp4.locator('video.cinema-visible').evaluate(v=>v.currentSrc.endsWith('.mp4')));
     report.cases.push({engine:engineName,mp4SourceFallback:true});await mp4.close();
    }
    await reduced.close();await page.close();
   }finally{await browser.close();}
  }
  assert.deepEqual(report.pageErrors,[]);report.passed=true;
  if(output){fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify(report,null,2));}
  console.log(JSON.stringify({passed:true,cases:report.cases.length,frameTiming:report.frameTiming,pageErrors:report.pageErrors}));
 }finally{local.server.close();}
})().catch(e=>{console.error(e.stack);process.exitCode=1;});
