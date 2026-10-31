/* 交互验证：点掉「字体库文件夹」里的移除按钮，确认行能消失。 */
import { chromium } from "playwright-core";

const OUT = "G:/Mine/一些自制工具/Fontora/app/.preview/out/";
const browser = await chromium.launch({
  executablePath: "C:/Program Files/Google/Chrome/Application/chrome.exe",
  headless: true,
});
const ctx = await browser.newContext({ viewport: { width: 1440, height: 940 }, deviceScaleFactor: 1 });
const page = await ctx.newPage();
const errs = [];
page.on("pageerror", (e) => errs.push(String(e).slice(0, 300)));

await page.goto("http://127.0.0.1:8899/mock.html?theme=light", { waitUntil: "networkidle" });
await page.waitForTimeout(2000);
await page.click('button[aria-label="设置"]');
await page.waitForTimeout(800);

// 把「字体库文件夹」那一段滚进来
await page.evaluate(() => {
  const secs = [...document.querySelectorAll(".settings-section")];
  const t = secs.find((s) => s.querySelector(".detail-heading")?.textContent?.includes("字体库文件夹"));
  t?.scrollIntoView({ block: "center" });
});
await page.waitForTimeout(500);

const before = await page.locator(".settings-folder").count();
const rows = await page.locator(".settings-folder .settings-folder-path").allTextContents();
console.log("folders before:", before, rows);

// 关掉「使用自定义文件夹」看关掉的观感
await page.locator(".settings-section", { hasText: "字体库文件夹" }).locator(".pill-toggle").click();
await page.waitForTimeout(500);
await page.screenshot({ path: `${OUT}off-toggle.png` });

// 点最后一个 × （自定义字体库目录那一行）
await page.locator(".settings-section", { hasText: "字体库文件夹" }).locator(".settings-folder-remove").click();
await page.waitForTimeout(800);
const after = await page.locator(".settings-folder").count();
const rows2 = await page.locator(".settings-folder .settings-folder-path").allTextContents();
console.log("folders after:", after, rows2);
await page.screenshot({ path: `${OUT}after-remove.png` });

console.log("errors:", errs.length ? errs : "none");
await browser.close();
