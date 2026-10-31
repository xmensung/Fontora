/* 浅色主题：多视图回归截图（列表视图 / 命令面板 / 字体详情 / 右键菜单）。 */
import { chromium } from "playwright-core";

const THEME = process.argv[2] || "light";
const TAG = process.argv[3] || "views";
const OUT = "G:/Mine/一些自制工具/Fontora/app/.preview/out/";

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

// 列表视图
await page.click('[role="tablist"] [role="tab"]:nth-child(2)');
await page.waitForTimeout(900);
await page.screenshot({ path: `${OUT}${TAG}-list.png` });

// 回到网格，打开右键菜单
await page.click('[role="tablist"] [role="tab"]:nth-child(1)');
await page.waitForTimeout(700);
const card = page.locator(".family-card").first();
await card.click({ button: "right" });
await page.waitForTimeout(600);
await page.screenshot({ path: `${OUT}${TAG}-ctxmenu.png` });
await page.keyboard.press("Escape");
await page.waitForTimeout(300);

// 命令面板
await page.keyboard.press("Control+k");
await page.waitForTimeout(800);
await page.screenshot({ path: `${OUT}${TAG}-palette.png` });
await page.keyboard.press("Escape");
await page.waitForTimeout(300);

// 字体详情（双击卡片）
await card.dblclick();
await page.waitForTimeout(1200);
await page.screenshot({ path: `${OUT}${TAG}-detail.png` });

console.log("errors:", errs.length ? errs : "none");
await browser.close();
