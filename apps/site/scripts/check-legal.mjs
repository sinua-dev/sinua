// The legal pages (content/legal) carry [PLACEHOLDERS] until the owner's details
// are filled in: the legal name, the address, the city for jurisdiction. A local
// build only warns; CI (CI=true) fails, so a page with a placeholder never ships.
import { readdirSync, readFileSync } from "node:fs";

const dir = new URL("../content/legal/", import.meta.url);
const found = [];
for (const f of readdirSync(dir).filter((f) => f.endsWith(".mdx"))) {
  for (const m of readFileSync(new URL(f, dir), "utf8").matchAll(/\[([A-Z][A-Z ]+)\](?!\()/g)) found.push(`${f}: [${m[1]}]`);
}
if (found.length) {
  const msg = `legal pages still have placeholders:\n  ${[...new Set(found)].join("\n  ")}`;
  if (process.env.CI) {
    console.error(msg);
    process.exit(1);
  }
  console.warn(`warning: ${msg}`);
}
