import { cp, mkdir } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const output = new URL("dist/", root);

await mkdir(output, { recursive: true });
await cp(new URL("public/", root), output, { recursive: true });
