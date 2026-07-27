// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Spike F1 driver.
//
//   node run/run.mjs --engine=chromium,webkitgtk [--repeats=3] [--quick]
//
// Writes results/<engine>.json and results/summary.json. Engines are plug-in:
// adding WebView2 or WKWebView later means one more engine-*.mjs exposing the
// same five methods, and the scenarios are untouched.

import { mkdir, writeFile, readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { start as startServers } from '../harness/serve.mjs';
import {
  TUPLES, probeEnvironment, layoutDoc, positionRun,
  selectionRun, dragSelectionRun, monotonicRun,
} from './scenarios.mjs';

const HERE = dirname(fileURLToPath(import.meta.url));
const RESULTS = join(HERE, '..', 'results');
const SPINE = join(HERE, '..', 'corpus', 'book', 'spine.json');

const argv = new Map(process.argv.slice(2).map((a) => {
  const [k, v] = a.replace(/^--/, '').split('=');
  return [k, v ?? 'true'];
}));

const QUICK = argv.get('quick') === 'true';
const REPEATS = Number(argv.get('repeats') ?? (QUICK ? 2 : 3));
const ENGINES = String(argv.get('engine') ?? 'chromium,webkitgtk').split(',').filter(Boolean);
const WINDOW = { windowWidth: 1600, windowHeight: 1200 };

const log = (msg) => process.stdout.write(`${msg}\n`);

async function loadEngine(name) {
  if (name === 'chromium') return import('./engine-chromium.mjs');
  if (name === 'webkitgtk') return import('./engine-webkitgtk.mjs');
  throw new Error(`unknown engine ${name}`);
}

/** Pass criterion 1: page count deterministic per settings tuple. */
async function determinism(driver, hostUrl, docs, tuples) {
  const runs = [];
  for (const settings of tuples) {
    for (let repeat = 0; repeat < REPEATS; repeat += 1) {
      // A fresh host load per repeat: determinism that only holds inside one
      // long-lived document is not the property a reader needs.
      await driver.goto(hostUrl);
      const perDoc = [];
      for (const doc of docs) {
        const r = await layoutDoc(driver, {
          href: doc.href,
          settings,
          withCoverage: repeat === 0,
          withStats: repeat === 0 && settings.id === 'T1-baseline',
        });
        perDoc.push(r);
      }
      runs.push({ tuple: settings.id, repeat, perDoc });
      log(`    ${settings.id} repeat ${repeat + 1}/${REPEATS}: ` +
        `${perDoc.reduce((a, d) => a + d.metrics.pageCount, 0)} pages`);
    }
  }
  return runs;
}

async function main() {
  const spine = JSON.parse(await readFile(SPINE, 'utf8'));
  const docs = QUICK
    ? spine.filter((s, i) => s.kind === 'stress' || i % 9 === 0)
    : spine;
  const tuples = QUICK ? TUPLES.slice(0, 3) : TUPLES;

  const positionDocs = spine.filter((s, i) => i === 0 || i === 12
    || s.href.includes('floats') || s.href.includes('tables') || s.href.includes('cjk'));
  const selectionDocs = spine.filter((s, i) => i === 0
    || s.href.includes('tables') || s.href.includes('cjk'));

  await mkdir(RESULTS, { recursive: true });
  const servers = await startServers();
  log(`servers: ${servers.hostOrigin} (app) / ${servers.contentOrigin} (content)`);
  const hostUrl = `${servers.hostOrigin}/host.html?content=${encodeURIComponent(servers.contentOrigin)}`;

  const summary = { generatedAt: new Date().toISOString(), repeats: REPEATS, quick: QUICK, engines: [] };

  for (const engineName of ENGINES) {
    log(`\n=== ${engineName} ===`);
    const mod = await loadEngine(engineName);
    let driver;
    try {
      driver = await mod.launch(WINDOW);
    } catch (err) {
      log(`  launch failed: ${err.message}`);
      summary.engines.push({ engine: engineName, launchError: err.message });
      continue;
    }

    const out = {
      engine: driver.name, family: driver.family, version: driver.version,
      startedAt: new Date().toISOString(),
    };

    try {
      log('  probe: sandbox and engine capabilities');
      out.environment = await probeEnvironment(driver, hostUrl, spine[0].href);

      log('  S1: page-count determinism');
      out.determinism = await determinism(driver, hostUrl, docs, tuples);

      log('  S2: position preservation across settings change');
      out.position = [];
      for (const doc of positionDocs) {
        for (const to of ['T2-large-font', 'T4-phone', 'T5-wide-sans']) {
          await driver.goto(hostUrl);
          out.position.push({
            transition: `T1-baseline -> ${to}`,
            ...await positionRun(driver, {
              href: doc.href,
              from: TUPLES[0],
              to: TUPLES.find((t) => t.id === to),
              sampleCount: QUICK ? 4 : 8,
            }),
          });
        }
        log(`    ${doc.href}`);
      }

      log('  S3: selection');
      out.selection = [];
      for (const doc of selectionDocs) {
        await driver.goto(hostUrl);
        out.selection.push(await selectionRun(driver,
          { href: doc.href, settings: TUPLES[0], page: 3 }));
        await driver.goto(hostUrl);
        out.selection.push(await dragSelectionRun(driver,
          { href: doc.href, settings: TUPLES[0], page: 3 }));
        log(`    ${doc.href}`);
      }

      log('  S4: page count vs font size');
      await driver.goto(hostUrl);
      out.monotonic = await monotonicRun(driver, {
        href: spine[0].href,
        base: TUPLES[0],
        sizes: QUICK ? [14, 18, 24] : [12, 14, 16, 18, 20, 24, 28],
      });

      out.consoleErrors = (driver.consoleErrors ?? []).slice(0, 40);
    } catch (err) {
      out.fatal = `${err.message}\n${err.stack ?? ''}`.slice(0, 2000);
      log(`  FATAL: ${err.message}`);
    } finally {
      out.finishedAt = new Date().toISOString();
      await driver.close().catch(() => {});
    }

    await writeFile(join(RESULTS, `${engineName}.json`), JSON.stringify(out, null, 2));
    summary.engines.push({
      engine: engineName, family: out.family, version: out.version, fatal: out.fatal ?? null,
    });
    log(`  wrote results/${engineName}.json`);
  }

  await writeFile(join(RESULTS, 'summary.json'), JSON.stringify(summary, null, 2));
  servers.stop();
  log('\ndone');
}

await main();
