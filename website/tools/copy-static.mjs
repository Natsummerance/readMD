import { cp, mkdir, readdir, unlink, link, stat } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const output = new URL("dist/", root);

await mkdir(output, { recursive: true });
await cp(new URL("public/", root), output, { recursive: true, filter: async (source, target) => {
  try {
    const [a, b] = await Promise.all([stat(source), stat(target)]);
    return !a.isFile() || a.dev !== b.dev || a.ino !== b.ino;
  } catch { return true; }
} });
for (const dir of ['videos','posters','captions']) {
 const source=new URL(`public/showcase/${dir}/`,root),dest=new URL(`dist/showcase/${dir}/`,root);
 for(const name of await readdir(source)) {
  // The canonical showcase and the publishable build share immutable media
  // on one volume; a deployment archive still contains ordinary files.
  try {await unlink(new URL(name,dest));await link(new URL(name,source),new URL(name,dest));}
  catch {await cp(new URL(name,source),new URL(name,dest));}
 }
}

// Feature demonstrations load on demand; no animation framework is required.
