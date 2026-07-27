<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# AppThere Futhark — Program Specification (Spec 00)

| Field | Value |
|---|---|
| Document | `FUTHARK_PROGRAM_SPEC.md` |
| Spec ID | 00 |
| Status | **Draft — pending open decisions in §12** |
| Version | 0.10.0 |
| Date | 2026-07-27 |
| Supersedes | — |
| Depends on | `LOKI_*` specs (document model precedent), `APPTHERE_CLOUD_*` (sync substrate) |
| Naming | Components are named descriptively. Context names match crate names. |

---

## 1. Purpose

Futhark is the AppThere suite's **ebook library manager, reader, and editor**. It occupies the
position Calibre + a reader app + Sigil occupy today, but as one coherent native application
running the same core on desktop and mobile.

The product has three primary surfaces:

1. **Library** — the shelf. Catalog of every book and document the user owns, with metadata,
   covers, collections, search, and filtering.
2. **Reader** — the reading surface. Paginated or scrolling reflow for EPUB/MOBI, fixed-page
   rendering for PDF, with annotations, bookmarks, progress, and typography controls.
3. **Editor** — the workbench. Structural and source-level editing of EPUB 2/3 packages, with a
   live preview and validation.

### 1.1 In scope

- Reading: EPUB 2, EPUB 3, MOBI/PRC, AZW/AZW3 (KF8), PDF.
- Editing: EPUB 2 and EPUB 3 only.
- Authoring/export: EPUB 2, EPUB 3, and KF8/AZW3 — on all five platforms, at parity.
- Library management across desktop (Windows, macOS, Linux) and mobile (Android, iOS).
- Local-first storage with optional AppThere Cloud sync of annotations and reading position.

### 1.2 Non-goals (v1)

| Non-goal | Rationale |
|---|---|
| PDF editing | Explicitly excluded by charter. PDF is read-only. |
| DRM circumvention of any kind | See ADR-F021. Non-negotiable. |
| KFX, in any direction | Removed from the program entirely. See §5.3. |
| Writing MOBI/KF7 in any form | Dropped (D9). KF8 is the only output. Reading dual KF7+KF8 files is unaffected. |
| Comic formats (CBZ/CBR) | Deferred to Phase 7. Cheap once the fixed-layout pipeline exists. |
| Audiobooks / M4B | Out of program. Belongs with Bragi if ever. |
| Store integration / purchasing | No commerce surface in v1. |
| OPDS *serving* | Futhark consumes OPDS catalogs; it does not publish one in v1. |
| Full-text search across the whole library | Deferred to Phase 5 (see R11). Per-book search is v1. |

---

## 2. Core user journeys

| ID | Journey | Surface |
|---|---|---|
| J1 | Point Futhark at a folder of 4,000 mixed-format files; get a browsable, deduplicated library with covers in under two minutes. | Library |
| J2 | Open a 900-page EPUB 3 and reach the last-read position in under 400 ms. | Reader |
| J3 | Highlight a passage on the phone; see the highlight on the laptop within seconds. | Reader + Sync |
| J4 | Adjust font, size, margins, line height, and theme; pagination reflows without losing position. | Reader |
| J5 | Open a malformed EPUB from an untrusted source; it renders as well as possible and cannot touch the filesystem. | Reader + Security |
| J6 | Fix a broken `<nav>` document and a stylesheet in an EPUB, validate, and save without corrupting the package. | Editor |
| J7 | Read a scanned 300 MB PDF with smooth pinch-zoom on a mid-range Android phone. | Reader |
| J8 | Import a DRM-free MOBI purchased years ago and read it with the same typography controls as EPUB. | Ingest + Reader |
| J9 | Export an EPUB to AZW3 on the desktop, copy it to a Kindle over USB, and have it appear under Books with a working ToC and cover. | Conversion |
| J10 | Export the same book to AZW3 on a phone and hand it to Files, Drive, or a USB-C drive via the share sheet. | Conversion |

---

## 3. Bounded contexts

Fifteen contexts. Each maps one-to-one onto a crate in §4.1, and the names are the same in both
places so there is never a translation step between the spec and the tree.

| # | Context | Crate | Responsibility |
|---|---|---|---|
| 1 | **Catalog** | `futhark-catalog` | Holdings, collections, series, search, dedup, cover cache. |
| 2 | **Ingest** | `futhark-ingest` | Watched folders, sniffing, format detection, integrity checks, quarantine. |
| 3 | **Codecs** | `futhark-{epub,mobi,pdf}` | Per-format readers and writers. The surface that touches hostile bytes. |
| 4 | **Document Model** | `futhark-doc` | The normalized in-memory representation all surfaces consume. |
| 5 | **Navigation** | `futhark-nav` | Spine traversal, CFI/locators, page mapping, position stability. |
| 6 | **Rendering** | `futhark-render` | Content presentation, font resolution, image decode, PDF raster. |
| 7 | **Editor** | `futhark-editor` | Package editing, source editing, live preview, validation, undo. |
| 8 | **Annotations** | `futhark-annot` | Highlights, notes, bookmarks, anchoring and re-anchoring. |
| 9 | **Storage** | `futhark-store` | SQLite catalog, blob store, file layout, migrations. |
| 10 | **Progress** | `futhark-stats` | Reading position, sessions, streaks, per-book and library-wide statistics. |
| 11 | **Sandbox** | `futhark-sandbox` | Untrusted-content isolation, CSP, resource policy, script neutralization. |
| 12 | **Presentation** | shell + `futhark-core` | Themes, typography settings, TTS, screen-reader semantics, reduced motion. |
| 13 | **Sync** | `futhark-sync` | CRDT replication of annotations and reading position via AppThere Cloud. |
| 14 | **Metadata** | `futhark-meta` | OPDS clients, Open Library / Google Books lookup, cover fetch, ISBN matching. |
| 15 | **Conversion** | `futhark-convert` | Cross-format transformation and export. Owns EPUB to KF8 compilation, CSS downconversion, and loss reporting. |

Conversion is a full context rather than a writer inside Codecs because EPUB to KF8 is a
compilation step, not a serialization step: it flattens a multi-document spine into a single
blob with a skeleton/fragment index, and downconverts CSS to a narrower subset. That is a
transformation with its own fidelity model and its own failure modes.

### 3.1 Dependency direction

```
Ingest ──▶ Codecs ──▶ Document Model ──▶ Navigation ──▶ Rendering
              │              │                │             ▲
              │              │                ▼             │
              │              │          Annotations ────────┘
              ▼              ▼
           Editor        Storage ◀── {Catalog, Annotations, Progress, Sync}
                            ▲
                         Catalog ◀── Metadata

Sandbox      wraps Rendering and the Editor preview          (cross-cutting)
Presentation feeds Rendering and Navigation                  (cross-cutting)
Sync         replicates {Annotations, Progress} only — never document bytes
```

Document Model is the waist of the hourglass. Nothing above it knows what format a book was.

---

## 4. Architecture

### 4.1 Crate layout

```
futhark/
├─ crates/
│  ├─ futhark-catalog/        # Catalog: library model, search, collections
│  ├─ futhark-ingest/         # Ingest: detection, sniffing, watched folders
│  ├─ futhark-doc/            # Document Model: normalized IR, spine, resources
│  ├─ futhark-epub/           # Codecs: EPUB 2/3 read + write
│  ├─ futhark-mobi/           # Codecs: PalmDB/MOBI/KF8 read + write (bespoke)
│  ├─ futhark-pdf/            # Codecs + Rendering: hayro integration
│  ├─ futhark-nav/            # Navigation: locators, CFI, pagination map
│  ├─ futhark-render/         # Rendering: raster pipeline, tile cache, font resolution
│  ├─ futhark-annot/          # Annotations: annotation model + anchoring
│  ├─ futhark-store/          # Storage: SQLite, migrations, blob store
│  ├─ futhark-stats/          # Progress: sessions, progress
│  ├─ futhark-sandbox/        # Sandbox: sanitization, CSP policy, resource gate
│  ├─ futhark-editor/         # Editor: package mutation, validation, undo
│  ├─ futhark-sync/           # Sync: Loro CRDT + AppThere Cloud client
│  ├─ futhark-meta/           # Metadata: OPDS, metadata providers
│  ├─ futhark-convert/        # Conversion: EPUB→KF8 compiler, CSS downconvert, loss report
│  └─ futhark-core/           # Shared types, errors, config, i18n
├─ shell/                     # Presentation shell (see §6)
├─ conformance/               # appthere-conformance suite for Futhark
└─ patches/
```

Every crate from `futhark-catalog` through `futhark-meta` is **UI-framework-agnostic**. They
compile with zero dependency on the shell. This is enforced in CI (ADR-F002) and is what makes
§6's decision reversible.

### 4.2 Shared AppThere dependencies

| Crate | Use |
|---|---|
| `appthere-color` | ICC handling for PDF and image-heavy fixed-layout books. |
| `appthere-conformance` | Golden-image regression harness, promoted from `loki-acid`. |
| `appthere-canvas` | Reused only if the Dioxus Native path is chosen (§6). |
| `appthere-file-access` | Android (and possibly iOS) file I/O. Promoted from Loki's `loki-file-access`; Futhark is its second consumer. See D11. |
| Loki document model | **Not** reused. EPUB's model is XHTML+CSS, not Loki's paragraph IR. See ADR-F003. |

---

## 5. Format support matrix

| Format | Read | Edit | Library metadata | Crate strategy |
|---|---|---|---|---|
| EPUB 3.x | Full | Full | Full | `rbook` evaluated; likely bespoke on `quick-xml` + `zip` |
| EPUB 2.0.1 | Full | Full | Full | Same, with NCX/OPF 2 paths |
| MOBI / PRC (KF7) | Full | No | Full | Bespoke `futhark-mobi`. Write only via dual-format opt-in. |
| AZW3 / KF8 | Full | Via export | Full | Bespoke `futhark-mobi` (container) + `futhark-convert` (compile) |
| PDF | Full render, no edit | No | Embedded XMP/Info | `hayro` |

"Edit" means in-place structural editing. KF8 is not edited in place — it is *produced*, by
compiling from EPUB. A user who wants to change an AZW3 edits the EPUB and re-exports. This is
the same model Calibre and Sigil use, and it avoids maintaining a mutation path over a compiled
container format.

### 5.1 EPUB

`rbook` 0.7.x is the strongest crate in the ecosystem — fast, format-agnostic traits, and it
already claims read/build/edit for EPUB 2 and 3. It is the default starting point for
`futhark-epub`. The risk is that an *editor* needs lossless round-tripping: preserving comments,
attribute order, whitespace, and unknown elements so that saving a file the user did not touch
produces byte-identical output. Few parsers are built for that. Spike F2 (§10) decides whether
`rbook` is adopted, wrapped, or replaced by a bespoke reader over `quick-xml`.

### 5.2 MOBI / KF8 — reading

No maintained Rust crate covers KF8 properly. The `mobi` crate handles PalmDOC headers and
metadata but stalled around 2022 and does not do KF8. `libmobi` (C) is the reference
implementation but is a C dependency, which conflicts with the `#![forbid(unsafe_code)]` standard.

**Decision:** write `futhark-mobi` in safe Rust, using `palmdoc-compression` for the LZ77 layer and
the MobileRead wiki + KindleUnpack as the format reference. KF8 is structurally a compiled EPUB —
a skeleton/fragment index over an XHTML blob — so once unpacked it feeds the same Document Model IR
as EPUB, and display comes free: the content is XHTML and renders through the identical pipeline.
No second renderer, no second typography stack.

Read scope covers PalmDB containers, Record 0 (PalmDOC header, MOBI header, EXTH block, full name),
PalmDOC-compressed and HUFF/CDIC-compressed text records, FDST, the skeleton and fragment indexes,
NCX indexes, image records, and CONT/CRES high-DPI resources. Dual KF7+KF8 files are detected via
the BOUNDARY record and the KF8 half is preferred. Note the asymmetry: Futhark **reads** dual-format
files because they exist in the wild, but never **writes** one.

### 5.3 KF8 / AZW3 — writing

This is the Kindle interoperability story, and it replaces KFX entirely.

**Why this is tractable where KFX is not.** KF8 is documented on the MobileRead wiki, was
reverse-engineered over a decade ago, and — critically — is a *static target*: Amazon stopped
evolving it when KFX arrived. There is no symbol-table churn to chase. `kindling` is an existence
proof that a pure-Rust KF8 writer works, emitting KF8-only AZW3 by default and dual KF7+KF8 behind
a legacy flag. Writing a DRM-free KF8 file circumvents nothing, so it carries none of the §1201
exposure that reading store-purchased Kindle files does.

**Why it is worth building at all**, given that Send to Kindle now accepts EPUB and converts
server-side: sideloaded AZW3 over USB files under **Books** rather than **Docs**, works with no
account and no network, and keeps the file under the user's control rather than round-tripping it
through Amazon's servers. That is a real difference for a local-first library manager, and it is
the only part of the Kindle path Amazon's own service does not already cover.

**Export runs on all five platforms** (ADR-F031). The compiler is pure Rust with no platform
surface, so parity costs nothing in the pipeline itself and buys a single implementation, a single
conformance suite, and no `cfg` divergence. On mobile the user takes delivery through the share
sheet — into Files, Drive, Dropbox, or a USB-C drive — rather than a save dialog. Futhark's job
ends at producing a correct file; where it goes next is the user's business, and refusing to
produce one on a phone would be an arbitrary restriction rather than a technical one.

**The pipeline** (Conversion context, `futhark-convert`):

1. Normalize the source to the Document Model IR.
2. Flatten the spine into a single XHTML blob, recording fragment boundaries.
3. Build the skeleton and fragment indexes over that blob.
4. Downconvert CSS to the KF8 subset, emitting a structured loss report (ADR-F028).
5. Emit images as records; patch JFIF headers on the cover for Kindle compatibility.
6. Build the NCX index from the EPUB nav document or NCX.
7. Assemble Record 0: PalmDOC header, MOBI header, EXTH metadata, full name.
8. Compress text into PalmDOC records — each non-final record must decompress to exactly 4096
   bytes to match the declared `text_record_size`. This constraint is easy to violate and produces
   files that fail silently on device; it is a conformance test, not a code comment.
9. Emit KF8-only. No KF7 half, no BOUNDARY record, ever (ADR-F027).

**What is deliberately not done:** no DRM is written, ever (ADR-F021). No ASIN is fabricated to
unlock X-Ray or Goodreads integration — the Calibre KFX Output workflow does this, and writing
false identifiers into a user's file to trick a vendor's device is not something Futhark will do
on the user's behalf (ADR-F029).

**What users lose versus KFX:** Enhanced Typesetting, Page Flip, and the KFX-era layout features
are unavailable in KF8 and always will be. Product copy should say so plainly rather than let
users infer parity.

### 5.4 PDF

`hayro` (Laurenz Stampfl) is the clear choice: the most feature-complete pure-Rust PDF rasterizer,
Apache-2.0, `#![forbid(unsafe_code)]`, 1000+ file regression suite scraped from the pdf.js and
PDFBOX suites, and a `hayro-interpret` `Device` trait that lets Futhark emit into its own backend
rather than only bitmaps. It also lines up with the `krilla` work already in the Loki server spec —
same author, same lineage.

Known gaps to plan around: no password-protected/encrypted PDF loading, no blend modes or knockout
groups, non-embedded CID fonts unsupported, and performance has explicitly not been optimized yet.
Encrypted-PDF support is the one that will generate user reports on day one (see R7).

---

## 6. Presentation shell evaluation: Tauri 2 vs. Dioxus Native

This is the decision the rest of the program hangs on. It is evaluated on its own merits for
Futhark, not inherited from Loki.

### 6.1 Why Futhark's calculus differs from Loki's

Loki chose Dioxus Native / Blitz / Vello / Parley for a defensible reason: **OOXML and ODF have no
native runtime.** No platform ships a `.docx` layout engine. Building one on Vello was not a
preference, it was the only path to fidelity Kevin controls.

Futhark's primary format inverts that. **EPUB *is* XHTML plus CSS.** Every platform ships a mature,
heavily-tested, standards-compliant engine for exactly this content, with twenty-five years of
compatibility work baked in. Choosing to reimplement it is a fundamentally different bet than
choosing to implement OOXML, because in the EPUB case a correct implementation already exists on
every target device.

That single asymmetry drives most of what follows.

### 6.2 Framework status (verified July 2026)

**Tauri 2** — stable since October 2024; current line 2.10.x (2.10.1, March 2026). Mobile ships from
the same Rust core: WKWebView on iOS, Android System WebView on Android, minimum iOS 9 / Android 8
(API 26). The team has been explicit that 2.0 was *not* the "mobile as a first-class citizen"
release — it is a solid foundation, with some desktop plugins still unported. Capability-based
permission model replaced v1's allowlist. Typical bundles 3–8 MB, resident memory roughly half of
Electron.

**Dioxus Native / Blitz** — Kevin's existing Loki stack: Stylo for CSS, Taffy for layout, Vello +
wgpu for paint, Parley for text. Android viable per the Loki Tier 1 analysis; iOS blocked on
Blitz-on-Metal validation. Spike G1 (multi-touch gesture support on the Blitz Android timeline)
remains open and unresolved.

### 6.3 Criterion-by-criterion

**EPUB 3 CSS and layout fidelity.** The single largest differentiator. Real-world EPUBs lean on
CSS multi-column (the standard mechanism for paginated reflow), floats, tables, `writing-mode:
vertical-rl` and ruby annotations for CJK, MathML for STEM titles, SVG, and embedded fonts. Blitz's
Stylo integration gives real cascade and parsing, but layout coverage across multicol, floats,
vertical writing modes, and ruby is incomplete, and none of it is on Kevin's roadmap to fix.
Webviews cover all of it today. *Tauri: 5, Dioxus Native: 2.*

**Editor implementation cost.** EPUB editing is XHTML/CSS editing. In a webview, CodeMirror 6 gives
syntax highlighting, folding, linting, and multi-cursor for free, with a live preview that is
literally the same engine as the reader. Outside a webview, every one of those is bespoke on top of
Parley. Sigil and Calibre's editor both exist because this problem is large.
*Tauri: 5, Dioxus Native: 2.*

**Cross-platform reach.** Tauri ships all five targets now. Dioxus Native ships desktop and
Android; iOS is blocked indefinitely on Blitz-on-Metal. Note the irony: iOS *mandates* WKWebView for
web content anyway, so on that platform the webview is not a compromise, it is the only option.
*Tauri: 5, Dioxus Native: 2.*

**Rendering determinism.** WKWebView, WebView2, and WebKitGTK are three text layout engines with
three sets of bugs. Write once, test three times. Identical pagination across devices — the property
that makes "page 214 of 480" and annotation anchoring stable — is not achievable on webviews without
substantial per-platform correction. Vello + Parley produce byte-identical output everywhere; Loki
already proved this with `vello_cpu` goldens. *Tauri: 2, Dioxus Native: 5.*

**Untrusted content security.** This is Tauri's real weakness and it deserves more than a line.
Futhark opens arbitrary files from the internet. EPUB 3 permits scripted content. A webview
rendering a hostile EPUB, in a process holding an IPC bridge to a Rust backend with filesystem
access, is a privilege-escalation surface. It is defensible — sandboxed iframe on a distinct
origin, strict CSP, Tauri's isolation pattern, script stripping at ingest — but it must be designed
deliberately and audited, not assumed (see ADR-F005, ADR-F006, R2). Dioxus Native has no JS engine
at all: scripted EPUB content simply does not execute, and the threat surface collapses to parser
memory safety, which safe Rust already covers. *Tauri: 2, Dioxus Native: 5.*

**AppThere stack reuse.** Dioxus Native reuses `appthere-canvas`, the shared `vello::Renderer`,
`FontResources`, the render-cache tiering, the memory work that took Loki from 2.83 GB to 750 MB,
and the `appthere-conformance` golden harness. Tauri reuses essentially none of the presentation
stack — though it reuses every crate in §4.1, which is most of the actual program.
*Tauri: 1, Dioxus Native: 5.*

**Touch and gesture.** A reader is almost entirely gestural: swipe to page, pinch to zoom,
long-press to select, drag handles to extend selection. Webviews get all of this natively.
Spike G1 flags multi-touch as unvalidated on Blitz Android — a moderate risk for Loki's timeline UI,
a critical-path risk for Futhark, where gesture *is* the interface. *Tauri: 4, Dioxus Native: 2.*

**Footprint.** Tauri: 3–8 MB bundles, ~45 MB RSS typical; no bundled Chromium. Dioxus Native: no
webview process, tighter control, and Loki's memory discipline transfers directly.
*Tauri: 3, Dioxus Native: 4.*

**PDF integration.** `hayro` renders in Rust in both worlds, so the delta is only transport. Under
Dioxus Native, `hayro-interpret`'s `Device` can emit straight into a Vello scene — no rasterize,
no copy, and zoom re-renders at native resolution. Under Tauri, tiles must be rasterized and handed
to the webview; done via a custom URI scheme serving WebP tiles (never base64 over IPC — see
ADR-F008) this is fine, but it is a copy and a re-encode. *Tauri: 3, Dioxus Native: 5.*

**Long-term dependency control.** Tauri's risk is three divergent webviews Kevin does not control
and cannot patch. Dioxus Native's risk is Blitz's roadmap, which Kevin also does not control but
*can* patch — the `patches/` directory already carries fixes for `blitz-dom`, `blitz-shell`, and
`fontique`. *Tauri: 3, Dioxus Native: 4.*

### 6.4 Weighted score

| Criterion | Weight | Tauri 2 | Dioxus Native |
|---|---:|---:|---:|
| EPUB 3 CSS/layout fidelity | 20% | 5 | 2 |
| Editor implementation cost | 12% | 5 | 2 |
| Cross-platform reach (incl. iOS) | 12% | 5 | 2 |
| Untrusted-content security | 12% | 2 | 5 |
| Rendering determinism | 10% | 2 | 5 |
| AppThere stack reuse | 10% | 1 | 5 |
| Touch/gesture input | 8% | 4 | 2 |
| Footprint | 6% | 3 | 4 |
| PDF integration path | 5% | 3 | 5 |
| Dependency control | 5% | 3 | 4 |
| **Weighted total** | **100%** | **3.54** | **3.33** |

### 6.5 Recommendation

**Adopt Tauri 2 for Futhark's presentation shell.** (ADR-F001)

The margin is thin — 3.54 to 3.33 — and it should be read as *thin*, not decisive. The
recommendation rests on one argument, and it is worth stating plainly:

> Futhark's fidelity requirement is CSS conformance, and CSS conformance is a decades-deep problem
> that already has a correct, free, per-platform implementation. Reimplementing it is not a
> differentiator for an ebook reader — it is a tax. Loki's situation was the opposite, which is why
> Loki's answer was the opposite.

For a solo developer, the cost side is decisive even where the score is close. The Dioxus Native
path implies owning paginated CSS multicol layout, vertical writing modes, ruby, MathML, text
selection over a paginated flow, IME, and a source-code editor — before the first user opens a book.
That is a multi-year project sitting on the critical path of *every* feature.

**Sensitivity.** This flips under two conditions, either of which is sufficient:

1. **iOS drops from v1** and platform-identical typography is elevated to a product requirement
   (e.g. Futhark's differentiator becomes "the reader whose typography is better than everyone
   else's"). Re-weighting determinism to 20% and dropping reach to 4% gives Dioxus Native 3.63
   to Tauri's 3.24.
2. **Spike F1 fails** — see §10. If a webview cannot deliver stable, position-preserving
   pagination via CSS multicol across all three engines, Tauri's headline advantage evaporates,
   because paginated reflow *is* the reader.

Spike F1 therefore gates ADR-F001, and ADR-F001 is provisional until F1 returns.

**The hedge, and it is a real one.** §4.1 keeps every domain crate free of UI dependencies, enforced
in CI. If F1 fails, or if Blitz's CSS coverage closes the gap in eighteen months, the shell is
replaceable without touching the codecs, the IR, the catalog, the annotation model, or sync — which
is roughly 80% of the program by volume. Do not let shell types leak downward. That rule is what
buys the option.

**What is genuinely lost.** Be honest about it: the Loki Android multi-instance work
(multiprocess + Loro relay) does not transfer, the shared `vello::Renderer` and `FontResources`
memory work does not transfer, and the `appthere-canvas` extraction does not get a second consumer.
Futhark becomes the first AppThere app on a different presentation stack, and the suite loses some
architectural uniformity. That is a genuine cost, and "consistency with the rest of the suite" is a
legitimate reason to overrule this recommendation — it just is not, on the evidence, a technical one.

### 6.6 Rejected alternatives

| Option | Why rejected |
|---|---|
| Hybrid: Dioxus Native chrome + webview content viewport | Two toolkits, two input models, two font stacks, two theming systems, and the security boundary lands in the hardest place. Worst of both. |
| Electron | Bundle size, memory, and no Rust-native core. Non-starter for a suite built on Rust. |
| Kotlin Multiplatform + Compose | Strong on mobile, weak on desktop Linux, and abandons the Rust core entirely. |
| Flutter | Own text stack, no EPUB advantage over Blitz, and non-Rust. Same reimplementation tax without the AppThere reuse. |
| Native per-platform UI (SwiftUI/Compose/GTK) | Four UIs for a solo developer. CommerceKit can afford this because its clients are thin; Futhark's client is the product. |

---

## 7. Shell architecture (conditional on ADR-F001)

### 7.1 Process and origin model

```
┌─────────────────────────────────────────────────────────────┐
│ Rust core (Tauri backend)                                   │
│  futhark-* crates · SQLite · hayro · codecs · Loro sync     │
└───────────┬───────────────────────────┬─────────────────────┘
            │ Tauri IPC (commands)      │ custom URI schemes
            │ capability-gated          │ (no IPC bridge bound)
┌───────────▼───────────────┐  ┌────────▼────────────────────┐
│ App origin                │  │ Content origin              │
│  tauri://localhost        │  │  futhark-content://<book-id>│
│  Svelte UI: library,      │  │  sandboxed iframe           │
│  chrome, settings,        │  │  strict CSP, no scripts,    │
│  editor source pane       │  │  no network, no __TAURI__   │
└───────────────────────────┘  └─────────────────────────────┘
```

The content origin is the security boundary (Sandbox). Book resources are served from the Rust side
through a registered asynchronous URI scheme, so relative `href`s, `@font-face`, and stylesheet
imports resolve naturally — the fidelity benefit and the security benefit come from the same
mechanism. The content iframe never has `window.__TAURI__` bound.

### 7.2 Frontend stack

Svelte, consistent with CommerceKit's web tier. Semantic HTML5, no component library, CSS Grid
layout. CodeMirror 6 for the editor source pane. No client-side routing framework.

### 7.3 PDF transport

`hayro` rasterizes tiles in Rust. Tiles are encoded WebP and served over
`futhark-pdf://<doc-id>/<page>/<z>/<x>/<y>`, cached in the Rust process with the same three-tier
Hot/Warm/Cold policy Loki uses in `loki-render-cache`. Never base64 over IPC.

---

## 8. Architecture Decision Records

| ID | Decision | Status |
|---|---|---|
| **ADR-F001** | Tauri 2 is the presentation shell for all five targets. | **Provisional — gated on Spike F1** |
| **ADR-F002** | All `futhark-*` domain crates are UI-framework-agnostic; CI fails on any shell dependency in a domain crate. | Accepted |
| **ADR-F003** | Futhark does **not** reuse Loki's document IR. EPUB's model is XHTML+CSS; forcing it through a paragraph-oriented IR is lossy and pointless when the renderer consumes XHTML anyway. | Accepted |
| **ADR-F004** | Document Model IR is a thin normalization layer — spine, resource map, ToC, metadata — over format-native content, not a universal document model. Content stays in its native form as far down as possible. | Accepted |
| **ADR-F005** | Book content renders in a sandboxed iframe on a distinct custom-scheme origin with no IPC binding. | Accepted |
| **ADR-F006** | EPUB scripted content is stripped at ingest by default. A per-book opt-in exists but remains sandboxed and network-denied. Futhark is a reader, not a browser. | Accepted |
| **ADR-F007** | Book resources are served via a registered async URI scheme, never inlined into the DOM or passed over IPC. **The paginator ships from the resource server as part of the served document** — an opaque-origin frame exposes no `contentDocument` for the app origin to inject into (Spike F1). | Accepted |
| **ADR-F008** | PDF tiles are WebP over a custom scheme, not base64 over IPC. | Accepted |
| **ADR-F009** | `hayro` for all PDF parsing and rasterization. No PDFium, no MuPDF, no C dependency. | Accepted |
| **ADR-F010** | `futhark-mobi` is bespoke safe Rust, bidirectional (read and write). No `libmobi` FFI. | Accepted |
| **ADR-F011** | EPUB reader/writer starts from `rbook`; adoption confirmed or reversed by Spike F2 on round-trip fidelity. | Provisional |
| **ADR-F012** | Editor saves must be byte-lossless for untouched files. Comments, attribute order, whitespace, and unknown elements are preserved. | Accepted |
| **ADR-F013** | Paginated reflow uses CSS multi-column inside the content iframe, with scroll-offset paging. **Exception: `writing-mode: vertical-rl` is not paginable this way** and falls back to scroll mode (ADR-F034). | **Accepted** — Spike F1 returned |
| **ADR-F014** | Reading position is stored as EPUB CFI where available, with a text-anchored fallback (prefix/suffix quote matching) for MOBI and malformed EPUBs. | Accepted |
| **ADR-F015** | Annotations anchor on the same locator scheme as position, and re-anchor by quote matching when the underlying document changes. **Annotation text is always taken from `Range.toString()`, never `Selection.toString()`** — they diverge across table boundaries on both engine families (Spike F1), which would silently break re-anchoring. | Accepted |
| **ADR-F016** | SQLite via `sqlx` for the catalog. Blobs (covers, extracted resources) live on the filesystem, referenced by content hash. Not in the database. | Accepted |
| **ADR-F017** | Books are referenced in place by default. Futhark does not restructure the user's folders unless "managed library" is explicitly enabled. | Accepted |
| **ADR-F018** | Sync (Sync) replicates annotations, bookmarks, and reading position only — never document bytes. Loro CRDT over the AppThere Cloud relay, reusing Loki's transport. | Accepted |
| **ADR-F019** | Sync is optional and off by default. Futhark is fully functional with no account. | Accepted |
| **ADR-F020** | Deduplication is by content hash first, then by ISBN, then by fuzzy title+author. Never automatic deletion — duplicates are surfaced, not resolved. | Accepted |
| **ADR-F021** | Futhark implements no DRM circumvention of any kind, for any format, and ships no code path that assists it. DRM-protected files report as such and stop. | Accepted — non-negotiable |
| **ADR-F022** | **KFX is out of the program in both directions.** No reader, no writer, no bridge to Kindle Previewer. Rationale in §5.3 and D6. | Accepted |
| **ADR-F023** | Metadata providers (Metadata) are opt-in per lookup. No background network calls without explicit user action. | Accepted |
| **ADR-F024** | TTS uses platform speech APIs via Tauri plugins, not a bundled engine. | Accepted |
| **ADR-F025** | Conformance goldens use `vello_cpu`-equivalent determinism where possible; where the webview renders, goldens are per-engine with an explicit tolerance budget. | Accepted |
| **ADR-F026** | Untrusted-input parsing (Codecs) is fuzzed in CI. Every codec crate gets a `cargo-fuzz` target from the day it is created, not retrofitted. | Accepted |
| **ADR-F027** | KF8 export emits KF8-only AZW3. Dual KF7+KF8 output is **not** built (D9). Reading dual-format files is unaffected. | Accepted |
| **ADR-F028** | CSS downconversion to the KF8 subset produces a structured, user-visible loss report. Export never silently degrades a book. | Accepted |
| **ADR-F029** | Futhark never fabricates ASINs or other vendor identifiers to unlock device features. Exported metadata reflects the actual book. | Accepted |
| **ADR-F030** | KF8 export round-trips through the conformance harness: every exported AZW3 is re-parsed by `futhark-mobi` and compared against the source IR before the file is handed to the user. | Accepted |
| **ADR-F031** | Export ships on all five platforms at parity. `futhark-convert` contains no platform-conditional code; one implementation, one conformance suite, no feature flags. | Accepted |
| **ADR-F032** | File delivery is abstracted behind a `DeliverTarget` trait with two implementations: native save dialog on desktop, and `appthere-file-access` plus share-sheet handoff on mobile. The compiler never touches a path. | Accepted |
| **ADR-F033** | `loki-file-access` is promoted to `appthere-file-access` rather than vendored or forked, following the `loki-acid` → `appthere-conformance` and `appthere-canvas` precedents. Futhark is its second consumer, which is the justification for promotion. | Provisional — see D11 |
| **ADR-F034** | Vertical writing modes are detected at ingest and rendered in **scroll mode, never paginated**. Detection is content-based, not language-based: the horizontal-CJK control in Spike F1 paginates cleanly on both engines, so this is a writing-mode constraint and must not be applied to CJK generally. | Accepted |
| ~~**ADR-F039**~~ | ~~Host-sources emitted first in CSP source lists.~~ **RETRACTED at 0.8.0.** The WebKit source-ordering bug it encoded did not exist: the probe read `img.complete` synchronously with no `load`/`error` listener, so an in-flight image read as blocked and the slower engine looked stricter. Apparent ordering-dependence was cache warmth across sequential runs. | **Retracted** |
| **ADR-F039a** | The content CSP **names the content origin explicitly and never relies on `'self'`**. `'self'` matches nothing in an opaque-origin frame — WebKit enforces this correctly; Chromium is leniently permissive. Spec-derivable once the opaque origin is accounted for, and it does not expire when an engine changes. | Accepted |
| **ADR-F039b** | `'none'` is **exclusive by construction**. A builder that appends a source to a directive already set to `'none'` produces a malformed list that both engines resolve permissively, in either order — silently opening the directive. The builder must make this unrepresentable. | Accepted |
| **ADR-F045** | **Cross-engine agreement is not evidence of correctness.** Every CSP assertion is checked against intent, not only against engine consensus. Both engines resolve `'none'` alongside other sources permissively (ADR-F039b) — a shared leniency that a disagreement-only rule cannot see, and the existence proof that consensus can be uniformly wrong. | Accepted |
| **ADR-F040** | CSP behaviour is **validated per engine in the conformance suite**, never reasoned about from the specification. The original justifying example was retracted; the principle is better evidenced by the retraction than it was by the finding, since the real divergence (`'self'` under an opaque origin) still required measurement to surface, and the false one was only caught by re-measuring. | Accepted |
| **ADR-F036** | Round-trip divergence is **classified before it is counted**: (a) entry content, (b) archive container metadata, (c) declared-vs-actual mismatch. Only (a) bears on ADR-F012. A reader that reorders zip entries but preserves every entry byte-for-byte satisfies the lossless-save promise. | Accepted |
| **ADR-F037** | The conformance corpus is **two-tier**. Tier A is redistributable and lives in the repo (W3C/IDPF `epub-tests`, Standard Ebooks, a Project Gutenberg sample, synthetic adversarial fixtures). Tier B is the developer's own library, **never committed** — referenced by a hash manifest only. CI runs Tier A and skips Tier B without failing. | Accepted |
| **ADR-F038** | For round-trip spikes the **published artifact is the result manifest**, not the books: content hash, producer string, divergence class, classifier version, and diff summary per file. This is what makes Tier B usable without redistributing a single copyrighted byte. | Accepted |
| **ADR-F042** | Every sandbox assertion must carry a **positive liveness proof**. A probe that cannot report is not a probe that found nothing — the mechanism under test can suppress the report. Absence of a failure signal scores as *unobservable*, never as a pass, and each result is nonce-keyed to the policy that produced it. | Accepted |
| **ADR-F043** | **No synchronous check of an asynchronous outcome.** Every resource-load assertion settles on `load`/`error`. This single error produced the retracted ADR-F039 and one of the three false findings in F1c; it is a harness invariant, not a style note. | Accepted |
| **ADR-F044** | The **`http` control is permanent conformance infrastructure**, not spike scaffolding. It caught three false findings before any Mac time was spent, all of which would have been "confirmed" on the target platform by the same artifact. It runs in CI alongside the engine suites for as long as the sandbox architecture stands. | Accepted |
| **ADR-F047** | F2 on a normalized corpus is a **screening test, not a qualifying test**, and the asymmetry is recorded in the findings. A manager's writer normalizes exactly the constructs that break parsers, so such a corpus under-represents the hard cases: a **failure is decisive** (rbook cannot handle even the easy population — reject it), a **pass is uninformative** (ADR-F011 stays open pending an un-normalized corpus). Never read a screening pass as adoption. | Accepted |
| **ADR-F046** | The manifest records **`normalized-by` as an axis separate from `producer`**. A file rewritten by a library manager reports that manager as its producer, erasing the original toolchain. Tier B's value is in-the-wild producer diversity, so a corpus that has passed through one management tool measures that tool's writer, not the ecosystem. D12's coverage gate counts producers among files with no normalization signal. | Accepted |
| **ADR-F041** | The divergence **taxonomy is frozen before Tier B runs**. Tier B may add a new named class for genuinely unclassifiable cases; it may never silently move an existing class boundary. Otherwise the instrument is fitted to the data it is measuring and the result means nothing. Classifier version is recorded per manifest entry so any run is reproducible. | Accepted |
| **ADR-F035** | Reading progress is reported in **device-independent locators**, not page numbers. Page numbers are per-device and explicitly labelled as such in the UI. Spike F1 measured 2.99% mean page-count divergence between engine families (6.45% max), which is tolerable for display and unacceptable as an anchor. | Accepted |

---

## 9. Risk register

| ID | Risk | Sev | Likelihood | Mitigation |
|---|---|---|---|---|
| **R1** | ~~CSS multicol pagination unstable across engines.~~ **Largely retired by Spike F1**: page counts 100% deterministic per settings tuple across 192 doc/tuple pairs × 3 loads on both engine families; position preserved on 100% of resize and font-change samples; selection usable in every case including drags across a column boundary. | Low | Low | Residual risk is WKWebView, which F1 could not exercise. See R21 and Spike F1b. |
| **R2** | Hostile EPUB escapes the content sandbox and reaches the Rust backend. | **Critical** | Low | ADR-F005/F006/F007. External security review before public release. Threat model as a Spec 01 deliverable. |
| **R3** | Users expect Kindle-library support and find Futhark opens none of their purchased KFX books. Dropping KFX removes the code risk, not the expectation. | High | **High** | Product copy states the Kindle story as "export to your Kindle", never "read your Kindle library". Surface a clear, non-apologetic message on encountering KFX. |
| **R4** | `futhark-mobi` KF8 parsing is a larger effort than estimated; MOBI slips out of v1. | High | Medium | Phase-gate it. MOBI is Phase 4, not Phase 1. Ship EPUB-only if needed. |
| **R5** | `rbook` cannot round-trip losslessly; `futhark-epub` becomes a bespoke build, adding a phase. | High | Medium | Spike F2 in Phase 0. |
| **R6** | Blitz's CSS coverage closes the gap and the Tauri decision looks wrong in retrospect. | Medium | Low | ADR-F002 keeps the shell replaceable. Re-evaluate at Phase 5. |
| **R7** | Encrypted/password-protected PDFs cannot be opened (`hayro` limitation). Common in library-loan and enterprise documents. | High | **High** | Detect and report clearly. Upstream contribution is the only real fix; scope it as a possible Phase 6 item. |
| **R8** | `hayro` performance on large scanned PDFs is inadequate on mobile — performance is explicitly not yet optimized upstream. | High | Medium | Benchmark in Spike F3. Aggressive tile caching, background pre-render, downsampled proxies. |
| **R9** | Webview memory on mobile with a large book open triggers Android LMK / iOS jetsam. | Medium | Medium | Spine-window loading; never load the whole book into one document. Reuse Loki's memory-tracking harness. |
| **R10** | Text selection across a multicol-paginated iframe is unreliable, breaking highlights. | High | Medium | Fold into Spike F1 acceptance criteria — selection is not a separate concern from pagination. |
| **R11** | Library-wide full-text search over 10k books needs an index Futhark has no story for. | Medium | Medium | Deferred to Phase 5. Evaluate `tantivy` then. |
| **R12** | iOS App Store rejection: readers that reach external stores trigger IAP rules; sideloading affordances may draw scrutiny. | Medium | Low | No purchase paths in v1 (§1.2). Export is local-file-out only, no store interaction. |
| **R13** | `writing-mode: vertical-rl` is **not paginable via multicol at all**. Spike F1 measured WebKitGTK 2.50 reporting a single page while stranding ~80% of the chapter past the last reachable scroll position — **silent content loss**, not degraded rendering. A horizontal-CJK control with identical text paginates cleanly on both engines, isolating this to the writing mode rather than the script. | **High** | Confirmed | ADR-F034: detect at ingest, force scroll mode. Never paginate vertical text. Vertical and horizontal CJK both stay in the conformance corpus from Phase 1 so the distinction cannot regress into a blanket CJK exclusion. |
| **R14** | Annotation re-anchoring fails after a user edits a book they have annotated — a workflow only Futhark creates, by shipping reader and editor together. | Medium | **High** | Design for it explicitly in Annotations. Quote-based fallback (ADR-F015) plus a visible "orphaned annotation" state rather than silent loss. |
| **R15** | Amazon further restricts sideloading, or drops AZW3 support on new devices, stranding the KF8 export path. Send-to-Kindle was already cut for unsupported Kindles in April 2026 and older formats are being phased out. | Medium | Medium | Keep EPUB export as the primary Kindle path (Send to Kindle accepts it). KF8 export is an enhancement, never the only route. Monitor device support each Phase gate. |
| **R16** | KF8 export produces files that appear to work but fail subtly on device — bad ToC, missing cover, mis-sized text records. Failures are silent and only visible on hardware. | High | Medium | ADR-F030 round-trip verification, plus a physical-device test matrix (Paperwhite, Scribe, Kindle app) as a Phase 6 exit criterion. |
| **R17** | CSS downconversion loses layout fidelity badly enough on complex books that users blame Futhark rather than the format ceiling. | Medium | Medium | ADR-F028 loss report shown before export completes, with a preview diff for the worst-affected sections. |
| **R18** | Mobile file delivery is harder than desktop parity implies. Tauri's save dialog is still an open enhancement request on both Android and iOS; `open` returns `content://` URIs on Android and `file://` URIs on iOS rather than paths; there is no folder picker on Android and no first-party external-storage permission plugin. | Low | Low | **Largely retired by prior art.** The abandoned Tauri/Lexical Loki Text build already solved Android file I/O, and `loki-file-access` is a working read/write implementation. Futhark consumes it via `appthere-file-access` (D11) rather than rediscovering the problem. Residual risk is iOS coverage and create-new-file flows — see R20. |
| **R21** | WKWebView is unexercised. Spike F1 covered Blink (proxy for WebView2 and Android System WebView) and WebKitGTK 2.50 (the actual Linux Tauri webview), but not WKWebView — which is **macOS as well as iOS**. macOS is in scope for desktop v1, so this gap is not closed by resolving D2. | **High** | Medium | Spike F1b on the MacBook Air. **F1b partially closes this, not fully:** safaridriver drives Safari over http, while Tauri serves the content origin through `WKURLSchemeHandler`. The custom scheme *is* the origin mechanism ADR-F005/F007 rest on, so F1b cannot speak to the sandbox and origin criteria on the configuration Futhark actually ships. See R23. |
| **R24** | ~~CSP under-blocking / fail-open under source permutation.~~ **Answered: no fail-open.** Thirteen policies across Chromium 141 and WebKitGTK 2.52, agreeing on every case — directive order irrelevant, first duplicate wins, an unparseable source does not relax the rest of the list, an unknown directive is ignored without weakening the policy, case and tab variants enforced. `default-src 'none'` holds. | Low | Low | Retained as a standing conformance suite so a future engine cannot regress it silently. |
| ~~**R25**~~ | ~~Host-first ordering is a workaround that could expire.~~ **Moot — ADR-F039 retracted.** | — | — | Superseded by ADR-F039a, which is spec-derived and does not expire. |
| **R26** | **Engine leniency masks CSP errors during development.** Chromium permits `'self'` in an opaque-origin frame where WebKit correctly matches nothing, so a mistake of that class passes on Linux and Windows and fails on macOS and iOS — the platforms with the least coverage (R21, R23). | Medium | Medium | **Symmetric rule: any cross-engine CSP disagreement is a policy bug until explained.** Do not phrase this as "trust the strict engine" — WebKit is spec-correct on `'self'` under an opaque origin, but that is not a general property of the engines, and a case where Chromium is stricter would slip through an asymmetric rule. The matrix already reports both sides. |
| **R23** | Custom-scheme origin semantics under `WKURLSchemeHandler` differ from an `http` origin in ways that could affect opaque-origin behaviour, CSP application, and iframe sandbox enforcement — precisely the criteria the security architecture depends on. F1b's Safari-over-http path cannot detect this. | **High** | Medium | Treat F1b criteria 1–4 as engine evidence and criterion 5 as provisional. Confirm the origin and sandbox criteria in a real Tauri shell on macOS before closing D1, regardless of whether F1b agrees with WebKitGTK. |
| **R22** | The paginator now executes inside the sandboxed content frame alongside book content (ADR-F007). If ADR-F006 script-stripping is ever bypassed, hostile script shares an execution context with the component reporting pagination state. | Medium | Low | Opaque origin (`allow-scripts` without `allow-same-origin`) means hostile script still cannot reach the app origin or the IPC bridge. Treat paginator output as untrusted input on the Rust side and validate it, rather than trusting reported offsets. |
| **R27** | **Tier B may be silently homogeneous.** Any EPUB rewritten by calibre or a similar manager carries that tool as its producer string and its zip writer's characteristics, obliterating the original publisher toolchain. A library curated with such a tool could report healthy producer diversity while actually measuring one writer — the exact failure D12's gate exists to prevent, wearing the gate's own passing signal. | **High** | **High** | ADR-F046: detect normalization independently of the producer string (`calibre:*` meta, `dc:contributor`, sidecar `metadata.opf`, characteristic entry ordering) and report it as a separate axis. Prefer unmodified files for Tier B. Run `scan` before committing to the corpus. |
| **R20** | `loki-file-access` may not cover what export actually needs: iOS as well as Android, and `ACTION_CREATE_DOCUMENT`-style *create-new-file* flows rather than read/write of files the user already picked. | Medium | Medium | Audit the crate's actual surface before Phase 5 (D11). Gaps are incremental additions to a working crate, not a new bridge from scratch. |
| **R19** | Compiling a large image-heavy EPUB flattens the spine into a single blob and spikes memory, triggering Android LMK or iOS jetsam mid-export. Export is now a mobile feature, so this is on the critical path. | Medium | Medium | Stream PalmDOC text records to disk incrementally rather than building the whole blob in memory. The exact-4096-byte record constraint makes chunked emission the natural implementation anyway. Reuse Loki's memory-tracking harness. |

---

## 10. Phase 0 spikes

All three block Spec 01. None should take more than a week.

| ID | Spike | Question | Pass criteria |
|---|---|---|---|
| ~~**F1**~~ | Multicol pagination | — | **RETURNED. Pass**, on Blink and WebKitGTK, with the `vertical-rl` exception (ADR-F034). ADR-F013 accepted. `docs/spikes/SPIKE_F1_MULTICOL_PAGINATION.md`. |
| **F1b** | WKWebView coverage | Does the F1 harness produce equivalent results on WKWebView, on macOS? | Pagination, position, and selection criteria via safaridriver. **Origin and sandbox criteria require a real Tauri shell** (R23) and are not closed by safaridriver alone. Gates D1 with F1c. |
| **F1c** | Custom-scheme origin | Do opaque-origin, CSP, and iframe-sandbox semantics hold under `WKURLSchemeHandler` in an actual Tauri macOS build? | Content frame is opaque-origin, cannot reach the app origin or `__TAURI__`, and CSP applies as served. Small: one Tauri shell, one book, the F1 assertions for criterion 5 only. |
| **F2** | EPUB round-trip | Does `rbook` (or a `quick-xml` build) round-trip a corpus of 200 real EPUBs when nothing is edited, **preserving every byte of every archive entry**? | Divergence is classified before it is counted (ADR-F036). **Entry-content divergence ≥99% clean** — this is what ADR-F012 actually promises. Container-level divergence (entry order, timestamps, compression level) is reported but does not fail the spike. |
| **F3** | PDF on mobile | Can `hayro` render a 300 MB scanned PDF at acceptable pan/zoom latency on the Lenovo LOQ and a mid-range Android device? | First tile <150 ms; sustained pan without visible tile pop at 60 fps target. |

---

## 11. Roadmap

| Phase | Name | Contents | Exit criterion |
|---|---|---|---|
| **0** | Spikes & foundation | F1–F3, workspace scaffold, CI (300-line ceiling, `forbid(unsafe_code)`, SPDX, clippy, fuzz targets), conformance corpus assembly. | All spikes returned; ADR-F001 confirmed or reversed. |
| **1** | EPUB read path | Ingest, Codecs(EPUB), Document Model, Navigation, Rendering, Sandbox. Reader only. Desktop only. | Open, paginate, and navigate an EPUB 3 with stable position. |
| **2** | Library | Catalog, Storage, Metadata. Catalog, covers, collections, search, dedup, watched folders. | J1 met: 4,000 files ingested in under two minutes. |
| **3** | Reading experience | Annotations, Progress, Presentation. Annotations, bookmarks, progress, themes, typography, accessibility, TTS. | J2, J4 met. Annotations survive settings changes. |
| **4** | Formats (read) | Codecs (MOBI/KF8 read), Codecs (PDF). | J7, J8 met. PDF and KF8 at parity with EPUB for reading. |
| **5** | Mobile | Android then iOS. Gesture layer, LMK/jetsam resilience, platform packaging. Library-wide search evaluated. **Audit and promote `appthere-file-access` early (D11, R20)** — it gates J10 in Phase 6. | Reader and library shipping on both mobile platforms; share-sheet delivery proven. |
| **6** | Editor & Conversion | Editor: source editing, live preview, validation, package operations, undo. Conversion: EPUB to KF8 compiler, CSS downconversion, loss reporting, AZW3 export on all five platforms. | J6, J9, J10 met. Byte-lossless saves (ADR-F012). Device test matrix passed (R16). |
| **7** | Sync & polish | Sync. CRDT annotation/position sync, device handoff. CBZ/CBR if cheap. Security review. | J3 met. External security review passed. |

Phases 1–3 are the minimum viable product; Futhark could ship publicly as an EPUB-only reader and
library at the end of Phase 3 without embarrassment. That is a deliberate property of the ordering.

---

## 12. Open decisions (blocking Spec 01)

| ID | Decision | Notes |
|---|---|---|
| **D1** | Confirm or reverse ADR-F001. | **F1's reversal condition did not fire** — multicol pagination works. Remaining input is Spike F1b (WKWebView on macOS, R21). The §6.5 sensitivity condition that could still flip this is D2, not F1. |
| **D2** | Is iOS in v1, or is it Phase 5+ and possibly later? | This is the largest single input to the D1 sensitivity analysis (§6.5). |
| **D3** | Managed library vs. reference-in-place as the *default* (ADR-F017). | Calibre chose managed and users resent it; reference-in-place is harder to keep consistent. |
| **D4** | Does the editor target EPUB 3 only, or EPUB 2 as well? | EPUB 2 editing roughly doubles the validation surface for a shrinking format. |
| **D5** | Sync: reuse the Loki relay verbatim, or a Futhark-specific document type on the same transport? | Affects whether Sync can start before the Loki server ships. |
| **D6** | ~~Legal review: ship `futhark-kfx`?~~ **Resolved: KFX dropped** (ADR-F022). Residual question is narrower — does KF8 *authoring* need legal review at all? | Provisional answer: no. Writing a DRM-free file in a reverse-engineered, publicly documented format circumvents nothing and implicates no §1201 question. Worth a short confirmatory opinion, not a QSA-scale gate. |
| ~~**D9**~~ | ~~Dual KF7+KF8 output?~~ **Resolved: no.** KF8 only. | Served pre-2011 devices at the cost of a second output path testable only on hardware nobody has. |
| ~~**D10**~~ | ~~Export on mobile?~~ **Resolved: yes, at parity** (ADR-F031). | Compiler is platform-agnostic; parity removes `cfg` divergence and halves the test matrix. Mobile users deliver via share sheet to cloud storage or removable media. |
| **D14** | If `scan` returns an un-normalized population of zero or one producer, which path? | Three options, not two. **(a)** Source unmodified files — original downloads predating library import are the highest-yield place to look, then direct-from-publisher DRM-free stores. **(b)** Narrow F2's claim to rbook-versus-one-writer and say so. **(c)** Run it as a screening test under ADR-F047 — cheap, potentially disqualifying, and it does not require solving the corpus problem first. (c) does not preclude (a) later. |
| **D11** | Promote `loki-file-access` to `appthere-file-access`, or keep it Loki-local and depend on it directly? | Promotion matches the established extraction pattern and a second consumer justifies it. Blocking sub-questions: (a) does it cover iOS or Android only? (b) does it support create-new-file, or only read/write of already-picked files? (c) was it written against the old Tauri stack or the current Dioxus Native one — if the latter, does its context/activity acquisition assume a non-Tauri host? |
| ~~**D7**~~ | ~~Corpus sourcing and licensing.~~ **Resolved: two-tier** (ADR-F037/F038). Note: agent environments reach `index.crates.io` only — Gutenberg, Standard Ebooks, and GitHub are proxy-denied — so **Tier A acquisition is a local task**, not an agent task. Harness, classifier, manifest, and synthetic fixtures are all buildable without it. | Note that F1 and F2 have *opposite* corpus needs. F1 wanted synthetic content under full control. F2 wants in-the-wild producer diversity — InDesign, Sigil, calibre, ancient EPUB 2, publisher toolchains — which is exactly what a generated corpus cannot supply and what Tier A's single-toolchain sources (Gutenberg's ebookmaker, Standard Ebooks) under-represent. Tier B carries F2. |
| ~~**D12**~~ | ~~Producer-diversity bar for Tier B?~~ **Resolved: coverage, not count.** | Record producer-string distribution in the manifest; gate on distinct producers, not file count. |
| ~~**D13**~~ | ~~Build the F2 classifier against real producers or against divergence classes?~~ **Resolved: divergence classes** (ADR-F041). | A classifier is a measuring instrument, so it is validated against inputs whose class is known by construction — not against inputs that are merely representative. Tier A's homogeneity would otherwise encode one toolchain's quirks as the definition of normal. |
| **D8** | Does Futhark ship as a standalone product or as part of an AppThere suite bundle? | Affects packaging, licensing, and whether the shell divergence in §6.5 matters commercially. |

---

## 13. Engineering standards

Inherited from the AppThere program, unchanged:

- Rust 2024 edition.
- 300-line file ceiling, enforced in CI.
- `#![forbid(unsafe_code)]` at every crate root; exceptions documented and reviewed.
- `thiserror` for typed errors. No `unwrap()` or `expect()` in library code.
- Apache-2.0 SPDX headers, correct ordering.
- All user-visible strings through `fl!()`. No hardcoded English.
- Audit-first: no implementation before the corresponding spec is accepted.

Futhark-specific additions:

- **Every codec crate ships a `cargo-fuzz` target at creation** (ADR-F026). Futhark's inputs are
  hostile by default in a way Loki's mostly are not.
- **No panics on malformed input, ever.** A corrupt book renders partially or reports an error; it
  never takes down the process. This is a test requirement, not a guideline.
- **CI enforces that no `futhark-*` domain crate depends on the shell** (ADR-F002). This is the
  mechanism that keeps §6's decision reversible, and it is worth a dedicated check.
