<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1b — WKWebView coverage

| Field | Value |
|---|---|
| Document | `SPIKE_F1B_WKWEBVIEW.md` |
| Spike ID | F1b |
| Status | **Not run — protocol only. Harness ready.** |
| Version | 0.1.0 |
| Date | 2026-07-27 |
| Depends on | `SPIKE_F1_MULTICOL_PAGINATION.md`, `FUTHARK_PROGRAM_SPEC.md` 0.5.0 §10, R21 |
| Gates | Final confirmation of D1 |
| Harness | `spikes/f1-multicol/` — no changes needed |

---

## 1. Why this exists

F1 measured Blink (Chromium 141, standing in for WebView2 and Android System
WebView) and WebKitGTK 2.50 (the webview Tauri loads on Linux). It did not
measure WKWebView, because WKWebView cannot be driven from Linux.

WKWebView is **macOS as well as iOS**. macOS is in scope for desktop v1 (§1.1),
so this gap is not closed by resolving D2 against iOS — it would still be open
for a desktop-only v1. That is R21, and it is the last factual input to D1.

The claim under test is narrow: *WebKitGTK's results transfer to WKWebView.* It
is plausible — the two share WebCore's fragmentation code, which is what F1
exercises — and unverified. Page counts fall out of line breaking, and line
breaking runs through CoreText on Apple platforms and Pango/HarfBuzz on GTK.
Plausible is not measured.

## 2. Question

> Does the F1 harness produce equivalent results on WKWebView, on macOS?

"Equivalent" means the same three pass criteria hold, not that the numbers match
Linux. Cross-engine page-count divergence is expected and already characterised
(F1 §5.4, ADR-F035); it is not a failure mode here.

## 3. Pass criteria

Identical to F1, evaluated on the safaridriver results alone:

| # | Criterion | Threshold |
|---|---|---|
| 1 | Page count deterministic per settings tuple | 100% of 192 document/tuple pairs stable across 3 fresh loads |
| 2 | Position preserved across a settings change | 100% of anchors visible on the restored page; CFI and quote channels agree |
| 3 | Selection returns usable ranges | every case usable, including drags across a column boundary; 100% locator round-trip |

Plus the two invariants F1 checked alongside them:

| # | Invariant | Expectation |
|---|---|---|
| 4 | Content reachable by paging | no text stranded past the last page, **except** the `vertical-rl` chapter, which is expected to fail — ADR-F034 already routes it to scroll mode |
| 5 | Sandbox boundary | app origin cannot reach `contentDocument`; content is not same-origin with parent; storage denied |

Criterion 5 is the one most likely to behave differently, because iframe sandbox
enforcement is where WebKit's platform ports have historically diverged most from
each other. If it fails, ADR-F005 needs a look before ADR-F001 does.

## 4. Procedure

On the MacBook Air, from a clean checkout of this branch:

```bash
cd spikes/f1-multicol
npm install
npm run corpus                        # deterministic; must match the Linux corpus

safaridriver --enable                 # once per machine, admin prompt
# Safari > Settings > Advanced > Show features for web developers
# Safari > Develop > Allow Remote Automation

node run/run.mjs --tag=macos          # chromium + safaridriver, ~12 min
npm run report
```

Commit `results/chromium-macos.json`, `results/safaridriver-macos.json`,
`results/summary-macos.json`, and the regenerated `results/REPORT.md`.

The Chromium control on the same machine is not redundant. If `chromium-macos`
reproduces `chromium` from Linux, then any WebKit-side difference is attributable
to the engine rather than to the hardware, the fonts, or the display scaling. If
it does *not* reproduce, that is the first thing to explain, before reading the
safaridriver numbers at all.

## 5. Recording the result

Update this document with a §6 Results and a §7 Verdict in the shape F1 used, and
either:

- **Pass** — R21 retired, D1's factual blocker cleared. ADR-F001 becomes a
  decision about product priorities rather than one waiting on evidence.
- **Fail** — record which criterion, on which documents, with the same
  reachability instrumentation F1 used. A criterion-1 or criterion-2 failure on
  WKWebView would re-open §6.5's second sensitivity condition, which F1 closed.
  A criterion-5 failure is a security finding first and a shell finding second.

Either way the spec's F1b row in §10, R21, and D1 need updating to match.

## 6. Known limits of this spike before it runs

- **safaridriver drives Safari, not a Tauri-embedded WKWebView.** Same system
  WebKit, different embedding and process configuration. A disagreement with
  WebKitGTK should be re-checked inside a real Tauri shell before it is believed.
- **iOS is still not covered.** WKWebView on iOS is the same engine family but a
  different device class, with different default text sizing and no pointer
  input. If D2 puts iOS in v1, gesture and text-inflation behaviour there is a
  Phase 5 question, not something F1b answers.
- **macOS-specific text rendering is in scope, and that is the point.** Any
  page-count difference from Linux WebKit is expected to come from CoreText.
  Characterising it is useful; it is not a failure unless criterion 1 breaks.
