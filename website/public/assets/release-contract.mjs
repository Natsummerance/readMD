export const REPOSITORY = 'Natsummerance/rust-ReadMD';
export const API_URL = `https://api.github.com/repos/${REPOSITORY}/releases/latest`;
const base = `https://github.com/${REPOSITORY}`;
export function packageNames(v) {
  return ['ReadMDSetup-windows-x64.exe', 'ReadMD-windows-x64.zip',
    'ReadMD-macos-arm64.zip', 'ReadMD-macos-x64.zip', 'ReadMD-macos-arm64.dmg', 'ReadMD-macos-x64.dmg',
    'ReadMD-linux-x86_64.tar.gz', 'ReadMD-linux-x86_64.deb',
    `readmd-vscode-${v}.vsix`, `readmd-mcp-server-${v}.zip`, 'SHA256SUMS.txt'];
}
// Validate the complete published asset set. Preserve the exact, case-sensitive tag.
export function publishedRelease(data) {
  const releaseTag = data?.tag_name ?? data?.releaseTag;
  if (!/^[vV]\d+\.\d+\.\d+$/.test(releaseTag || '') || data.draft || data.prerelease)
    throw Error('Expected a published stable release');
  const version = releaseTag.slice(1);
  if (data.version && data.version !== version) throw Error('Version differs from tag');
  const assetsBaseUrl = `${base}/releases/download/${releaseTag}/`;
  const assets = packageNames(version).map(name => {
    const asset = data.assets?.find(a => a.name === name);
    if (!asset || !(asset.size > 0) || (asset.state && asset.state !== 'uploaded') ||
      (asset.browser_download_url ?? asset.url) !== assetsBaseUrl + name)
      throw Error('Missing or invalid published package: ' + name);
    return { name, size: asset.size, url: assetsBaseUrl + name };
  });
  const updatedAt = data.published_at ?? data.updatedAt;
  if (!updatedAt || !Number.isFinite(Date.parse(updatedAt))) throw Error('Missing publication date');
  return { version, releaseTag, repository: REPOSITORY, updatedAt, assetsBaseUrl,
    checksumUrl: assetsBaseUrl + 'SHA256SUMS.txt', assets };
}
export function releaseLink(url, r) {
  const match = /^https:\/\/github\.com\/Natsummerance\/(?:rust-)?readmd\/releases\/(?:download\/[^/]+|latest\/download)\/([^?#/]+)(?:[?#].*)?$/i.exec(url);
  if (!match) {
    if (/^https:\/\/github\.com\/Natsummerance\/(?:rust-)?readmd\/tree\/main\/packages\/vscode-extension$/i.test(url))
      return r.assetsBaseUrl + `readmd-vscode-${r.version}.vsix`;
    if (/^https:\/\/github\.com\/Natsummerance\/(?:rust-)?readmd\/tree\/main\/(?:rust\/readmd-kernel\/src\/mcp(?:_server)?\.rs|packages\/mcp-server(?:\/.*)?)$/i.test(url))
      return r.assetsBaseUrl + `readmd-mcp-server-${r.version}.zip`;
    return url;
  }
  let name = match[1];
  if (/^readmd-vscode-.*\.vsix$/i.test(name)) name = `readmd-vscode-${r.version}.vsix`;
  else if (/^readmd-mcp-server(?:-.*)?\.zip$/i.test(name)) name = `readmd-mcp-server-${r.version}.zip`;
  else if (/^ReadMDSetup(?:-windows-x64)?\.exe$/i.test(name)) name = 'ReadMDSetup-windows-x64.exe';
  else if (/^ReadMD(?:-windows-x64)?\.exe$/i.test(name)) name = 'ReadMD-windows-x64.zip';
  else if (/^ReadMD-linux-x86_64\.AppImage$/i.test(name)) name = 'ReadMD-linux-x86_64.tar.gz';
  else if (/^readmd_.*_amd64\.deb$/i.test(name)) name = 'ReadMD-linux-x86_64.deb';
  return r.assets.some(a => a.name === name) ? r.assetsBaseUrl + name : `${base}/releases/tag/${r.releaseTag}`;
}
export const intro = {
  en: r => `Download ReadMD <span class="latest-version-badge" data-version-slot>${r.releaseTag}</span>, the current stable release. Published <time data-release-date datetime="${r.updatedAt}">${r.updatedAt.slice(0, 10)}</time>. Choose a package below and verify SHA256SUMS.txt before installing.`,
  'zh-CN': r => `下载 ReadMD <span class="latest-version-badge" data-version-slot>${r.releaseTag}</span> 当前正式版，发布于 <time data-release-date datetime="${r.updatedAt}">${r.updatedAt.slice(0, 10)}</time>。选择下方安装包，安装前请核对 SHA256SUMS.txt。`,
  'zh-TW': r => `下載 ReadMD <span class="latest-version-badge" data-version-slot>${r.releaseTag}</span> 目前正式版，發行於 <time data-release-date datetime="${r.updatedAt}">${r.updatedAt.slice(0, 10)}</time>。選擇下方安裝套件，安裝前請核對 SHA256SUMS.txt。`,
  ja: r => `ReadMD <span class="latest-version-badge" data-version-slot>${r.releaseTag}</span> 安定版。公開日：<time data-release-date datetime="${r.updatedAt}">${r.updatedAt.slice(0, 10)}</time>。以下のパッケージを選び、インストール前に SHA256SUMS.txt を確認してください。`,
};
export function updateReleaseHtml(html, r, { download = false, historical = false, candidate = false } = {}) {
  const previous = new Set();
  // Legacy deployments can have current JSON-LD but stale visible titles/badges.
  for (const label of html.matchAll(/<(?:title\b[^>]*|[a-z]+\b[^>]*(?:data-version-slot|class="latest-version-badge)[^>]*)>([^<]*)</gi))
    for (const tag of label[1].matchAll(/[vV](\d+\.\d+\.\d+)/g)) previous.add(tag[1]);
  html = html.replace(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/g, (original, body) => {
    let value; try { value = JSON.parse(body); } catch { return original; }
    const visit = object => {
      if (!object || typeof object !== 'object') return;
      if (object['@type'] === 'SoftwareApplication' && !candidate) {
        if (object.softwareVersion) previous.add(object.softwareVersion);
        object.softwareVersion = r.version;
        if (object.dateModified) object.dateModified = r.updatedAt.slice(0, 10);
      }
      for (const [key, item] of Object.entries(object)) {
        if (typeof item === 'string') object[key] = releaseLink(item, r); else visit(item);
      }
    };
    visit(value);
    return '<script type="application/ld+json">' + JSON.stringify(value, null, 2) + '</script>';
  });
  html = html.replace(/https:\/\/github\.com\/Natsummerance\/(?:rust-)?readmd\/[^\s"'<>]+/gi, url => releaseLink(url, r));
  if (!historical && !candidate) for (const v of previous)
    html = html.replace(new RegExp(`[vV]${v.replaceAll('.', '\\.')}(?![\\d.])`, 'g'), r.releaseTag);
  if (download) {
    const lang = /<html[^>]*lang="([^"]+)"/i.exec(html)?.[1] || 'en';
    html = html.replace(/(<p\b[^>]*>)(?:(?!<\/p>)[\s\S])*?class="latest-version-badge\b(?:(?!<\/p>)[\s\S])*?<\/p>/,
      (_, open) => open + (intro[lang] || intro.en)(r) + '</p>');
    html = html.replace(/便携免安装版 \(\.exe\)/g, '便携免安装版 (.zip)')
      .replace(/可攜免安裝版 \(\.exe\)/g, '可攜免安裝版 (.zip)')
      .replace(/(Portable[^<\n]*?)\(\.exe\)/gi, '$1(.zip)')
      .replace(/(ポータブル[^<\n]*?)\(\.exe\)/g, '$1(.zip)')
      .replace(/<code>(?:ReadMD --mcp|readmd-mcp-server\.zip)<\/code>/g, `<code>readmd-mcp-server-${r.version}.zip</code>`);
  }
  if (!html.includes('src="/assets/release.js"'))
    html = html.replace('</body>', '<script type="module" src="/assets/release.js"></script></body>');
  return html;
}
