// Headless check of the static export (`npm run build` first). It serves out/ the way a
// plain static host does (no rewrites: a directory URL serves its index.html,
// "/docs/x" 301s to "/docs/x/", like python -m http.server, S3 or GitHub Pages).
// Then it loads every exported page and fails on any console error or uncaught
// exception (e.g. React #418 hydration), a failed load, or a search with no results.
import { createServer } from "node:http";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { extname, join } from "node:path";
import { chromium } from "playwright";

const OUT = new URL("../out/", import.meta.url).pathname;
if (!existsSync(OUT)) throw new Error("no out/: run `npm run build` first");

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".json": "application/json", ".txt": "text/plain", ".svg": "image/svg+xml", ".woff2": "font/woff2" };
const server = createServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
  let file = join(OUT, path);
  if (existsSync(file) && statSync(file).isDirectory()) {
    if (!path.endsWith("/")) return res.writeHead(301, { Location: path + "/" }).end();
    file = join(file, "index.html");
  }
  if (!existsSync(file) || statSync(file).isDirectory()) return res.writeHead(404).end("not found");
  res.writeHead(200, { "Content-Type": TYPES[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
});
await new Promise((r) => server.listen(0, "127.0.0.1", r));
const base = `http://127.0.0.1:${server.address().port}`;

// Every exported page: out/docs/**/index.html -> /docs/**/ (and any docs/**/x.html -> its own URL)
const pages = [];
const walk = (dir, url) => {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) walk(join(dir, e.name), `${url}${e.name}/`);
    else if (e.name === "index.html") pages.push(url);
    // A page exported as x.html (no trailingSlash) is reachable only by that URL on a plain host.
    else if (e.name.endsWith(".html")) pages.push(url + e.name);
  }
};
walk(join(OUT, "docs"), "/docs/");
pages.sort();
// Pages outside the docs tree (the gallery) are checked too.
for (const extra of ["/gallery/"]) if (existsSync(join(OUT, extra, "index.html"))) pages.push(extra);

// A local Chrome for Testing via SINUA_CHROME; otherwise Playwright's Chromium (new headless).
const browser = await chromium.launch({ headless: true, ...(process.env.SINUA_CHROME ? { executablePath: process.env.SINUA_CHROME } : { channel: "chromium" }) });
const page = await browser.newPage();
let errors = [];
page.on("console", (m) => m.type() === "error" && errors.push(m.text().split("\n")[0]));
page.on("pageerror", (e) => errors.push(e.message.split("\n")[0]));
let failed = 0;
for (const p of ["/", ...pages]) {
  errors = [];
  const r = await page.goto(base + p, { waitUntil: "networkidle" });
  await page.waitForTimeout(150); // hydration errors surface after load
  const ok = r?.ok() && errors.length === 0;
  if (!ok) {
    failed++;
    console.log(`FAIL ${p} (${r?.status()}) ${[...new Set(errors)].join(" | ")}`);
  }
}
// The gallery is a site page: it has the shared nav, and a docs page's
// /gallery/?family=... link arrives filtered.
const gallery = await (async () => {
  if (!pages.includes("/gallery/")) return "skipped";
  await page.goto(base + "/gallery/", { waitUntil: "networkidle" });
  const nav = await page.locator("header.lp-top a", { hasText: "Docs" }).count();
  const home = await page.locator("header.lp-top a.lp-wordmark").count();
  await page.goto(base + "/gallery/?family=ring", { waitUntil: "networkidle" });
  await page.waitForTimeout(600);
  const tiles = await page.locator("#gallery-grid .fx-tile").count();
  const all = await page.goto(base + "/gallery/", { waitUntil: "networkidle" }).then(async () => {
    await page.waitForTimeout(400);
    return page.locator("#gallery-grid .fx-tile").count();
  });
  return nav === 1 && home === 1 && tiles > 0 && tiles < all ? "ok" : `nav ${nav}, home ${home}, family filter ${tiles}/${all}`;
})();
if (gallery !== "ok" && gallery !== "skipped") failed++;

// Static search: the index loads and a query finds a page.
const found = await (async () => {
  await page.goto(base + pages[0], { waitUntil: "networkidle" });
  await page.getByRole("button", { name: /search/i }).first().click({ timeout: 5000 });
  await page.keyboard.type("liquid");
  await page.locator('[role="dialog"]').getByText("Liquid", { exact: true }).first().waitFor({ timeout: 5000 });
  return true;
})().catch(() => false);
if (!found) failed++;
console.log(`${pages.length + 1} pages loaded, ${failed ? `${failed} problem(s)` : "no console errors"}; search ${found ? "ok" : "FAILED"}; gallery nav+filter ${gallery}`);
await browser.close();
server.close();
process.exit(failed ? 1 : 0);
