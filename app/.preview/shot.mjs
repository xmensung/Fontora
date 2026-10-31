import { chromium } from 'playwright-core';
const browser = await chromium.launch({
  executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe',
  headless: true,
});
const page = await browser.newPage({ viewport: { width: 1440, height: 940 }, deviceScaleFactor: 1 });
await page.goto('file:///D:/WorkBuddy/2026-10-19-10-00-09/app/.preview/detail.html');
await page.waitForTimeout(600);
await page.screenshot({ path: '.preview/detail-light.png' });
await page.evaluate(() => document.documentElement.setAttribute('data-theme', 'dark'));
await page.waitForTimeout(300);
await page.screenshot({ path: '.preview/detail-dark.png' });
await browser.close();
console.log('ok');
