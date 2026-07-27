// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// WKWebView coverage for Spike F1b (R21). macOS only.
//
// `safaridriver` ships with macOS and speaks W3C WebDriver, so this adapter is
// the same protocol client the WebKitGTK one uses — only the launch differs.
//
// What this measures, precisely. Safari and WKWebView on the same macOS build
// share the system WebKit: the same WebCore layout and fragmentation code, the
// same CoreText rasterisation, the same font stack. That is the whole surface
// F1 exercises, so for pagination, position, and selection this is WKWebView's
// engine, not an approximation of it.
//
// What it does not measure: process configuration and embedding. A Tauri
// WKWebView is instantiated by the app with its own `WKWebViewConfiguration`,
// preferences, and process pool; Safari is not. Nothing F1 measures is known to
// depend on that, but "known to" is doing work in that sentence — a result here
// that disagrees with WebKitGTK should be re-checked in a real Tauri shell
// before it is believed.
//
// Setup, once per machine:
//
//     safaridriver --enable          # requires admin; prompts for a password
//     # Safari > Settings > Advanced > Show features for web developers
//     # Safari > Develop > Allow Remote Automation
//
// Then, from spikes/f1-multicol:
//
//     node run/run.mjs --engine=safaridriver
//     node run/run.mjs --engine=chromium,safaridriver     # both, one report

import { spawn } from 'node:child_process';
import { configureSession, rpc, sessionDriver, waitForDriver } from './webdriver.mjs';

export const family = 'WebKit';

export async function launch({ windowWidth, windowHeight, port = 8195 }) {
  if (process.platform !== 'darwin') {
    throw new Error('safaridriver is macOS-only; this is the R21 / Spike F1b path');
  }

  const driver = spawn('safaridriver', [`--port=${port}`], { stdio: ['ignore', 'pipe', 'pipe'] });
  const driverLog = [];
  driver.stdout.on('data', (d) => driverLog.push(String(d)));
  driver.stderr.on('data', (d) => driverLog.push(String(d)));
  driver.on('error', (e) => driverLog.push(`spawn failed: ${e.message}`));

  try {
    await waitForDriver(port);
  } catch (err) {
    throw new Error(`${err.message}\nRun \`safaridriver --enable\` and enable `
      + `Develop > Allow Remote Automation.\n${driverLog.join('')}`);
  }

  const base = `http://127.0.0.1:${port}/session`;
  // safaridriver permits exactly one session at a time and rejects unknown
  // vendor capabilities, so this stays deliberately bare.
  const created = await rpc('POST', base, {
    capabilities: { alwaysMatch: { browserName: 'safari' } },
  });
  const sess = `${base}/${created.sessionId}`;
  const ua = await configureSession(sess, { windowWidth, windowHeight });

  return sessionDriver({
    sess,
    name: 'safaridriver',
    family,
    version: String(ua),
    log: driverLog,
    teardown: async () => { driver.kill(); },
  });
}
