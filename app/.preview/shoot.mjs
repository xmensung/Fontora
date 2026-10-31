/* 打开 mock 页面并按主题截图（首页 + 设置面板）。 */
import { chromium } from "playwright-core";

const THEME = process.argv[2] || "light";
const TAG = process.argv[3] || THEME;
const OUT = "G:/Mine/一些自制工具/Fontora/app/.preview/out/";
const BASE = "http://127.0.0.1:8899/mock.html";

const browser = await chromium.launch({
  executablePath: "C:/Program Files/Google/Chrome/Application/chrome.exe",
  headless: true,
});
const ctx = await browser.newContext({
  viewport: { width: 1440, height: 940 },
  deviceScaleFactor: 1,
});
const page = await ctx.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push(String(e).slice(0, 200)));

await page.goto(`${BASE}?theme=${THEME}`, { waitUntil: "networkidle" });
await page.waitForTimeout(2500);
await page.screenshot({ path: `${OUT}${TAG}-home.png` });

// 打开设置
await page.click('button[aria-label="设置"]');
await page.waitForTimeout(900);
await page.screenshot({ path: `${OUT}${TAG}-settings.png` });

// 滚到字体库文件夹那一段
await page.evaluate(() => {
  const rows = [...document.querySelectorAll(".settings-section")];
  const target = rows.find((r) => r.textContent?.includes("字体库文件夹"));
  if (target) target.scrollIntoView({ block: "center" });
});
await page.waitForTimeout(700);
await page.screenshot({ path: `${OUT}${TAG}-settings-folders.png` });

console.log("errors:", errs.length ? errs : "none");
await browser.close();
