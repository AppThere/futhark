// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// WebKitGTK driver — the webview Tauri actually loads on Linux.
//
// This one is not a stand-in: webkit2gtk-4.1 is a shipping target measured
// directly. It is *not* a substitute for WKWebView (R21): same WebCore
// fragmentation code, different platform text stack, different embedding.
// WKWebView is covered by engine-safaridriver.mjs, on a Mac.

import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { configureSession, rpc, sessionDriver, waitForDriver } from './webdriver.mjs';

export const family = 'WebKit';

const MINIBROWSER_CANDIDATES = [
  '/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser',
  '/usr/lib/aarch64-linux-gnu/webkit2gtk-4.1/MiniBrowser',
  '/usr/lib/webkit2gtk-4.1/MiniBrowser',
];

const sleep = (ms) => new Promise((r) => { setTimeout(r, ms); });

export function findMiniBrowser() {
  return MINIBROWSER_CANDIDATES.find((p) => existsSync(p)) ?? null;
}

export async function launch({ windowWidth, windowHeight, display = ':99', port = 8194 }) {
  const binary = findMiniBrowser();
  if (!binary) throw new Error('MiniBrowser not found; install webkit2gtk-driver');

  // WebKitGTK has no headless mode, so it gets a virtual display. An existing
  // one is reused when F1_REUSE_DISPLAY is set — useful for watching it work.
  let xvfb = null;
  if (!process.env.F1_REUSE_DISPLAY) {
    xvfb = spawn('Xvfb', [display, '-screen', '0', `${windowWidth}x${windowHeight}x24`],
      { stdio: 'ignore' });
    await sleep(1500);
  }

  const driver = spawn('WebKitWebDriver', [`--port=${port}`, '--host=127.0.0.1'], {
    env: { ...process.env, DISPLAY: process.env.F1_REUSE_DISPLAY ? process.env.DISPLAY : display },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const driverLog = [];
  driver.stdout.on('data', (d) => driverLog.push(String(d)));
  driver.stderr.on('data', (d) => driverLog.push(String(d)));
  await waitForDriver(port);

  const base = `http://127.0.0.1:${port}/session`;
  const created = await rpc('POST', base, {
    capabilities: {
      alwaysMatch: {
        browserName: 'MiniBrowser',
        'webkitgtk:browserOptions': { binary, args: ['--automation'] },
      },
    },
  });
  const sess = `${base}/${created.sessionId}`;
  const ua = await configureSession(sess, { windowWidth, windowHeight });

  return sessionDriver({
    sess,
    name: 'webkitgtk',
    family,
    version: String(ua),
    log: driverLog,
    teardown: async () => {
      driver.kill();
      if (xvfb) xvfb.kill();
    },
  });
}
