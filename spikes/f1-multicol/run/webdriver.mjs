// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Minimal W3C WebDriver client, shared by every engine that speaks the protocol.
//
// No client library: the protocol is a dozen HTTP calls, and a dependency here
// would be a dependency to audit for a harness that is throwaway by charter.
// Factored out of engine-webkitgtk.mjs when safaridriver became the second
// consumer — the two adapters differ only in how the browser is started.

const sleep = (ms) => new Promise((r) => { setTimeout(r, ms); });

export async function rpc(method, url, body) {
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

/** Poll the driver's status endpoint instead of guessing at a startup delay. */
export async function waitForDriver(port, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    try {
      await rpc('GET', `http://127.0.0.1:${port}/status`);
      return;
    } catch (err) {
      if (Date.now() > deadline) throw new Error(`driver on :${port} never came up: ${err.message}`);
      await sleep(250);
    }
  }
}

/**
 * Build the engine-facing surface from a live session. Every engine adapter
 * returns this same five-method shape, so `scenarios.mjs` never learns which
 * browser it is talking to.
 */
export function sessionDriver({ sess, name, family, version, log, teardown }) {
  return {
    name,
    family,
    version,
    consoleErrors: log ?? [],

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
      if (teardown) await teardown();
    },
  };
}

/** Common session setup: generous timeouts, best-effort window sizing. */
export async function configureSession(sess, { windowWidth, windowHeight }) {
  await rpc('POST', `${sess}/timeouts`, { script: 120000, pageLoad: 120000 });
  try {
    await rpc('POST', `${sess}/window/rect`,
      { x: 0, y: 0, width: windowWidth, height: windowHeight });
  } catch {
    // Some drivers refuse to move or resize the window. The iframe box drives
    // page geometry, so this is cosmetic as long as the window is big enough.
  }
  return rpc('POST', `${sess}/execute/sync`, { script: 'return navigator.userAgent', args: [] });
}
