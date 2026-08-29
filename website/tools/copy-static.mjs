import { cp, mkdir } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const output = new URL("dist/", root);

await mkdir(output, { recursive: true });
await cp(new URL("public/", root), output, { recursive: true });

// Keep motion dependencies local: the published site never needs a third-party CDN
// before its interaction layer becomes available.
const gsapDist = new URL("node_modules/gsap/dist/", root);
const motionOutput = new URL("dist/assets/vendor/gsap/", root);
await mkdir(motionOutput, { recursive: true });
for (const filename of [
  "gsap.min.js",
  "ScrollTrigger.min.js",
  "ScrollToPlugin.min.js",
  "Flip.min.js",
]) {
  await cp(new URL(filename, gsapDist), new URL(filename, motionOutput));
}
