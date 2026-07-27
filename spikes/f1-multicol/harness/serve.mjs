// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Two origins, because the security model needs two (ADR-F005/F007):
//
//   :7101  app origin      — the host page, stands in for tauri://localhost
//   :7102  content origin  — book resources, stands in for futhark-content://<id>
//
// The content server is the stand-in for the registered async URI scheme handler.
// It does what that handler will have to do: serve the unpacked OCF tree with the
// right media types, attach the CSP, and inject the reader's own stylesheet and
// paginator into each content document. Injection happens *here*, on the trusted
// side, precisely because the content document is untrusted — the reader's script
// arrives with the resource rather than being reached in from the app origin,
// which the sandbox forbids anyway.

import { createServer } from 'node:http';
import { readFile, stat } from 'node:fs/promises';
import { dirname, extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
// Content root is the OPF's directory, so hrefs resolve exactly as they do
// inside the container — the URI scheme handler will have the same job.
const BOOK = join(HERE, '..', 'corpus', 'book', 'OEBPS');

const HOST_PORT = Number(process.env.F1_HOST_PORT ?? 7101);
const CONTENT_PORT = Number(process.env.F1_CONTENT_PORT ?? 7102);
const CONTENT_ORIGIN = `http://127.0.0.1:${CONTENT_PORT}`;

const TYPES = {
  '.xhtml': 'application/xhtml+xml; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.opf': 'application/oebps-package+xml',
  '.ncx': 'application/x-dtbncx+xml',
  '.svg': 'image/svg+xml',
  '.json': 'application/json; charset=utf-8',
};

// default-src 'none' means every resource class has to be named explicitly, which
// is the point: anything the book reaches for that is not on this list fails loud.
const CSP = [
  "default-src 'none'",
  `script-src 'self' ${CONTENT_ORIGIN}`,
  `style-src 'self' 'unsafe-inline' ${CONTENT_ORIGIN}`,
  `img-src 'self' data: ${CONTENT_ORIGIN}`,
  `font-src 'self' ${CONTENT_ORIGIN}`,
  "connect-src 'none'",
  "media-src 'none'",
  "object-src 'none'",
  "form-action 'none'",
  "base-uri 'none'",
].join('; ');

const INJECT = `<link rel="stylesheet" type="text/css" href="/__f1/reader.css"/>
<script src="/__f1/locator.js"></script>
<script src="/__f1/paginate.js"></script>
`;

function safeJoin(rootDir, urlPath) {
  const clean = normalize(decodeURIComponent(urlPath)).replace(/^(\.\.[/\\])+/, '');
  const full = join(rootDir, clean);
  return full.startsWith(rootDir) ? full : null;
}

async function sendFile(res, path, extraHeaders, transform) {
  let body;
  try {
    const info = await stat(path);
    if (!info.isFile()) throw new Error('not a file');
    body = await readFile(path);
  } catch {
    res.writeHead(404, { 'content-type': 'text/plain' });
    res.end('not found');
    return;
  }
  const type = TYPES[extname(path)] ?? 'application/octet-stream';
  if (transform) body = Buffer.from(transform(body.toString('utf8')), 'utf8');
  res.writeHead(200, {
    'content-type': type,
    'content-length': body.length,
    'cache-control': 'no-store',
    ...extraHeaders,
  });
  res.end(body);
}

/** Inject the reader stylesheet and paginator before </head>. XHTML-safe. */
function injectReader(source) {
  if (!source.includes('</head>')) return source;
  return source.replace('</head>', `${INJECT}</head>`);
}

const hostServer = createServer(async (req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${HOST_PORT}`);
  const name = url.pathname === '/' ? '/host.html' : url.pathname;
  const path = safeJoin(HERE, name);
  if (!path) {
    res.writeHead(400).end('bad path');
    return;
  }
  await sendFile(res, path, {
    // The app origin gets a CSP too. In production this is where the IPC bridge
    // lives, so it is the origin most worth constraining.
    'content-security-policy': `default-src 'self'; frame-src ${CONTENT_ORIGIN}; style-src 'self' 'unsafe-inline'`,
  });
});

const contentServer = createServer(async (req, res) => {
  const url = new URL(req.url, CONTENT_ORIGIN);
  if (url.pathname.startsWith('/__f1/')) {
    const path = safeJoin(HERE, url.pathname.slice('/__f1'.length));
    if (!path) {
      res.writeHead(400).end('bad path');
      return;
    }
    await sendFile(res, path, { 'content-security-policy': CSP });
    return;
  }
  const path = safeJoin(BOOK, url.pathname);
  if (!path) {
    res.writeHead(400).end('bad path');
    return;
  }
  const isDoc = extname(path) === '.xhtml';
  await sendFile(res, path, {
    'content-security-policy': CSP,
    'x-content-type-options': 'nosniff',
  }, isDoc ? injectReader : null);
});

export function start() {
  return new Promise((resolve) => {
    hostServer.listen(HOST_PORT, '127.0.0.1', () => {
      contentServer.listen(CONTENT_PORT, '127.0.0.1', () => {
        resolve({
          hostOrigin: `http://127.0.0.1:${HOST_PORT}`,
          contentOrigin: CONTENT_ORIGIN,
          stop: () => {
            hostServer.close();
            contentServer.close();
          },
        });
      });
    });
  });
}

if (process.argv[1] && process.argv[1].endsWith('serve.mjs')) {
  const s = await start();
  process.stdout.write(`host:    ${s.hostOrigin}\ncontent: ${s.contentOrigin}\n`);
}
