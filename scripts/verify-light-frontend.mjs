import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const assets = resolve("dist/assets");
const files = readdirSync(assets);
if (files.some((file) => file.startsWith("instrument-chord-corpus-"))) {
  throw new Error("the instrument chord corpus leaked into the Light frontend");
}
for (const file of files.filter((entry) => entry.endsWith(".js"))) {
  const source = readFileSync(join(assets, file), "utf8");
  if (source.includes("verified-chord-fingering-corpus")) {
    throw new Error(`instrument voicings leaked into the Light frontend: ${file}`);
  }
}
console.log("Verified Light frontend without Piano, Guitar, Ukulele, or their chord corpus");
