import pw from 'file:///C:/Users/Hex/.workbuddy/binaries/node/workspace/node_modules/playwright-core/index.mjs';
const { chromium } = pw;
const browser = await chromium.launch({
  executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe',
  headless: true,
});
for (const name of ['cmap', 'health']) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 940 }, deviceScaleFactor: 1 });
  await page.goto(`file:///D:/WorkBuddy/2026-10-19-10-00-09/app/.preview/${name}.html`);
  await page.waitForTimeout(500);
  await page.screenshot({ path: `.preview/${name}-light.png` });
  await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'dark'));
  await page.waitForTimeout(300);
  await page.screenshot({ path: `.preview/${name}-dark.png` });
  await page.close();
}
await browser.close();
console.log('ok');
