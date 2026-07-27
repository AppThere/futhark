<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# CLAUDE.md — AppThere Futhark

Ebook library manager, reader, and editor. Rust. Desktop (Windows, macOS, Linux) and mobile
(Android, iOS). Reads EPUB 2/3, MOBI/KF7, AZW3/KF8, and PDF. Edits EPUB. Exports EPUB and KF8.

Authoritative design document: `docs/FUTHARK_PROGRAM_SPEC.md` (Spec 00). Read it before
proposing architecture. This file is the operating manual; the spec is the design.

---

## STATUS — read before writing any code

**The project is in Phase 0.** Only three things are in scope right now:

1. Spikes F1, F2, F3 (§10 of the spec)
2. Workspace scaffold
3. CI

**Do not build the presentation shell.** ADR-F001 (Tauri 2) is **provisional and gated on Spike
F1**. If F1 fails, the shell becomes Dioxus Native and any shell code written now is discarded.

### Provisional decisions — do not treat as settled

| ADR | Decision | Gated on |
|---|---|---|
| ADR-F001 | Tauri 2 as presentation shell | Spike F1 |
| ADR-F011 | `rbook` as the EPUB reader | Spike F2 |
| ADR-F013 | CSS multicol pagination | Spike F1 |
| ADR-F033 | Promote `loki-file-access` → `appthere-file-access` | D11 audit |

Eleven open decisions (D1–D11) are listed in §12. If a task requires resolving one, **stop and
ask** rather than picking. Recording a decision the human did not make is worse than blocking.

---

## Hard rules

These are not style preferences. CI enforces them; violations fail the build.

- **Rust 2024 edition.**
- **300-line file ceiling.** Split before you exceed it, not after.
- **`#![forbid(unsafe_code)]` at every crate root.** Exceptions require a documented ADR.
- **No `unwrap()` or `expect()` in library code.** Tests may use them.
- **`thiserror` for all error types.** Typed errors, never `Box<dyn Error>` in public APIs.
- **Apache-2.0 SPDX headers on every file**, correctly ordered (`SPDX-FileCopyrightText` before
  `SPDX-License-Identifier`).
- **All user-visible strings through `fl!()`.** No hardcoded English anywhere in the UI path.
- **No C dependencies.** No FFI shortcuts. If a C library is the obvious answer, that is a
  discussion, not a decision to make mid-task.

## Futhark-specific rules

- **No panics on malformed input. Ever.** Every parser handles hostile bytes. A corrupt book
  renders partially or reports a typed error; it never takes down the process. This is a test
  requirement, not an aspiration.
- **Every codec crate gets a `cargo-fuzz` target the day it is created.** Not retrofitted.
  Futhark's inputs are hostile by default in a way Loki's mostly are not.
- **No domain crate may depend on the shell** (ADR-F002). CI enforces this. It is the mechanism
  that keeps the shell decision reversible — do not let shell types leak downward, not even a
  convenience `From` impl.
- **No DRM is written, read around, or circumvented** (ADR-F021). No KFX support in any
  direction (ADR-F022). If a task drifts toward either, stop.
- **No fabricated vendor identifiers** (ADR-F029). Never write a fake ASIN to unlock device
  features.

---

## Layout

```
futhark/
├─ crates/
│  ├─ futhark-core/       # shared types, errors, config, i18n
│  ├─ futhark-catalog/    # Catalog: holdings, collections, search, dedup
│  ├─ futhark-ingest/     # Ingest: detection, sniffing, watched folders
│  ├─ futhark-doc/        # Document Model: normalized IR
│  ├─ futhark-epub/       # Codecs: EPUB 2/3 read + write
│  ├─ futhark-mobi/       # Codecs: PalmDB/MOBI/KF8 read + write
│  ├─ futhark-pdf/        # Codecs: hayro integration
│  ├─ futhark-nav/        # Navigation: locators, CFI, pagination map
│  ├─ futhark-render/     # Rendering: raster pipeline, tile cache, fonts
│  ├─ futhark-annot/      # Annotations: model + anchoring
│  ├─ futhark-store/      # Storage: SQLite, migrations, blob store
│  ├─ futhark-stats/      # Progress: sessions, reading position
│  ├─ futhark-sandbox/    # Sandbox: sanitization, CSP policy, resource gate
│  ├─ futhark-editor/     # Editor: package mutation, validation, undo
│  ├─ futhark-sync/       # Sync: Loro CRDT + AppThere Cloud client
│  ├─ futhark-meta/       # Metadata: OPDS, providers
│  └─ futhark-convert/    # Conversion: EPUB→KF8 compiler, CSS downconvert
├─ shell/                 # presentation shell — DO NOT POPULATE IN PHASE 0
├─ conformance/           # appthere-conformance suite
├─ docs/                  # specs and ADRs
└─ patches/
```

Context names in the spec map one-to-one onto crate names. There is no translation step.

---

## Workflow

**Audit-first.** Before implementing anything: read the existing code, state what is there, and
identify what is missing. Do not write code in the same turn you first look at a subsystem.

**Spec-driven.** Implementation follows an accepted spec. Spec 00 is a program spec and is
deliberately not implementable — it defines boundaries, not behaviour. Subsystem specs (01, 02, …)
come after Phase 0 and after the spikes resolve.

**Small, verifiable units.** One crate or one module per session. Run `cargo test` and
`cargo clippy` before declaring anything done.

**When a decision is missing, ask.** Do not infer it from surrounding code and proceed.

## Standing review question

Three findings in this project have shared one shape: **a signal that looks like the healthy one
but is produced by a different mechanism.**

- `Class::Unclassified` scoring as "no problem found" rather than "no explanation fits"
- A sandboxed probe's silence scoring as a pass rather than as unobservable
- A rewritten file's producer string scoring as ecosystem diversity rather than as one writer

Only the first generalized on its own; the others were caught by a control run and by review. So
ask it explicitly rather than waiting to notice it: **for every pass, green result, or healthy
count — what else could produce this exact signal?** Prefer a distinguishable failure over a
silent one, and make the ambiguous case its own reportable state (ADR-F042).

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo deny check            # license + advisory audit
./scripts/check-line-limit  # 300-line ceiling
./scripts/check-spdx        # header presence and ordering
./scripts/check-layering    # no shell deps in domain crates (ADR-F002)
```

---

## Do not

- Build shell code before Spike F1 resolves.
- Add KFX support in any form.
- Resolve an open decision (D1–D11) unilaterally.
- Introduce a C dependency or an `unsafe` block to work around a missing crate.
- Add `unwrap()` to library code because the error path is inconvenient.
- Exceed 300 lines and plan to split it later.
- Reuse Loki's document IR for EPUB (ADR-F003) — EPUB's model is XHTML+CSS.
