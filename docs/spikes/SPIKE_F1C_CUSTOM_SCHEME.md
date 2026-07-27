<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1c — custom-scheme origin semantics

| Field | Value |
|---|---|
| Document | `SPIKE_F1C_CUSTOM_SCHEME.md` |
| Spike ID | F1c |
| Status | **`http` control run and returned; one earlier finding retracted (§5.1). Custom-scheme run pending a Mac.** |
| Version | 0.3.0 |
| Date | 2026-07-27 |
| Depends on | `FUTHARK_PROGRAM_SPEC.md` 0.6.0 §10, R23, ADR-F005, ADR-F007 |
| Gates | D1, jointly with Spike F1b |
| Harness | `spikes/f1c-custom-scheme/` |

---

## 1. The question

> Do opaque-origin, CSP, and iframe-sandbox semantics hold under
> `WKURLSchemeHandler` in an actual Tauri macOS build?

F1b runs the pagination criteria on WKWebView through safaridriver, over `http`.
That is the right engine and the wrong transport. Tauri serves the content origin
through a registered URI scheme, and the custom scheme is not a delivery detail
beside the security model — it *is* the origin mechanism ADR-F005 and ADR-F007
rest on. Opaque-origin behaviour, CSP application, and sandbox enforcement all
hang off how the engine treats that origin.

So F1b's criterion 5 is provisional whatever it reports, and this spike settles
it. R23.

## 2. Design

One probe, two transports, nothing else different:

| | App origin | Content origin | Runs on |
|---|---|---|---|
| **Control** | `http://127.0.0.1:7201` | `http://127.0.0.1:7202` | any engine, anywhere |
| **Real** | `tauri://localhost` | `futhark-content://localhost` | Tauri, macOS |

`probe/probe.js` is byte-identical across both. The Tauri scheme handler and the
control server serve the same assets with the same CSP shape and the same
two-origin split. If the probe differed between them it could not be evidence
about the difference.

The probe records outcomes rather than asserting them. A probe that throws on its
first surprise tells you less than one that finishes and shows you the shape of
the surprise.

## 3. What is checked

| Group | Checks |
|---|---|
| Origin identity | `window.origin`, `document.origin`, `location.origin`, protocol, `isSecureContext` |
| Isolation from the app origin | `parent.location`, `parent.document`, `top.location`, localStorage, sessionStorage, IndexedDB `open()`, cookie write |
| IPC bridge absence | `__TAURI__`, `__TAURI_INTERNALS__`, `ipc`, `webkit.messageHandlers` |
| CSP enforcement | `fetch`, `XMLHttpRequest`, WebSocket **connection outcome**, inline-script execution |
| Sandbox enforcement | top navigation, `window.open`, form submission navigation |
| ADR-F007 fidelity | relative stylesheet applied, relative image loaded |
| Embedder side | `iframe.contentDocument` reachability, `event.origin` of frame messages |

## 4. `http` control results (Linux, 2026-07-27)

Both engines, `spikes/f1c-custom-scheme/results/http-*.json`.

| Check | chromium 141 | webkitgtk 2.52 |
|---|---|---|
| `window.origin` | `null` | `null` |
| `location.origin` | real origin | real origin |
| `isSecureContext` | true | true |
| parent/top location, parent document | SecurityError | SecurityError |
| localStorage, sessionStorage, cookie | SecurityError | SecurityError |
| IndexedDB `open()` | SecurityError | SecurityError |
| fetch / XHR / WebSocket | all blocked | all blocked |
| inline script | blocked | blocked |
| `window.open`, form navigation | blocked | blocked |
| IPC bridge in content frame | absent | absent |
| host reaches `contentDocument` | no | no |
| `event.origin` at the host | `"null"` | `"null"` |
| relative stylesheet | applied | applied |
| relative image | loaded | loaded (after the §5.1 timing fix) |

The sandbox holds on both engines over `http`. This is the baseline the
custom-scheme run is compared against, and it is also what F1b's criterion 5 will
be measuring, which is why a pass there is not evidence about the scheme.

Two corrections this control produced, both in the probe rather than the
architecture:

- **`location.origin` is not the opacity signal.** It is computed from the URL
  and reports a real origin inside an opaque-origin frame. `window.origin` is
  the document's origin and correctly reports `null`. F1's conclusion that the
  frame is opaque was sound — it rested on storage denial and cross-origin
  blocks, not on this value — but the probe now measures it directly.
- **Constructing a WebSocket is not the same as connecting one.** Chromium
  allows construction and fails asynchronously; WebKit throws at once. Judged on
  the constructor, Chromium looked permissive. Judged on the outcome, both block.

## 5. CSP findings

### 5.1 Retraction — there is no WebKit source-ordering bug

**Version 0.2.0 of this document reported that WebKitGTK drops a CSP host-source
when a keyword or scheme-source precedes it in the same directive. That finding
is withdrawn. It was a measurement artifact, and the engine behaves correctly.**

What produced it: the probe judged whether the relative image had loaded by
reading `img.complete && img.naturalWidth > 0` **synchronously**, at script
execution time, with no `load`/`error` listener. An image still in flight reports
`complete === false`, so a permitted resource read as "not loaded". WebKit was
simply slower to finish the request than Chromium was. The apparent
ordering-dependence across the bisection runs was cache warmth between sequential
runs, not policy evaluation.

Re-run under the race-free matrix harness, with the image judged on `load`/`error`,
every source list containing the content origin is permitted on both engines in
every ordering:

| `img-src` | chromium | webkitgtk |
|---|---|---|
| `<origin>` | allowed | allowed |
| `<origin> 'self'` | allowed | allowed |
| `<origin> data:` | allowed | allowed |
| `'self' <origin>` | allowed | allowed |
| `data: <origin>` | allowed | allowed |
| `'self' data: <origin>` | allowed | allowed |
| `'self'` | allowed | **blocked** |

Only the last row differs, and §5.2 is what it means.

**Consequences for the spec.** ADR-F039, ADR-F040's stated example, and R25 all
rest on the retracted premise and need revisiting — that is a decision for the
spec, not for this document. §5.2 offers a replacement rule that is simpler than
the one it replaces. ADR-F040's *principle* — validate CSP per engine rather than
reasoning from the specification — survives intact and is arguably better
evidenced now: the surviving difference was still not predictable from the
standard, and the retraction itself was only caught by re-measuring.

The methodological lesson is narrower and worth keeping: **a synchronous check of
an asynchronous outcome is not a measurement.** The same class of error was caught
twice in this spike — first with `WebSocket` construction versus connection, then
here — which suggests the probe should treat every resource-load check as async by
default rather than as an exception.

### 5.2 What actually differs: `'self'` in an opaque origin

`img-src 'self'` alone permits the image on Chromium and blocks it on WebKit.

WebKit is right. The frame is sandboxed without `allow-same-origin`, so its origin
is opaque, and `'self'` in an opaque-origin document matches nothing. Chromium
matching it is the lenient behaviour.

**Consequence, and it replaces the host-first rule.** The content CSP must name
the content origin explicitly and must never rely on `'self'` — not as an ordering
preference but because `'self'` is meaningless in the frame the policy governs.
That is a simpler rule than host-source-ordering, it is spec-derivable once the
opaque origin is accounted for, and it does not expire when an engine changes.

This matters for the custom-scheme run: whatever `'self'` means under
`futhark-content://`, the policy will not depend on it.

### 5.3 R24 — no ordering-dependent fail-open

The §5.1 table tested the permissive direction: sources that should permit and
did not. That fails closed — missing images, bad but safe. R24 asks the opposite
and more important question: can any ordering make a *restrictive* directive
silently permit? `default-src 'none'` is the backstop for the entire content
sandbox under ADR-F005.

Thirteen policies, both engines, `results/csp-matrix-*.json`. **No fail-open on
either engine.** Both agree on every case:

| Case | Requirement | chromium | webkitgtk |
|---|---|---|---|
| `default-src 'none'` alone | deny everything | script denied (rest unobservable) | same |
| `default-src 'none'; script-src <O>` | script allowed, rest denied | correct | correct |
| same, directive order reversed | identical to above | correct | correct |
| `img-src 'none'; img-src *` | first duplicate wins → deny | denied | denied |
| `img-src *; img-src 'none'` | first duplicate wins → allow | allowed | allowed |
| `img-src 'unsafe-bogus' 'none'` | unparseable source must not relax | denied | denied |
| unknown directive present | ignored, policy intact | denied | denied |
| `DEFAULT-SRC 'NONE'` | case-insensitive | enforced | enforced |
| `default-src<TAB>'none'` | tab separates tokens | enforced | enforced |
| permissive control | must observe allows | allowed | allowed |

Two caveats on how this was measured, because both nearly produced false results:

- **The probe is itself an external script.** Under a restrictive policy it cannot
  run, so silence means `script-src` denied it and every other class is
  *unobservable* — not "passing". Cases that test non-script directives therefore
  permit the probe's own script explicitly. A first version of this sweep scored
  unobservable classes as passes.
- **A stale result satisfied the wait.** The host originally auto-loaded the probe
  once under the default policy; the runner then set a new policy and read the
  previous run's message. That reported a WebKit fail-open on `default-src 'none'`
  that did not exist. Results are now keyed to a per-case nonce.

### 5.4 The one real hazard: `'none'` alongside other sources

`img-src 'none' <origin>` and `img-src <origin> 'none'` both **permit** the image,
on both engines, identically. A source list containing `'none'` together with
other sources is malformed per the CSP grammar; both engines resolve it by
ignoring the `'none'` and honouring the rest.

This is not an engine defect — the input is not valid — but it is a live hazard
for programmatic policy construction: a builder that appends a host source to a
directive already set to `'none'` produces a silently permissive policy, on every
engine. **`'none'` must be exclusive by construction**, enforced in the builder
rather than trusted to the caller.

That is the surviving argument for constructing the CSP programmatically, and it
is a different argument from the retracted one.

## 6. Running the custom-scheme half

The Tauri shell is written and compiles (Tauri 2.11.5, checked on Linux). It has
never been *run*, and never on macOS.

```bash
cd spikes/f1c-custom-scheme/tauri
cargo run                  # opens the probe window, prints the JSON, writes f1c-result.json
```

Save the output as `results/scheme-macos.json` and commit it beside the control.
Then run the control on the same machine for a same-hardware comparison:

```bash
cd .. && node http-control/run.mjs --engine=safaridriver
```

That pairing is the whole experiment: same engine, same probe, same machine, two
transports. Any difference is the scheme.

## 7. Pass criteria

Under `futhark-content://`, on macOS:

| # | Criterion | Required |
|---|---|---|
| 1 | `window.origin` is `null` | yes — an opaque origin is what ADR-F005 assumes |
| 2 | parent/top location, parent document all throw | yes |
| 3 | storage and cookies denied | yes |
| 4 | no IPC bridge, no `webkit.messageHandlers`, in the content frame | yes — ADR-F005 |
| 5 | CSP applied as served: inline script blocked, `connect-src 'none'` enforced | yes |
| 6 | sandbox enforced: no top navigation, no popups, no form navigation | yes |
| 7 | relative stylesheet and image resolve | yes — this is ADR-F007's whole rationale |
| 8 | host cannot reach `contentDocument` | yes |

A failure on 1–6 is a security finding and lands on ADR-F005 before ADR-F001.
A failure on 7 is an ADR-F007 finding: the scheme exists precisely so relative
resources resolve naturally, and if they do not, the fidelity argument for it
weakens even though the security argument survives.

Record `isSecureContext` and `event.origin` whatever they are. Custom schemes are
often not secure contexts, which gates APIs without being a security failure, and
an opaque `event.origin` means the app cannot authenticate frame messages by
origin — a design input for the real IPC boundary, and a live one given R22.

## 8. Known limits

- **macOS only tests macOS.** iOS WKWebView uses the same scheme mechanism but a
  different process model. If D2 puts iOS in v1, this needs a second run there.
- **Windows and Android are unmeasured for the scheme.** WebView2 and Android
  System WebView have their own custom-scheme implementations, and the WebKit
  finding in §5 is a standing warning that per-engine CSP behaviour is not
  something to assume. Worth a control run on each before Phase 1 closes.
- **The shell is spike code.** `spikes/f1c-custom-scheme/tauri/` is outside the
  Cargo workspace, is not the presentation shell, and nothing in `crates/` may
  depend on it. ADR-F001 is still provisional; this exists to answer one question.
