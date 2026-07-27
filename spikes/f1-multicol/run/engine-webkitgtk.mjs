// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// WebKit engine driver, over raw W3C WebDriver against WebKitWebDriver.
//
// This one is not a stand-in: webkit2gtk-4.1 *is* the webview Tauri uses on
// Linux, so these numbers are a shipping target measured directly. It is also
// the closest available proxy for WKWebView on macOS and iOS — same WebCore
// layout and fragmentation code, different platform integration and different
// text rasterisation. Close, not identical; the report says so.
//
// No client library: the W3C protocol is a dozen HTTP calls and a dependency
// here would be a dependency to audit.

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';

export const family = 'WebKit';

const MINIBROWSER_CANDIDATES = [
  '/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser',
  '/usr/lib/aarch64-linux-gnu/webkit2gtk-4.1/MiniBrowser',
  '/usr/lib/webkit2gtk-4.1/MiniBrowser',
];

const sleep = (ms) => new Promise((r) => { setTimeout(r, ms); });

async function rpc(method, url, body) {
  const res = await fetch(url, {
    method,
    headers: body ? { 'content-type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok || json.value?.error) {
    const err = json.value?.error ?? `http ${res.status}`;
    const msg = json.value?.message ?? '';
    throw new Error(`webdriver ${method} ${url}: ${err} ${msg}`.trim());
  }
  return json.value;
}

export function findMiniBrowser() {
  return MINIBROWSER_CANDIDATES.find((p) => existsSync(p)) ?? null;
}

export async function launch({ windowWidth, windowHeight, display = ':99', port = 8194 }) {
  const binary = findMiniBrowser();
  if (!binary) throw new Error('MiniBrowser not found; install webkit2gtk-driver');

  // WebKitGTK has no headless mode, so it gets a virtual display. Xvfb is left
  // running if it was already up (a human debugging the harness wants the screen).
  let xvfb = null;
  if (!process.env.F1_REUSE_DISPLAY) {
    xvfb = spawn('Xvfb', [display, '-screen', '0', `${windowWidth}x${windowHeight}x24`],
      { stdio: 'ignore' });
    await sleep(1500);
  }

  const driver = spawn('WebKitWebDriver', [`--port=${port}`, '--host=127.0.0.1'], {
    env: { ...process.env, DISPLAY: display },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const driverLog = [];
  driver.stdout.on('data', (d) => driverLog.push(String(d)));
  driver.stderr.on('data', (d) => driverLog.push(String(d)));
  await sleep(1500);

  const base = `http://127.0.0.1:${port}/session`;
  const created = await rpc('POST', base, {
    capabilities: {
      alwaysMatch: {
        browserName: 'MiniBrowser',
        'webkitgtk:browserOptions': { binary, args: ['--automation'] },
      },
    },
  });
  const sid = created.sessionId;
  const sess = `${base}/${sid}`;

  await rpc('POST', `${sess}/timeouts`, { script: 120000, pageLoad: 120000 });
  try {
    await rpc('POST', `${sess}/window/rect`,
      { x: 0, y: 0, width: windowWidth, height: windowHeight });
  } catch {
    // Window managers under Xvfb are optional; the iframe box drives geometry
    // anyway, so a refused resize is not fatal.
  }

  const ua = await rpc('POST', `${sess}/execute/sync`,
    { script: 'return navigator.userAgent', args: [] });

  return {
    name: 'webkitgtk',
    family,
    version: String(ua),
    consoleErrors: driverLog,

    async goto(url) {
      await rpc('POST', `${sess}/url`, { url });
    },

    async evalAsync(fnSource, arg) {
      const script = `
        var arg = arguments[0];
        var done = arguments[arguments.length - 1];
        try {
          Promise.resolve((${fnSource})(arg)).then(
            function (v) { done({ ok: true, value: v }); },
            function (e) { done({ ok: false, error: String((e && e.message) || e) }); });
        } catch (e) { done({ ok: false, error: String((e && e.message) || e) }); }`;
      const out = await rpc('POST', `${sess}/execute/async`, { script, args: [arg ?? null] });
      if (!out || out.ok !== true) throw new Error(out?.error ?? 'script failed');
      return out.value;
    },

    async drag(from, to, steps = 24) {
      const moves = [];
      for (let i = 1; i <= steps; i += 1) {
        moves.push({
          type: 'pointerMove',
          duration: 8,
          origin: 'viewport',
          x: Math.round(from.x + ((to.x - from.x) * i) / steps),
          y: Math.round(from.y + ((to.y - from.y) * i) / steps),
        });
      }
      await rpc('POST', `${sess}/actions`, {
        actions: [{
          type: 'pointer',
          id: 'mouse',
          parameters: { pointerType: 'mouse' },
          actions: [
            { type: 'pointerMove', duration: 0, origin: 'viewport', x: Math.round(from.x), y: Math.round(from.y) },
            { type: 'pointerDown', button: 0 },
            ...moves,
            { type: 'pointerUp', button: 0 },
          ],
        }],
      });
      await rpc('DELETE', `${sess}/actions`).catch(() => {});
    },

    async close() {
      await fetch(sess, { method: 'DELETE' }).catch(() => {});
      driver.kill();
      if (xvfb) xvfb.kill();
    },
  };
}
