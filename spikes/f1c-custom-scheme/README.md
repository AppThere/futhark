<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1c — custom-scheme origin semantics

Phase 0 spike. Spec 00 §10, R23. **Gates D1 jointly with F1b.**

> Do opaque-origin, CSP, and iframe-sandbox semantics hold under
> `WKURLSchemeHandler` in an actual Tauri macOS build?

Findings: [`docs/spikes/SPIKE_F1C_CUSTOM_SCHEME.md`](../../docs/spikes/SPIKE_F1C_CUSTOM_SCHEME.md).

One probe, two transports, nothing else different. `probe/` is byte-identical
across both; only the server changes.

| | App origin | Content origin |
|---|---|---|
| Control | `http://127.0.0.1:7201` | `http://127.0.0.1:7202` |
| Real | `tauri://localhost` | `futhark-content://localhost` |

## Running it

```bash
# control — any engine, any platform
node http-control/run.mjs                       # writes results/http-<engine>.json
node http-control/run.mjs --engine=safaridriver # on macOS, same engine as the real run

# real — Tauri, macOS
cd tauri && cargo run                           # prints the JSON, writes f1c-result.json
```

The control uses F1's engine adapters, so it needs `npm install` to have been run
once in `../f1-multicol`.

```bash
node http-control/csp-matrix.mjs                # R24: policy permutation sweep
```

The matrix sweeps CSP permutations and judges each against what the standard
requires: fail-open, over-blocking, malformed-list hazard, or the R25 canary. The
runner owns the expectations; the probe only reports what loaded.

## Status of the Tauri shell

Compiles against Tauri 2.11.5, checked on Linux. **Never run, and never on
macOS.** The `cargo run` above is its first execution; expect to fix something.

It is spike code: outside the Cargo workspace (note the empty `[workspace]` table
in its `Cargo.toml`), not the presentation shell, and nothing in `crates/` may
depend on it. ADR-F001 is still provisional and this does not pre-empt it — it
exists to answer one question about origin semantics and then to be evidence.

## Layout

| Path | Role |
|---|---|
| `probe/probe.js` | The probe. Runs in the content frame, records outcomes rather than asserting them. |
| `probe/host.js` | App-origin half. Adds the embedder-only checks and hands the result to Rust over IPC when there is a Rust. |
| `probe/content.xhtml` | Content document. Everything it references is relative, deliberately — that is ADR-F007's claim under test. |
| `http-control/serve.mjs` | Two-origin control server, same CSP shape as the Rust handler. |
| `http-control/run.mjs` | Drives the control on F1's engine adapters. |
| `http-control/csp-matrix.mjs` | R24 policy sweep. Owns the expectations; the probe only reports. |
| `probe/csp-probe.js` | Attempts one load of each resource class and reports which were permitted. |
| `tauri/src/main.rs` | The scheme handler. On macOS this is `WKURLSchemeHandler`, which is the entire point. |

## Why the control is permanent, not scaffolding

It has caught three measurement errors that would otherwise have become findings,
and one real difference:

- A synchronous `img.complete` check reported a permitted image as blocked, which
  produced a WebKit "CSP ordering bug" that does not exist. Retracted in §5.1 of
  the findings.
- A stale `postMessage` result satisfied the wait for the next policy, producing a
  `default-src 'none'` fail-open that does not exist. Results are nonce-keyed now.
- Judging a WebSocket by its constructor rather than its connection made Chromium
  look permissive when it is not.
- Real: `'self'` matches nothing in an opaque-origin frame on WebKit, and does
  match on Chromium. The content CSP names the origin explicitly and never relies
  on `'self'`.

The pattern in the first three is the same — **a synchronous check of an
asynchronous outcome is not a measurement** — and it is why every resource-load
check in the probe now settles on `load`/`error`. Per-engine CSP behaviour is not
something to reason about from the specification, and a probe's own results are
not something to trust without a control that can show it lying.
