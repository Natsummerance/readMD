import { API_URL, publishedRelease, releaseLink } from './release-contract.mjs';
let applied;
function apply(data) {
  const release = publishedRelease(data);
  if (applied) {
    const next = release.version.split('.').map(Number), current = applied.version.split('.').map(Number);
    for (let i = 0; i < 3; i++) {
      if (next[i] < current[i]) return;
      if (next[i] > current[i]) break;
    }
  }
  const oldTags = new Set();
  for (const text of [document.title, ...[...document.querySelectorAll('.latest-version-badge,[data-version-slot]')].map(el => el.textContent)])
    for (const tag of text.matchAll(/[vV](\d+\.\d+\.\d+)/g)) oldTags.add(tag[1]);
  document.querySelectorAll('script[type="application/ld+json"]').forEach(script => {
    try {
      const value = JSON.parse(script.textContent);
      const visit = object => {
        if (!object || typeof object !== 'object') return;
        if (object['@type'] === 'SoftwareApplication') {
          oldTags.add(object.softwareVersion); object.softwareVersion = release.version;
          if (object.dateModified) object.dateModified = release.updatedAt.slice(0, 10);
        }
        for (const [key, item] of Object.entries(object)) {
          if (typeof item === 'string') object[key] = releaseLink(item, release); else visit(item);
        }
      };
      visit(value); script.textContent = JSON.stringify(value);
    } catch {}
  });
  const replaceTags = text => {
    for (const v of oldTags) text = text.replace(new RegExp('[vV]' + v.replaceAll('.', '\\.') + '(?![\\d.])', 'g'), release.releaseTag);
    return text;
  };
  if (!/\/release-notes\//.test(location.pathname)) {
    document.title = replaceTags(document.title);
    document.querySelectorAll('meta[content]').forEach(meta => meta.content = replaceTags(meta.content));
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      const node = walker.currentNode;
      if (!node.parentElement.closest('script,style,pre,code')) node.nodeValue = replaceTags(node.nodeValue);
    }
  }
  document.querySelectorAll('a[href]').forEach(link => {
    const original = link.dataset.mirrorUrl || link.href;
    const updated = releaseLink(original, release);
    if (updated !== original || updated.startsWith(release.assetsBaseUrl)) {
      link.dataset.mirrorUrl = updated; link.href = updated;
    }
  });
  document.querySelectorAll('.latest-version-badge,[data-version-slot]').forEach(el => el.textContent = release.releaseTag);
  document.querySelectorAll('[data-pure-version]').forEach(el => el.textContent = release.version);
  document.querySelectorAll('[data-release-date]').forEach(el => {
    el.dateTime = release.updatedAt; el.textContent = release.updatedAt.slice(0, 10);
  });
  applied = release; document.documentElement.dataset.publishedRelease = release.releaseTag;
  document.dispatchEvent(new CustomEvent('readmd:release', { detail: release }));
}
async function refresh() {
  // The verified same-origin snapshot remains usable when GitHub is unavailable.
  try {
    const r = await fetch('/version.json', { cache: 'no-cache', signal: AbortSignal.timeout(5000) });
    if (r.ok) apply(await r.json());
  } catch {}
  try {
    const r = await fetch(API_URL, { headers: { Accept: 'application/vnd.github+json' }, signal: AbortSignal.timeout(5000) });
    if (r.ok) apply(await r.json());
  } catch {}
  return applied;
}
window.__readmdReleaseReady = refresh();
