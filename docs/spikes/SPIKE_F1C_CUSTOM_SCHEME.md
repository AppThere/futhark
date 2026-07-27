<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1c — custom-scheme origin semantics

| Field | Value |
|---|---|
| Document | `SPIKE_F1C_CUSTOM_SCHEME.md` |
| Spike ID | F1c |
| Status | **`http` control run and returned. Custom-scheme run pending a Mac.** |
| Version | 0.2.0 |
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
| **relative image** | **loaded** | **not loaded — see §5** |

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

## 5. Finding: WebKit drops a CSP host-source when a keyword precedes it

Reproducible on WebKitGTK 2.52, not present on Chromium 141. Same document, same
opaque-origin frame, only the `img-src` source list varies:

| `img-src` | Result |
|---|---|
| `http://127.0.0.1:7202` | loaded |
| `http://127.0.0.1:7202 'self'` | loaded |
| `http://127.0.0.1:7202 data:` | loaded |
| `http://example.invalid http://127.0.0.1:7202` | loaded |
| `'self'` | blocked |
| `'self' http://127.0.0.1:7202` | **blocked** |
| `data: http://127.0.0.1:7202` | **blocked** |

A source list is a permissive union, so `'self' <origin>` must allow anything
`<origin>` allows. On WebKit it does not: when a keyword (`'self'`) or a
scheme-source (`data:`) appears *before* a host-source, the host-source stops
matching. Putting the host-source first fixes it. A preceding *host*-source does
not trigger it.

It is also directive-specific. `script-src 'self' <origin>` in the same document
loads `probe.js` without trouble — which is why Spike F1's harness, whose CSP has
the same `'self'`-first shape, ran correctly on WebKitGTK. F1's corpus used inline
SVG rather than `<img>`, so it never exercised `img-src` at all.

**Consequence.** The production CSP is exactly this shape — one keyword plus one
scheme plus the content origin, per resource class. On WebKit that silently drops
book images: no error page, no console entry the user sees, just missing
illustrations. Order host-sources first in every directive, and treat the CSP as
something to validate per engine with this probe rather than to reason about from
the specification. That is an ADR-F005 implementation constraint; it does not
change the decision.

This was found on the control, before any Mac time was spent. It is the argument
for keeping the control permanently rather than treating it as scaffolding.

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
