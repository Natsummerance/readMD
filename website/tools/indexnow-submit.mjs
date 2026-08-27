import { readdir, readFile } from "node:fs/promises";

const origin = process.argv[2] || "https://readmd.asia";
const endpoint = process.argv[3] || "https://api.indexnow.org/IndexNow";
const publicRoot = new URL("../public/", import.meta.url);

const keyFile = (await readdir(publicRoot)).find(name => /^[0-9a-f]{32}\.txt$/.test(name));
if (!keyFile) throw new Error("IndexNow key file is missing");

const key = (await readFile(new URL(keyFile, publicRoot), "utf8")).trim();
if (!/^[0-9a-f]{32}$/.test(key)) throw new Error("IndexNow key must be 32 lowercase hexadecimal characters");

const sitemapResponse = await fetch(new URL("sitemap.xml", origin), { headers: { accept: "application/xml" } });
if (!sitemapResponse.ok) throw new Error(`Unable to read sitemap: HTTP ${sitemapResponse.status}`);
const sitemap = await sitemapResponse.text();
const urlList = [...new Set([...sitemap.matchAll(/<loc>(.*?)<\/loc>/g)].map(match => match[1]))];
if (!urlList.length) throw new Error("Sitemap contains no URLs");

const payload = {
  host: "readmd.asia",
  key,
  keyLocation: new URL(keyFile, origin).href,
  urlList,
};

const response = await fetch(endpoint, {
  method: "POST",
  headers: { "content-type": "application/json; charset=utf-8" },
  body: JSON.stringify(payload),
});

console.log(JSON.stringify({
  ok: response.ok,
  status: response.status,
  submittedUrls: urlList.length,
  endpoint,
  keyLocation: payload.keyLocation,
}, null, 2));

if (!response.ok) {
  const detail = await response.text();
  if (detail) console.error(detail);
  process.exitCode = 1;
}
