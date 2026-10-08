import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { packageNames, publishedRelease, releaseLink, updateReleaseHtml } from '../public/assets/release-contract.mjs';
const snapshot = JSON.parse(fs.readFileSync(new URL('../published-release.json', import.meta.url), 'utf8'));
const fixture = (tag='V0.0.5') => ({tag_name:tag, draft:false, prerelease:false, published_at:'2026-10-08T12:00:00Z',
  assets:packageNames(tag.slice(1)).map(name=>({name,size:1024,state:'uploaded',browser_download_url:`https://github.com/Natsummerance/rust-ReadMD/releases/download/${tag}/${name}`}))});
test('exact case and every uploaded asset survive manifest round trip',()=>{
  for(const tag of ['V0.0.5','v0.0.5']){const r=publishedRelease(fixture(tag));assert.equal(r.releaseTag,tag);assert.deepEqual(publishedRelease(r),r);assert.equal(r.assets.length,11);}
});
test('draft, prerelease, incomplete, zero length, wrong owner and malformed version are rejected',()=>{
  for(const mutate of [r=>r.draft=true,r=>r.prerelease=true,r=>r.assets.pop(),r=>r.assets[0].size=0,
    r=>r.assets[0].state='new',r=>r.assets[0].browser_download_url='https://example.com/file.exe',r=>r.tag_name='V0.0.5-rc1',r=>r.version='0.0.6']){
    const r=fixture();mutate(r);assert.throws(()=>publishedRelease(r));
  }
});
test('all old downloads move to actual current packages, including versioned integrations',()=>{
  const r=publishedRelease(fixture());
  for(const name of packageNames('2.3.9')) {
    const result=releaseLink('https://github.com/Natsummerance/readMD/releases/download/v2.3.9/'+name,r);
    assert.ok(r.assets.some(a=>a.url===result),result);
  }
  assert.equal(releaseLink('https://github.com/Natsummerance/readMD/releases/download/v2.3.9/ReadMD.exe',r),r.assetsBaseUrl+'ReadMD-windows-x64.zip');
  assert.equal(releaseLink('https://github.com/Natsummerance/readMD/releases/download/v2.3.9/ReadMD-linux-arm64.AppImage',r),'https://github.com/Natsummerance/rust-ReadMD/releases/tag/V0.0.5');
  assert.equal(releaseLink('https://example.org/private',r),'https://example.org/private');
});
test('offline snapshot describes the currently published stable version, not the working VERSION',()=>{
  const config=JSON.parse(fs.readFileSync(new URL('../release.json',import.meta.url),'utf8'));
  assert.equal(publishedRelease(snapshot).version,config.stable);
});
for(const lang of ['','zh-cn/','zh-tw/','ja/']) test('localized download markup and styles survive updates: '+(lang||'en'),()=>{
  const source=fs.readFileSync(new URL('../public/'+lang+'download/index.html',import.meta.url),'utf8');
  const r=publishedRelease(fixture());const out=updateReleaseHtml(source,r,{download:true});
  assert.match(out,/<title>[^<]*V0\.0\.5/);assert.match(out,/"softwareVersion": "0\.0\.5"/);
  assert.match(out,/2026-10-08<\/time>/);assert.doesNotMatch(out,/V0\.0\.4/);
  assert.deepEqual([...out.matchAll(/class="([^"]+)"/g)].map(m=>m[1]),[...source.matchAll(/class="([^"]+)"/g)].map(m=>m[1]));
  const urls=[...out.matchAll(/href="(https:\/\/github.com\/[^\"]+\/releases\/download\/[^\"]+)"/g)].map(m=>m[1]);
  assert.deepEqual(new Set(urls),new Set(r.assets.map(a=>a.url)));
  assert.equal(updateReleaseHtml(out,r,{download:true}),out);
});
test('historical release prose is preserved while old binary links are upgraded',()=>{
  const html='<html><body>v2.3.9 <a href="https://github.com/Natsummerance/readMD/releases/download/v2.3.9/ReadMD.exe">Old</a></body></html>';
  const out=updateReleaseHtml(html,publishedRelease(fixture()),{historical:true});
  assert.match(out,/v2\.3\.9/);assert.match(out,/download\/V0\.0\.5\/ReadMD-windows-x64.zip/);
});
test('stale visible title is repaired even when structured metadata was already updated',()=>{
  const html='<html><head><title>Download ReadMD | v2.3.9</title><script type="application/ld+json">{"@type":"SoftwareApplication","softwareVersion":"0.0.4"}</script></head><body>Stable · v2.3.9</body></html>';
  const out=updateReleaseHtml(html,publishedRelease(fixture()),{download:true});
  assert.match(out,/<title>Download ReadMD \| V0\.0\.5<\/title>/);assert.doesNotMatch(out,/2\.3\.9/);
});
