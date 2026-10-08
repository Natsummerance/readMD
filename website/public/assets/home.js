(() => {
 const key='readmd.site.theme';
 let saved;try{saved=localStorage.getItem(key);}catch{}
 const theme=saved||(matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light');
 document.documentElement.dataset.siteTheme=theme;
 document.querySelector('#home-theme')?.addEventListener('click',()=>{
  const value=document.documentElement.dataset.siteTheme==='dark'?'light':'dark';document.documentElement.dataset.siteTheme=value;
  try{localStorage.setItem(key,value);}catch{}
 });
 document.addEventListener('keydown',e=>{if(e.key==='Escape')document.querySelectorAll('.home-languages[open]').forEach(d=>{d.open=false;d.querySelector('summary').focus();});});
})();
