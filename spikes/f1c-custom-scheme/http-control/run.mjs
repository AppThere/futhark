// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Runs the F1c probe over `http` on whatever engines this machine has, and
// writes results/http-<engine>.json.
//
//   node http-control/run.mjs [--engine=chromium,webkitgtk]
//
// The engine adapters are F1's — same five methods, no reason to write them
// twice. Importing across spike directories is fine in one direction only:
// spikes may share with each other, and nothing in crates/ may touch either.

import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { start as startServers } from './serve.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const RESULTS = join(HERE, '..', 'results');

const argv = new Map(process.argv.slice(2).map((a) => {
  const [k, v] = a.replace(/^--/, '').split('=');
  return [k, v ?? 'true'];
}));
const ENGINES = String(argv.get('engine')
  ?? (process.platform === 'darwin' ? 'chromium,safaridriver' : 'chromium,webkitgtk'))
  .split(',').filter(Boolean);

const log = (m) => process.stdout.write(`${m}\n`);

function loadEngine(name) {
  return import(`../../f1-multicol/run/engine-${name}.mjs`);
}

/** Poll for the host page's result rather than racing its 6s fallback. */
const COLLECT = `async () => {
  for (let i = 0; i < 160; i += 1) {
    if (window.__F1C_RESULT) return window.__F1C_RESULT;
    await new Promise((r) => setTimeout(r, 100));
  }
  return { error: 'probe never reported' };
}`;

async function main() {
  await mkdir(RESULTS, { recursive: true });
  const servers = await startServers();
  log(`app: ${servers.appOrigin}  content: ${servers.contentOrigin}`);
  const url = `${servers.appOrigin}/host.html?transport=http`
    + `&content=${encodeURIComponent(servers.contentOrigin)}`;

  for (const name of ENGINES) {
    log(`\n=== ${name} ===`);
    let driver;
    try {
      driver = await (await loadEngine(name)).launch({ windowWidth: 1400, windowHeight: 1000 });
    } catch (err) {
      log(`  launch failed: ${err.message}`);
      continue;
    }
    try {
      await driver.goto(url);
      const result = await driver.evalAsync(COLLECT, null);
      result.engine = driver.name;
      result.engineVersion = driver.version;
      result.platform = `${process.platform}-${process.arch}`;
      await writeFile(join(RESULTS, `http-${name}.json`), JSON.stringify(result, null, 2));
      log(`  wrote results/http-${name}.json`);
    } catch (err) {
      log(`  FATAL: ${err.message}`);
    } finally {
      await driver.close().catch(() => {});
    }
  }

  servers.stop();
  log('\ndone');
}

await main();
