/* 浅色主题：逐个页面回归截图。 */
import { chromium } from "playwright-core";

const THEME = process.argv[2] || "light";
const TAG = process.argv[3] || "pages";
const OUT = "G:/Mine/一些自制工具/Fontora/app/.preview/out/";
const PAGES = ["关于", "备份与迁移", "自动归档", "回收站"];

const browser = await chromium.launch({
  executablePath: "C:/Program Files/Google/Chrome/Application/chrome.exe",
  headless: true,
});
const ctx = await browser.newContext({ viewport: { width: 1440, height: 940 }, deviceScaleFactor: 1 });
const page = await ctx.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push(String(e).slice(0, 300)));

await page.goto(`http://127.0.0.1:8899/mock.html?theme=${THEME}`, { waitUntil: "networkidle" });
await page.waitForTimeout(2200);

let i = 0;
for (const name of PAGES) {
  const row = page.locator(".nav-row", { hasText: name }).first();
  try {
    await row.click({ timeout: 5000 });
  } catch {
    console.log("skip", name);
    continue;
  }
  await page.waitForTimeout(900);
  await page.screenshot({ path: `${OUT}${TAG}-page${i++}.png` });
}

console.log("errors:", errs.length ? errs : "none");
await browser.close();
