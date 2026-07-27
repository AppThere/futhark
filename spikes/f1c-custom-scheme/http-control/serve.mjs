// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// The `http` control for Spike F1c.
//
// Serves the same probe assets the Tauri scheme handler serves, with the same
// CSP shape and the same two-origin split, over plain http. That is the point:
// F1c's question is what changes when only the transport changes, and the
// question is unanswerable without a baseline taken the other way.
//
//   :7201  app origin      — host.html
//   :7202  content origin  — content.xhtml and its relative resources
//
// This also validates the probe itself on engines that are available here, so
// the version that reaches the Mac is not making its first run in the place
// where a bug is most expensive to diagnose.

import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const PROBE = join(HERE, '..', 'probe');

const APP_PORT = Number(process.env.F1C_APP_PORT ?? 7201);
const CONTENT_PORT = Number(process.env.F1C_CONTENT_PORT ?? 7202);
const CONTENT_ORIGIN = `http://127.0.0.1:${CONTENT_PORT}`;

const TYPES = {
  '.xhtml': 'application/xhtml+xml',
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.css': 'text/css',
  '.png': 'image/png',
};

// Same policy as tauri/src/main.rs, with this transport's origin substituted.
// Any relaxation here would make the comparison meaningless.
const CSP = [
  "default-src 'none'",
  `script-src 'self' ${CONTENT_ORIGIN}`,
  `style-src 'self' ${CONTENT_ORIGIN}`,
  // Overridable so a directive can be bisected when an engine disagrees:
  // F1C_CSP_IMG="*" separates "CSP blocked it" from "the load failed".
  `img-src ${process.env.F1C_CSP_IMG ?? `'self' data: ${CONTENT_ORIGIN}`}`,
  `font-src 'self' ${CONTENT_ORIGIN}`,
  "connect-src 'none'",
  "object-src 'none'",
  "form-action 'none'",
  "base-uri 'none'",
].join('; ');

async function asset(name) {
  const clean = name.replace(/^\/+/, '') || 'content.xhtml';
  if (!/^[a-z0-9.\-]+$/i.test(clean)) return null;
  try {
    return { body: await readFile(join(PROBE, clean)), ext: clean.slice(clean.lastIndexOf('.')) };
  } catch {
    return null;
  }
}

function send(res, found, headers, transform) {
  if (!found) {
    res.writeHead(404, { 'content-type': 'text/plain' }).end('not found');
    return;
  }
  const body = transform ? Buffer.from(transform(found.body.toString('utf8')), 'utf8') : found.body;
  res.writeHead(200, {
    'content-type': `${TYPES[found.ext] ?? 'application/octet-stream'}; charset=utf-8`,
    'content-length': body.length,
    'cache-control': 'no-store',
    ...headers,
  });
  res.end(body);
}

const appServer = createServer(async (req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${APP_PORT}`);
  const name = url.pathname === '/' ? 'host.html' : url.pathname;
  send(res, await asset(name), {
    'content-security-policy':
      `default-src 'self'; frame-src ${CONTENT_ORIGIN}; style-src 'self'`,
  });
});

const contentServer = createServer(async (req, res) => {
  const url = new URL(req.url, CONTENT_ORIGIN);
  const found = await asset(url.pathname);
  // The R24 sweep drives the policy from the query so a hundred permutations
  // cost one server. Only the document's header matters for enforcement, so
  // subresources are served plainly.
  const override = url.searchParams.get('csp');
  send(res, found, {
    'content-security-policy': override && found?.ext === '.xhtml' ? override : CSP,
    'x-content-type-options': 'nosniff',
  }, found?.ext === '.xhtml'
    ? (s) => s.replace('data-transport="unknown"', 'data-transport="http"')
    : null);
});

export function start() {
  return new Promise((resolve) => {
    appServer.listen(APP_PORT, '127.0.0.1', () => {
      contentServer.listen(CONTENT_PORT, '127.0.0.1', () => {
        resolve({
          appOrigin: `http://127.0.0.1:${APP_PORT}`,
          contentOrigin: CONTENT_ORIGIN,
          stop: () => { appServer.close(); contentServer.close(); },
        });
      });
    });
  });
}

if (process.argv[1] && process.argv[1].endsWith('serve.mjs')) {
  const s = await start();
  process.stdout.write(`app:     ${s.appOrigin}\ncontent: ${s.contentOrigin}\n`);
}
