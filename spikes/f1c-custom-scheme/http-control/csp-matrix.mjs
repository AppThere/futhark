// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// R24 — does any source or directive ordering make a restrictive CSP fail open?
//
//   node http-control/csp-matrix.mjs [--engine=chromium,webkitgtk]
//
// The img-src finding tested over-blocking, which fails safe. This sweeps the
// other direction. `default-src 'none'` is the backstop for the entire content
// sandbox under ADR-F005; if any ordering can make it, or a restrictive
// override, silently permit a load, that is an R2-class finding rather than a
// rendering bug.
//
// The runner owns the expectations. The probe only reports what loaded.

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
const loadEngine = (n) => import(`../../f1-multicol/run/engine-${n}.mjs`);

/**
 * Each case declares the outcome the CSP *requires*, per the specification.
 * `allow` and `deny` are assertions; `either` records without judging, for
 * cases where the standard genuinely does not pin the answer.
 *
 * The security-relevant assertions are the `deny`s. An `allow` that comes back
 * blocked is the over-blocking class already characterised in §5 of the
 * findings — noted, not fatal. A `deny` that comes back allowed is fail-open.
 */
function cases(origin) {
  const O = origin;
  return [
    {
      id: 'backstop-none',
      csp: "default-src 'none'",
      expect: { externalScript: 'deny', inlineScript: 'deny', evalAllowed: 'deny', externalStyle: 'deny', image: 'deny', fetch: 'deny', xhr: 'deny' },
      note: 'The whole sandbox rests on this one line.',
    },
    {
      id: 'none-plus-script-after',
      csp: `default-src 'none'; script-src ${O}`,
      expect: { externalScript: 'allow', inlineScript: 'deny', evalAllowed: 'deny', externalStyle: 'deny', image: 'deny', fetch: 'deny' },
      note: 'Specific directive overrides the default.',
    },
    {
      id: 'none-plus-script-before',
      csp: `script-src ${O}; default-src 'none'`,
      expect: { externalScript: 'allow', inlineScript: 'deny', evalAllowed: 'deny', externalStyle: 'deny', image: 'deny', fetch: 'deny' },
      note: 'Same policy, directive order reversed. Order must not matter.',
    },
    {
      id: 'duplicate-deny-then-allow',
      csp: `default-src 'none'; script-src ${O}; img-src 'none'; img-src *`,
      expect: { image: 'deny' },
      note: 'CSP: the first occurrence of a directive wins, later duplicates are ignored. A later permissive duplicate winning is fail-open.',
    },
    {
      id: 'duplicate-allow-then-deny',
      csp: `default-src 'none'; script-src ${O}; img-src *; img-src 'none'`,
      expect: { image: 'allow' },
      note: 'Mirror of the above. Together they pin which duplicate the engine honours.',
    },
    {
      id: 'none-with-host-after',
      csp: `default-src 'none'; script-src ${O}; img-src 'none' ${O}`,
      expect: { image: 'deny-intent' },
      note: "'none' alongside other sources is a malformed list per the CSP grammar. Both engines ignore the 'none' and honour the rest — permissively, and identically in both orderings. Not an engine bug; a construction hazard (ADR-F039).",
    },
    {
      id: 'none-with-host-before',
      csp: `default-src 'none'; script-src ${O}; img-src ${O} 'none'`,
      expect: { image: 'deny-intent' },
      note: 'Same list, reversed. The pair is what shows the permissive resolution is order-independent, so this is not the R24 ordering hazard.',
    },
    {
      id: 'unknown-keyword-then-none',
      csp: `default-src 'none'; script-src ${O}; img-src 'unsafe-bogus' 'none'`,
      expect: { image: 'deny' },
      note: 'An unparseable source must not relax the rest of the list.',
    },
    {
      id: 'garbage-directive',
      csp: `default-src 'none'; script-src ${O}; not-a-directive whatever; img-src 'none'`,
      expect: { image: 'deny' },
      note: 'An unknown directive must be ignored without weakening the policy.',
    },
    {
      id: 'uppercase-directive',
      csp: `DEFAULT-SRC 'NONE'`,
      expect: { externalScript: 'deny' },
      note: 'Directive names and keywords are ASCII case-insensitive.',
    },
    {
      id: 'tab-separated',
      csp: `default-src\t'none'`,
      expect: { externalScript: 'deny' },
      note: 'Whitespace other than a plain space still separates tokens.',
    },
    {
      // R25: the host-first workaround in ADR-F039 exists because this case
      // blocks on WebKit. If it ever starts loading there, the engine has been
      // fixed and the workaround has expired — which is something to be told,
      // not to discover by accident years later.
      id: 'r25-canary-keyword-first',
      csp: `default-src 'none'; script-src ${O}; img-src 'self' ${O}`,
      expect: { image: 'canary' },
      note: 'WebKit drops the host-source when a keyword precedes it. Blocked here means the bug is still present and ADR-F039 is still required.',
    },
    // The original §5 bisection, re-run under the race-free harness and with an
    // image check that waits for load/error instead of reading `complete`
    // synchronously. The full default-policy shape is reproduced because the
    // first bisection varied only img-src within it.
    ...[
      ["host-only", O],
      ["host-then-self", `${O} 'self'`],
      ["host-then-data", `${O} data:`],
      ["self-only", "'self'"],
      ["self-then-host", `'self' ${O}`],
      ["data-then-host", `data: ${O}`],
      ["self-data-host", `'self' data: ${O}`],
    ].map(([id, sources]) => ({
      id: `order-${id}`,
      csp: `default-src 'none'; script-src 'self' ${O}; style-src 'self' ${O}; `
        + `img-src ${sources}; font-src 'self' ${O}; connect-src 'none'`,
      expect: { image: 'either' },
      note: `img-src sources: ${sources}`,
    })),
    {
      id: 'control-permissive',
      csp: `default-src *; script-src ${O} 'unsafe-inline'; style-src ${O} 'unsafe-inline'; img-src ${O}`,
      expect: { externalScript: 'allow', image: 'allow', externalStyle: 'allow' },
      note: 'Proves the matrix can observe an allow at all. Without this a probe that blocks everything looks like a perfect pass.',
    },
  ];
}

// Waits for a result carrying *this* request's nonce, so a message from the
// previous policy cannot be mistaken for this one's.
//
// A probe that never reports is not a missing measurement: the probe is itself
// an external script, so silence means script-src denied it. That is the only
// thing it proves — every other class is unobservable once the probe is gone,
// which is why the cases that test non-script directives always permit the
// probe's own script explicitly.
const COLLECT = `async (arg) => {
  const frame = document.getElementById('content');
  window.__F1C_CSP_RESULT = null;
  frame.src = arg.content + '/csp-probe.xhtml?csp=' + encodeURIComponent(arg.csp)
    + '&n=' + encodeURIComponent(arg.nonce);
  for (let i = 0; i < 100; i += 1) {
    const r = window.__F1C_CSP_RESULT;
    if (r && r.nonce === arg.nonce) return { probeRan: true, ...r.results };
    await new Promise((res) => setTimeout(res, 100));
  }
  return { probeRan: false };
}`;

function judge(expect, results) {
  const ran = results?.probeRan === true;
  const rows = [];
  for (const [key, want] of Object.entries(expect)) {
    // Silence tells us about script-src and nothing else.
    let got;
    if (!ran) got = key === 'externalScript' ? 'blocked' : 'unobservable';
    else got = results?.[key] ?? 'missing';

    let verdict = 'ok';
    if (got === 'unobservable') verdict = 'unobservable';
    // A malformed list resolved permissively is a hazard for whoever builds the
    // policy, not an engine defect — the grammar does not permit the input. It
    // is tracked separately so the fail-open count stays a count of real ones.
    else if (want === 'deny-intent') verdict = got === 'allowed' ? 'HAZARD-permissive' : 'ok';
    else if (want === 'canary') verdict = got === 'allowed' ? 'CANARY-TRIPPED' : 'bug-still-present';
    else if (want === 'deny' && got === 'allowed') verdict = 'FAIL-OPEN';
    else if (want === 'allow' && got === 'blocked') verdict = 'over-blocked';
    else if (want !== 'either' && got !== (want === 'deny' ? 'blocked' : 'allowed')) verdict = `unexpected:${got}`;
    rows.push({ key, want, got, verdict });
  }
  return rows;
}

async function main() {
  await mkdir(RESULTS, { recursive: true });
  const servers = await startServers();
  const list = cases(servers.contentOrigin);
  log(`app: ${servers.appOrigin}  content: ${servers.contentOrigin}  cases: ${list.length}`);

  for (const name of ENGINES) {
    log(`\n=== ${name} ===`);
    let driver;
    try {
      driver = await (await loadEngine(name)).launch({ windowWidth: 900, windowHeight: 700 });
    } catch (err) {
      log(`  launch failed: ${err.message}`);
      continue;
    }
    const out = { engine: driver.name, version: driver.version, platform: `${process.platform}-${process.arch}`, cases: [] };
    try {
      await driver.goto(`${servers.appOrigin}/csp-host.html`
        + `?content=${encodeURIComponent(servers.contentOrigin)}`);
      for (const c of list) {
        const results = await driver.evalAsync(COLLECT,
          { content: servers.contentOrigin, csp: c.csp, nonce: `${name}-${c.id}` });
        const rows = judge(c.expect, results);
        const failOpen = rows.filter((r) => r.verdict === 'FAIL-OPEN');
        const hazard = rows.filter((r) => r.verdict === 'HAZARD-permissive');
        const canary = rows.filter((r) => r.verdict === 'CANARY-TRIPPED');
        out.cases.push({ ...c, results, rows, failOpen: failOpen.length, hazard: hazard.length });
        const notes = [];
        if (failOpen.length) notes.push(`FAIL-OPEN: ${failOpen.map((r) => r.key).join(',')}`);
        if (hazard.length) notes.push('malformed-list resolved permissively');
        if (canary.length) notes.push('CANARY TRIPPED — WebKit ordering bug appears fixed; revisit ADR-F039');
        const odd = rows.filter((r) => r.verdict === 'over-blocked' || r.verdict.startsWith('unexpected'));
        if (odd.length) notes.push(odd.map((r) => `${r.key}=${r.got}`).join(' '));
        log(`  ${c.id.padEnd(26)} ${failOpen.length ? 'FAIL' : 'ok  '}`
          + `${notes.length ? `  ${notes.join(' | ')}` : ''}`);
      }
    } catch (err) {
      out.fatal = err.message;
      log(`  FATAL: ${err.message}`);
    } finally {
      await driver.close().catch(() => {});
    }
    out.failOpenTotal = out.cases.reduce((a, c) => a + (c.failOpen ?? 0), 0);
    out.hazardTotal = out.cases.reduce((a, c) => a + (c.hazard ?? 0), 0);
    await writeFile(join(RESULTS, `csp-matrix-${name}.json`), JSON.stringify(out, null, 2));
    log(`  fail-open: ${out.failOpenTotal}  construction hazards: ${out.hazardTotal}`
      + `  -> results/csp-matrix-${name}.json`);
  }

  servers.stop();
  log('\ndone');
}

await main();
