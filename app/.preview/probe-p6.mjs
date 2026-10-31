import pw from 'file:///C:/Users/Hex/.workbuddy/binaries/node/workspace/node_modules/playwright-core/index.mjs';
const { chromium } = pw;
const browser = await chromium.launch({ executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
for (const w of [1440, 1180, 980]) {
  const page = await browser.newPage({ viewport: { width: w, height: 940 } });
  await page.goto('file:///D:/WorkBuddy/2026-10-19-10-00-09/app/.preview/cmap.html');
  await page.waitForTimeout(400);
  console.log(w, JSON.stringify(await page.evaluate(() => window.__probe)));
  await page.close();
}
await browser.close();
