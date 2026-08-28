import { readFile, writeFile, mkdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "../../");
const websiteRoot = resolve(__dirname, "../");
const distDir = resolve(websiteRoot, "dist");

async function extractVersion() {
  // 1. Check website/.env or root/.env
  const envPaths = [
    resolve(websiteRoot, ".env"),
    resolve(projectRoot, ".env"),
    resolve(websiteRoot, ".env.local"),
    resolve(projectRoot, ".env.local")
  ];

  for (const p of envPaths) {
    if (existsSync(p)) {
      try {
        const content = await readFile(p, "utf8");
        const match = content.match(/(?:READMD_VERSION|VITE_APP_VERSION|APP_VERSION|VERSION)\s*=\s*["']?([^"'\r\n]+)["']?/);
        if (match && match[1]?.trim()) {
          const v = match[1].trim();
          console.log("[sync-version] Detected version from environment configuration");
          return v;
        }
      } catch (err) {
        console.warn("[sync-version] Could not read environment configuration:", err.message);
      }
    }
  }

  // 2. Check root/VERSION file
  const versionFilePath = resolve(projectRoot, "VERSION");
  if (existsSync(versionFilePath)) {
    try {
      const v = (await readFile(versionFilePath, "utf8")).trim();
      if (v) {
        console.log("[sync-version] Detected version from VERSION file:", v);
        return v;
      }
    } catch (err) {
      console.warn("[sync-version] Could not read VERSION file:", err.message);
    }
  }

  // 3. Fallback
  return "2.3.7";
}

async function main() {
  const version = await extractVersion();
  const releaseTag = version.startsWith("v") ? version : `v${version}`;
  const pureVersion = version.replace(/^v/, "");

  const versionData = {
    version: pureVersion,
    releaseTag: releaseTag,
    updatedAt: new Date().toISOString(),
    assetsBaseUrl: `https://github.com/Natsummerance/readMD/releases/download/${releaseTag}/`,
    checksumUrl: `https://github.com/Natsummerance/readMD/releases/download/${releaseTag}/SHA256SUMS.txt`
  };

  await mkdir(distDir, { recursive: true });
  const distVersionPath = resolve(distDir, "version.json");
  await writeFile(distVersionPath, JSON.stringify(versionData, null, 2), "utf8");
  console.log("[sync-version] Wrote dist/version.json successfully.");
}

main().catch((err) => {
  console.error("[sync-version] Error:", err);
  process.exit(1);
});
