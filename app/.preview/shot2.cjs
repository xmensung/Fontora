const { chromium } = require('C:/Users/Hex/.workbuddy/binaries/node/workspace/node_modules/playwright-core');
(async () => {
  const browser = await chromium.launch({
    executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe',
    headless: true,
  });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  await page.goto('file:///D:/WorkBuddy/2026-10-19-10-00-09/app/.preview/dupe.html');
  await page.waitForTimeout(400);
  await page.screenshot({ path: '.preview/dupe-light.png' });
  await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'dark'));
  await page.waitForTimeout(250);
  await page.screenshot({ path: '.preview/dupe-dark.png' });
  await browser.close();
  console.log('ok');
})();
