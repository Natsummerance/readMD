(() => {
 'use strict';
 const $=id=>document.getElementById(id);
 let features=[],category='',lastTrigger=null;
 let saved;try{saved=localStorage.getItem('readmd.site.theme');}catch{}
 document.documentElement.classList.toggle('dark',saved?saved==='dark':matchMedia('(prefers-color-scheme: dark)').matches);
 $('theme').addEventListener('click',()=>{const dark=document.documentElement.classList.toggle('dark');try{localStorage.setItem('readmd.site.theme',dark?'dark':'light');}catch{}});
 const close=()=>{$('video').pause();$('video').removeAttribute('src');$('video').load();$('player').close();lastTrigger?.focus();};
 $('close-player').addEventListener('click',close);
 $('player').addEventListener('cancel',e=>{e.preventDefault();close();});
 $('player').addEventListener('click',e=>{if(e.target===$('player')){const r=$('player').getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)close();}});
 const open=(f,trigger)=>{
  lastTrigger=trigger;$('player-id').textContent=f.id+' · '+f.recording.duration.toFixed(1)+' 秒';$('player-title').textContent=f.title;$('flow').textContent=f.flow;
  $('steps').replaceChildren(...f.recording.steps.map(s=>{const li=document.createElement('li');li.textContent=s.text;return li;}));
  const video=$('video');video.poster='/showcase/'+f.recording.poster;video.src='/showcase/'+f.recording.video;video.querySelector('track').src='/showcase/'+f.recording.captions;
  // Direct links open without a user gesture. Load the selected clip even
  // when the browser declines autoplay; the controls must still be ready.
  video.preload='auto';video.querySelector('track').track.mode='hidden';video.load();
  $('download-video').href=video.src;$('player').showModal();video.play().catch(()=>{});
  history.replaceState(null,'','#'+f.id);
 };
 const render=()=>{
  const query=$('search').value.trim().toLocaleLowerCase();
  const shown=features.filter(f=>(!category||f.section===category)&&(!query||[f.id,f.title,f.section,f.flow].join(' ').toLocaleLowerCase().includes(query)));
  $('category-title').textContent=category?category.replace(/^\d+\s*/,''):'全部功能';$('count').textContent=shown.length+' 项功能';$('empty').hidden=!!shown.length;
  $('cards').replaceChildren(...shown.map(f=>{
   if(f.recording.status!=='recorded'){const article=document.createElement('article');article.className='card pending';const id=document.createElement('small');id.textContent=f.id;const h=document.createElement('h3');h.textContent=f.title;const p=document.createElement('p');p.textContent='演示录制中';article.append(id,h,p);return article;}
   const b=document.createElement('button');b.type='button';b.className='card';b.setAttribute('aria-label','播放 '+f.title);const image=document.createElement('img');image.src='/showcase/'+f.recording.poster;image.alt='';image.loading='lazy';image.width=1280;image.height=800;const play=document.createElement('span');play.className='play';play.textContent='▶';play.setAttribute('aria-hidden','true');const body=document.createElement('div');body.className='card-body';const meta=document.createElement('div');meta.className='card-meta';const id=document.createElement('span');id.textContent=f.id;const time=document.createElement('span');time.textContent=f.recording.duration.toFixed(1)+' 秒';meta.append(id,time);const h=document.createElement('h3');h.textContent=f.title;body.append(meta,h);b.append(image,play,body);b.addEventListener('click',()=>open(f,b));return b;
  }));
  for(const b of $('categories').children)b.setAttribute('aria-pressed',String(b.dataset.category===category));
 };
 $('search').addEventListener('input',render);
 fetch('/showcase/catalog.json').then(r=>{if(!r.ok)throw Error('功能清单加载失败');return r.json();}).then(c=>{
  features=c.features;const categories=['',...new Set(features.map(f=>f.section))];
  $('categories').replaceChildren(...categories.map(name=>{const b=document.createElement('button');b.className='category';b.type='button';b.dataset.category=name;const text=document.createElement('div');text.textContent=name?name.replace(/^\d+\s*/,''):'全部功能';const count=document.createElement('span');count.textContent=features.filter(f=>!name||f.section===name).length;b.append(text,count);b.addEventListener('click',()=>{category=name;render();});return b;}));render();
  const f=features.find(f=>'#'+f.id===location.hash&&f.recording.status==='recorded');if(f)open(f,null);
 }).catch(e=>{$('empty').hidden=false;$('empty').textContent=e.message+'，请刷新重试。';});
})();
