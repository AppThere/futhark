<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1 — multicol pagination in a sandboxed iframe

Phase 0 spike. Spec 00 §10, R1, R10. **Gates ADR-F001 and ADR-F013.**

> Can a sandboxed iframe paginate a real 900-page EPUB via CSS multicol, on all
> three webview engines, with stable page counts, correct reflow on font-size
> change, position preservation, and working text selection?

Findings and the recommendation are in
[`docs/spikes/SPIKE_F1_MULTICOL_PAGINATION.md`](../../docs/spikes/SPIKE_F1_MULTICOL_PAGINATION.md).
Raw counts are in `results/REPORT.md`, regenerated from `results/*.json`.

This directory is throwaway by charter. Nothing here is a dependency of any
`futhark-*` crate, nothing here is in the Cargo workspace, and none of it ships.
It exists to answer one question and then to be evidence that the question was
answered.

## Running it

```bash
npm install                 # playwright only; browsers come from PLAYWRIGHT_BROWSERS_PATH
npm run corpus              # generate the book (deterministic, ~2 MB, gitignored)
npm run spike               # both engines, full sweep
npm run report              # results/*.json -> results/REPORT.md
```

Useful flags:

```bash
node run/run.mjs --engine=chromium          # one engine
node run/run.mjs --quick --repeats=1        # smoke test, ~2 minutes
node run/run.mjs --engine=webkitgtk         # needs webkit2gtk-driver + Xvfb
```

WebKitGTK needs `apt install webkit2gtk-driver xvfb`. The driver spawns its own
Xvfb unless `F1_REUSE_DISPLAY` is set, in which case it uses `$DISPLAY` — useful
when you want to watch the pagination happen.

## What is where

| Path | Role |
|---|---|
| `corpus/build-corpus.mjs` | Generates the EPUB 3 OCF tree: 26 prose chapters plus five stress chapters (floats, tables, CJK vertical-rl + ruby, MathML, unbreakable runs). |
| `harness/serve.mjs` | Two origins: app on `:7101`, content on `:7102`. The content server stands in for the registered URI scheme handler — it attaches the CSP and injects the reader stylesheet and paginator. |
| `harness/host.html` + `host.js` | App origin. Owns the sandboxed iframe, talks to it over postMessage, and cannot reach into it. |
| `harness/reader.css` | ADR-F013 geometry: `column-gap` equals twice the horizontal margin so the stride equals the viewport width exactly. |
| `harness/paginate.js` | The paginator, running on the content origin. Layout, paging, coverage, selection. |
| `harness/locator.js` | EPUB CFI paths and quote anchors (ADR-F014/F015). |
| `run/engine-*.mjs` | One file per engine, five methods each. Adding WebView2 or WKWebView means adding a file, not touching the scenarios. |
| `run/scenarios.mjs` | The measurements, one per clause of the pass criteria. |
| `run/report.mjs` | Counts. It is not allowed to draw conclusions. |

## Engine coverage, and what is missing

| Target webview | Engine family | Covered here | How |
|---|---|---|---|
| Linux (webkit2gtk-4.1) | WebKit | **directly** | WebKitGTK 2.50 via WebKitWebDriver |
| Windows (WebView2) | Blink | by proxy | Chromium, same layout engine |
| Android System WebView | Blink | by proxy | Chromium, same layout engine |
| macOS / iOS (WKWebView) | WebKit | **by proxy only** | WebKitGTK shares WebCore fragmentation, not the platform text stack |

The gap that matters is WKWebView on Apple hardware, which cannot be driven from
Linux at all. Re-running `run/run.mjs` on a Mac with a WKWebView engine adapter
closes it; the scenarios need no changes. Until then, any Apple-platform claim in
the findings is inference, and is labelled as such.
