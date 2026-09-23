// Headless smoke test of the built example apps (`npm run build` first): each app's
// <sinua-view> draws, fires fxframe, reacts to its bound `inputs` property, stops on
// removal, and logs no console errors. No audio (muted, no voice source).
import { preview } from "vite";
import { chromium } from "playwright";

const apps = ["html", "vue", "svelte", "solid", "angular"];
// A local Chrome for Testing can be used via SINUA_CHROME; otherwise Playwright's full Chromium (new headless).
const browser = await chromium.launch({ headless: true, args: ["--mute-audio"], ...(process.env.SINUA_CHROME ? { executablePath: process.env.SINUA_CHROME } : { channel: "chromium" }) });
let failed = 0;
for (const app of apps) {
  const server = await preview({ root: new URL(`./${app}/`, import.meta.url).pathname, preview: { port: 0, host: "127.0.0.1" }, logLevel: "error" });
  const url = server.resolvedUrls.local[0];
  const page = await browser.newPage();
  const errors = [];
  page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto(url);
  await page.waitForTimeout(2500);
  const fx = page.locator("sinua-view");
  const ink = await fx.evaluate((el) => { const c = el.shadowRoot.querySelector("canvas"); const d = c.getContext("2d").getImageData(0, 0, c.width, c.height).data; let n = 0; for (let i = 3; i < d.length; i += 4) if (d[i] > 10) n++; return n; });
  const frames = async () => Number(await page.locator("#frames").innerText());
  const f1 = await frames(); await page.waitForTimeout(600); const f2 = await frames();
  const shot = () => fx.evaluate((el) => el.shadowRoot.querySelector("canvas").toDataURL());
  await fx.evaluate((el) => el.setAttribute("paused", "")); await page.waitForTimeout(300);
  const before = await shot(); await page.click("#toggle"); await page.waitForTimeout(500);
  const inputsChange = before !== (await shot());
  await fx.evaluate((el) => el.removeAttribute("paused"));
  await page.click("#remove"); await page.waitForTimeout(300);
  const g1 = await frames(); await page.waitForTimeout(600); const g2 = await frames();
  const ok = ink > 500 && f2 > f1 && inputsChange && g1 === g2 && errors.length === 0;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${app.padEnd(8)} ink ${ink}, frames ${f1}→${f2}, inputs change ${inputsChange}, stops on remove ${g1 === g2}, errors ${errors.length ? errors.join(" | ") : "none"}`);
  await page.close();
  await new Promise((r) => server.httpServer.close(r));
}
await browser.close();
console.log(failed ? `${failed} app(s) failed` : "all example apps pass");
process.exit(failed ? 1 : 0);
