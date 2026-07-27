<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F1 — Multicol pagination in a sandboxed iframe

| Field | Value |
|---|---|
| Document | `SPIKE_F1_MULTICOL_PAGINATION.md` |
| Spike ID | F1 |
| Status | **Returned — criteria met, one qualified failure. D1 remains open pending F1b.** |
| Version | 1.1.0 |
| Date | 2026-07-27 |
| Depends on | `FUTHARK_PROGRAM_SPEC.md` 0.5.0 — §6, §10, R1, R10, R13, R21, R22 |
| Outcome | ADR-F013 accepted; ADR-F034, ADR-F035 written from these findings; R21 and Spike F1b raised |
| Harness | `spikes/f1-multicol/` |
| Raw results | `spikes/f1-multicol/results/REPORT.md` |

> **Revision note (1.1.0).** Version 1.0.0 framed the uncovered WKWebView engine
> as bearing on iOS, and pointed at D2 as the decision that would resolve its
> weight. That was wrong. macOS Tauri uses WKWebView too, and macOS is in scope
> for desktop v1, so resolving D2 against iOS leaves the gap exactly where it is.
> The gap is now R21 in the spec and is closed by Spike F1b, not by D2. §3.1, §6
> and §7 are corrected; no measurement changed.

---

## 1. The question

From Spec 00 §10:

> Can a sandboxed iframe paginate a real 900-page EPUB via CSS multicol, on all
> three webview engines, with stable page counts, correct reflow on font-size
> change, position preservation, and working text selection?
>
> **Pass criteria.** Page count deterministic per settings tuple; position
> preserved across resize; selection returns usable ranges on all three engines.

R10 folds text selection across a paginated iframe into F1 rather than treating
it separately, so it is measured here.

## 2. Answer

**Yes, with one exception that is narrower than it first looks.**

| Criterion | Result |
|---|---|
| Page count deterministic per settings tuple | **Pass.** 192 document/tuple combinations per engine, 3 fresh loads each, 100% identical on both engines. |
| Position preserved across a settings change | **Pass.** 144 / 138 samples per engine, 100% of anchors visible on the restored page, round-trip drift zero in all but one sample. |
| Selection returns usable ranges | **Pass.** Every case usable on both engines, including drags that cross a column boundary. Locator round-trip 100%. |
| Reflow on font-size change | **Pass.** Page count strictly increasing with font size on both engines, no content loss. |
| Content reachable by paging | **Pass except `writing-mode: vertical-rl`.** See §5.1. |

The exception is vertical writing mode, not CJK. A horizontal CJK chapter with
ruby annotations — the same text, differing only in `writing-mode` — paginates
cleanly on both engines. Vertical-rl content inside a horizontal multicol
container strands 164–280 text rectangles past the last reachable page on
WebKitGTK and 3–15 on Chromium, depending on the settings tuple.

**F1 does not fail.** The §6.5 sensitivity condition — "if a webview cannot
deliver stable, position-preserving pagination via CSS multicol" — is not met.
That condition was the trigger to reverse ADR-F001, and it did not fire.

**D1 is still the human's decision.** F1 removes the reason to reverse ADR-F001;
it does not confirm it, because F1 could not touch WKWebView — which is macOS as
well as iOS, and macOS is in scope for desktop v1. That gap is R21, and Spike F1b
closes it. See §3.1 and §6.

---

## 3. What was measured, and on what

The harness (`spikes/f1-multicol/`) generates a 32-document EPUB 3 OCF tree of
475,752 words — **900 pages at the baseline settings tuple on Chromium, 872 on
WebKitGTK** — and paginates it through the architecture ADR-F005/F007 describe:
a content origin distinct from the app origin, a sandboxed iframe with
`sandbox="allow-scripts"` and no `allow-same-origin`, `default-src 'none'` CSP,
and every measurement crossing `postMessage` because nothing else can cross.

Twenty-six chapters are generated prose. Six are stress cases chosen because they
are where multicol breaks rather than where it is comfortable: floats interleaved
with body text, long tables, CJK horizontal with ruby, CJK vertical-rl with ruby,
MathML, and overlong unbreakable runs with preformatted blocks.

### 3.1 Engine coverage, and the gap

| Target webview | Family | Platform | Covered | How |
|---|---|---|---|---|
| webkit2gtk-4.1 | WebKit | Linux desktop | **directly** | WebKitGTK 2.50.4 via `WebKitWebDriver` |
| WebView2 | Blink | Windows desktop | by proxy | Chromium 141 |
| Android System WebView | Blink | Android | by proxy | Chromium 141 |
| WKWebView | WebKit | **macOS desktop** and iOS | **not covered** | — |

The Linux target is measured directly rather than approximated: webkit2gtk-4.1 is
the webview Tauri actually loads on Linux. The Blink proxy is sound — WebView2 and
Android System WebView are Chromium's layout engine with different embedding.

**The gap is WKWebView, and it is a desktop gap before it is a mobile one.**
Tauri uses WKWebView on macOS as well as iOS. macOS is in scope for desktop v1
(§1.1), so this hole does not close by deciding iOS is out — D2 has no bearing on
it. WebKitGTK shares WebCore's fragmentation code with WKWebView, which is the
code under test here, but not CoreText rasterisation or the platform font stack,
and F1's numbers are sensitive to both: page counts fall out of line breaking.

**Every claim about macOS or iOS below is inference from WebKitGTK.** This is
R21 in the spec, and it is closed by Spike F1b, not by argument. The harness is
engine-agnostic and `run/engine-safaridriver.mjs` is written and committed, so
F1b is a run rather than a build — see §8.

The spike's phrase "all three webview engines" is therefore answered as two of
three directly, the third not at all. That is a limitation of this run on this
hardware, not a finding about the architecture.

---

## 4. Results

Raw counts, including every per-document number, are in
`spikes/f1-multicol/results/REPORT.md`.

### 4.1 Determinism — pass

192 document/tuple pairs per engine (32 documents × 6 settings tuples), each laid
out three times from a fresh host page load. **100% of page counts identical
across repeats, on both engines**, including every stress document. Whole-book
totals were bit-stable too: Chromium reported 900 / 1849 / 647 / 1686 / 891 / 970
pages for the six tuples on all three repeats; WebKitGTK 872 / 1844 / 635 / 1609 /
866 / 942.

The geometry that makes this work is worth recording, because it is the
implementation of ADR-F013 and it is not the obvious arrangement:

- `column-gap` is set to exactly twice the horizontal page margin, so the column
  stride (`column-width + column-gap`) equals the viewport width. Page *n* is at
  `scrollLeft = n × viewportWidth`, with no fractional stride to accumulate error
  across 900 pages.
- The scroll container is the root element with `overflow: hidden`. Programmatic
  `scrollLeft` works on both engines despite the hidden overflow; the harness
  asserts this rather than assuming it (`scrollerOk`).
- `text-size-adjust: none` is mandatory. Without it WebKit inflates text in narrow
  viewports, which makes page counts depend on the viewport in a way no user
  setting explains.

### 4.2 Position preservation — pass

Every sampled position is anchored three independent ways — a DOM id, an EPUB CFI
path, and a prefix/exact/suffix quote (ADR-F014, ADR-F015) — so that a locator bug
cannot be mistaken for an engine bug.

Transitions measured: baseline → 24px font, baseline → 414×896 phone viewport,
baseline → 1280×800 wide sans. Then back to baseline.

| Engine | Samples | Anchor visible on restored page | CFI and quote agree | Round-trip drift |
|---|---:|---:|---:|---|
| chromium | 144 | 144 (100%) | 100% | 0 pages in 144/144 |
| webkitgtk | 138 | 138 (100%) | 100% | 0 pages in 137/138 |

The single non-zero result is one sample in the tables chapter on WebKitGTK,
which returned to page 3 instead of page 4 after a baseline → phone → baseline
round trip — a locator landing one page early at a table boundary, not lost
content.

Note what "preserved" means here and what it cannot mean: the *page number*
changes when pagination changes — page 4 at 16px becomes page 8 at 24px — and
that is correct behaviour. What is preserved is the reader's place in the text.
This is the empirical case for ADR-F014: position must be a locator, never a page
index.

### 4.3 Selection — pass

| Case | Both engines |
|---|---|
| Programmatic range within one column | usable, text matches, locator round-trips |
| Programmatic range across a column boundary | usable, spans both columns, locator round-trips |
| Pointer drag within the visible page | usable, locator round-trips |
| Pointer drag past the right page edge | usable, spans both columns, locator round-trips |

Drags are issued by the driver against the host page, so they cross the sandbox
boundary the way a finger does. R10's worry — that selection across a
multicol-paginated iframe returns unusable ranges — is not borne out on either
engine.

One divergence, and it is not an engine defect: for a range crossing a table,
`Selection.toString()` and `Range.toString()` return different text on **both**
engines. Selection inserts tab and newline separators between cells; Range
concatenates raw character data. This is a specification-level difference, and it
has a design consequence — see §5.2.

### 4.4 Reflow — pass

Page count is strictly increasing in font size on both engines across
12/14/16/18/20/24/28px, with no content loss at any size and a constant
`scrollWidth` remainder, confirming the stride arithmetic holds as the document
grows to 90 pages.

### 4.5 Layout cost

Median layout flush per document: 10.2 ms Chromium, 13.0 ms WebKitGTK; worst case
21.9 ms and 68.0 ms. Documents here average ~28 pages. This is a per-spine-item
cost, which is the right unit — J2's 400 ms budget covers loading one spine item
and resolving a locator inside it, not paginating the book. Nothing here threatens
that budget on desktop; mobile is unmeasured.

### 4.6 The sandbox boundary held, and pagination worked through it

| Probe | chromium | webkitgtk |
|---|---|---|
| App origin can reach `iframe.contentDocument` | no | no |
| Content can reach `parent.location` | no | no |
| Content `localStorage` | `SecurityError` | `SecurityError` |
| `window.__TAURI__` present in content | no | no |

This is the result with the largest architectural consequence and it was not
guaranteed in advance: an opaque-origin sandboxed iframe can be paginated, paged,
measured, selected in, and anchored, entirely over `postMessage`, with no
same-origin access at any point.

---

## 5. Findings that change the design

### 5.1 `writing-mode: vertical-rl` is not paginable by this mechanism

This is R13, and it is worse than "renders poorly on one engine".

| Engine | Vertical CJK | Horizontal CJK (same text) |
|---|---|---|
| chromium | 3–15 text rects past the last reachable page, varying by tuple | clean |
| webkitgtk | 164–280 text rects stranded; `scrollWidth` never exceeds one viewport, so the document reports **1 page** while text is laid out out to page 12 | clean apart from two trailing blank pages |

WebKitGTK's behaviour is the serious one. It lays the vertical content out but
never grows the scroll region, so roughly 80% of the chapter is unreachable by
paging and the page count is silently wrong. A user would see page 1 of 1 and no
way to reach the rest.

The horizontal control chapter is what makes this actionable: the failure is the
writing mode, not the script. Most Chinese and modern Japanese ebooks are
horizontal and are unaffected.

**Consequence.** Vertical writing mode needs a different presentation path —
column-flow direction matched to the writing mode, or scrolled rather than
paginated presentation for vertical content — decided as part of the Rendering
spec rather than inherited from ADR-F013. ADR-F013 should be confirmed with
"except for vertical writing modes" written into it, not discovered in Phase 3.

Spec 00 §9 lists R13 as Medium/Medium with the mitigation "add CJK titles to the
conformance corpus from Phase 1". That mitigation is right and should stand; the
severity is understated for vertical-rl specifically, where the failure is silent
content loss rather than degraded rendering.

### 5.2 Annotation text must come from `Range`, never from `Selection.toString()`

Both engines serialise a table-crossing selection differently through the two
APIs. An annotation whose stored text came from `Selection.toString()` will not
match the document when it is re-anchored by quote (ADR-F015), because the quote
carries separators the document does not contain. Annotations should capture
`Range` contents; this costs nothing to do correctly and is expensive to
retrofit once highlights exist in users' libraries.

### 5.3 The paginator must be delivered by the resource server, not injected by the app origin

Because the content frame has an opaque origin, the app origin cannot inject a
script into it — there is no `contentDocument` to touch. The reader's own
pagination code has to arrive *with* the resource, from the URI scheme handler
that ADR-F007 already requires.

This is a constraint on ADR-F006's implementation rather than a conflict with it.
Book script is still stripped at ingest; the reader's script is added by the
trusted Rust side on the way out. The two are distinguishable because they have
different provenance, which is only true if injection happens in the handler.

It does move the paginator into the same execution context as book content, which
the spec now tracks as R22. The opaque origin still holds — hostile script reaches
neither the app origin nor the IPC bridge — so the mitigation is on the receiving
end: the Rust side validates reported offsets and page indices rather than
trusting them, exactly as it would validate any other untrusted input.

### 5.4 Page counts are stable per engine and portable across none of them

Across the 192 document/tuple pairs, Chromium and WebKitGTK agreed exactly 64
times, differed by one page 87 times, by two 24 times, by three 14 times, and by
four 3 times. On baseline prose the mean divergence is 2.99% and the worst 6.45%.

Within an engine this is perfectly reproducible — criterion 1 passed at 100% — so
"page 214 of 480" is a stable, meaningful number *on a given device*. It is not
the same number on another one.

Spec 00 §6.3 already concedes this as Tauri's cost against Dioxus Native's
byte-identical output, scoring rendering determinism 2 against 5. F1 confirms it
with numbers and bounds it: the divergence is small and systematic, not erratic.

**Consequence — and it is smaller than the raw number suggests.** Position and
annotations were never going to be page-based: ADR-F014 anchors on CFI, and §4.2
measures that at 100% locator round-trip and 100% position preservation within an
engine. So the divergence does not touch correctness. What it touches is the
*displayed page number*, and ADR-F035 resolves that the way Kindle and Kobo
already do — progress reported in device-independent locators, page numbers
labelled as per-device.

That converts most of the determinism penalty into a labelling decision, which is
why this finding should **not** be fed into the §6.5 sensitivity re-weighting as
though it were a fidelity failure. It is a real cost against Dioxus Native's
byte-identical output, already priced into §6.3's score of 2; F1 measures its
size rather than discovering a new one.

---

## 6. Recommendation

**On ADR-F013 (CSS multicol with scroll-offset paging): confirm**, with the
vertical writing mode exception from §5.1 written into the ADR text. *(Accepted
in spec 0.5.0, with ADR-F034 carrying the exception.)*

**On ADR-F001 (Tauri 2): the gate does not fire.** F1 was the condition under
which §6.5 said to reverse the recommendation, and F1 passed. Reversing on this
evidence would not be justified.

**D1 is not resolved by this document.** One thing blocks it, and it is factual
rather than a judgement call: **WKWebView is unmeasured, and it is a desktop
target.** ADR-F001 commits macOS to WKWebView, macOS is in v1, and F1 produced no
evidence from that engine at all. Spike F1b answers it; until then D1 rests on
inference for one of its three engines.

Two things that are *not* blockers, stated so they are not mistaken for them:

- **The 2.99% page-count divergence (§5.4)** is a display concern, resolved by
  ADR-F035. It is already priced into §6.3's determinism score and should not be
  re-litigated as though F1 discovered it.
- **D2 (iOS in v1?)** does not gate the WKWebView question. It changes how much
  the §6.5 re-weighting matters, but it cannot close R21, because dropping iOS
  leaves macOS on the same engine.

What F1 establishes for D1 is that the *technical* objection is gone on the two
engine families it could reach: the pagination mechanism works, the security
model does not obstruct it, and position and selection survive.

---

## 7. What F1 did not answer

- **WKWebView on real Apple hardware — macOS first, iOS second.** §3.1, R21,
  Spike F1b. This is the one gap that gates D1.
- **Mobile performance and memory.** Layout cost was measured on desktop only.
  R9 (LMK/jetsam with a large book open) is untouched; the harness loads one
  spine item at a time, which is the mitigation, but never measured its cost on
  a device.
- **Real-world EPUB corpus.** The corpus is synthetic because D7 is open. It is
  structurally faithful — real OPF, nav, NCX, XHTML content documents served as
  `application/xhtml+xml` — and the stress chapters are hand-written to be
  hostile. It is not a substitute for 200 real books, which is F2's corpus
  problem too and should be solved once for both.
- **Scripted EPUB content.** ADR-F006 strips it; the spike never enabled the
  per-book opt-in, so the sandbox was never tested against content that
  actively tries to escape. That belongs to the R2 threat model, not here.
- **`.epub` zip containers.** The harness serves the unpacked OCF tree, which is
  what the URI scheme handler will serve. Container parsing is F2's question.

---

## 8. Reproducing

```bash
cd spikes/f1-multicol
npm install
npm run corpus
npm run spike            # ~12 minutes; engines default by platform
npm run report
```

WebKitGTK needs `apt install webkit2gtk-driver xvfb`. Chromium comes from
Playwright. Results land in `results/*.json`; `results/REPORT.md` is generated
from them and contains no interpretation.

### 8.1 Running F1b on macOS

The WKWebView adapter is committed. On the MacBook Air, once:

```bash
safaridriver --enable    # admin prompt
# Safari > Settings > Advanced > Show features for web developers
# Safari > Develop > Allow Remote Automation
```

then:

```bash
npm run corpus
node run/run.mjs --tag=macos          # chromium + safaridriver
npm run report
```

`--tag` suffixes the output filenames, so the macOS results sit beside the Linux
ones rather than overwriting them and `report.mjs` tabulates all four engines
together. The Blink control on the same machine is deliberate: if
`chromium-macos` matches `chromium` from Linux, then any WebKit-side difference
is the engine rather than the hardware.

**What safaridriver measures.** Safari and WKWebView on the same macOS build
share the system WebKit — the same WebCore fragmentation code, the same CoreText
rasterisation, the same fonts. That is the entire surface F1 exercises. What it
does not share is process configuration and embedding: a Tauri WKWebView is
created by the app with its own `WKWebViewConfiguration`. Nothing measured here
is known to depend on that, but a safaridriver result that disagrees with
WebKitGTK should be re-checked in a real Tauri shell before it is believed.
