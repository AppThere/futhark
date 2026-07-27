// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Blink engine driver. Chromium here stands in for two shipping webviews Futhark
// targets: WebView2 on Windows and Android System WebView. Both are Blink, both
// track Chromium's layout engine closely. It does not stand in for anything on
// Apple platforms — see engine-webkitgtk.mjs for that half.

import { chromium } from 'playwright';

export const family = 'Blink';

export async function launch({ windowWidth, windowHeight }) {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: windowWidth, height: windowHeight },
    deviceScaleFactor: 1,
    bypassCSP: false,
  });
  const page = await context.newPage();
  const consoleErrors = [];
  page.on('console', (m) => {
    if (m.type() === 'error') consoleErrors.push(m.text().slice(0, 300));
  });
  page.on('pageerror', (e) => consoleErrors.push('pageerror: ' + String(e.message).slice(0, 300)));

  const version = browser.version();

  return {
    name: 'chromium',
    family,
    version: `Chromium ${version}`,
    consoleErrors,

    async goto(url) {
      await page.goto(url, { waitUntil: 'load' });
    },

    async evalAsync(fnSource, arg) {
      // Playwright treats a string as an expression, not as a function to call,
      // so the invocation is written out and the argument inlined as JSON. Same
      // shape as the WebDriver path, which keeps the scenarios engine-neutral.
      return page.evaluate(`(${fnSource})(${JSON.stringify(arg ?? null)})`);
    },

    async drag(from, to, steps = 24) {
      await page.mouse.move(from.x, from.y);
      await page.mouse.down();
      for (let i = 1; i <= steps; i += 1) {
        await page.mouse.move(
          from.x + ((to.x - from.x) * i) / steps,
          from.y + ((to.y - from.y) * i) / steps,
        );
      }
      await page.mouse.up();
    },

    async close() {
      await context.close();
      await browser.close();
    },
  };
}
