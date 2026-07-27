<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F2 — EPUB round-trip divergence classifier

Phase 0 spike. Spec 00 §10, R5, ADR-F011, ADR-F012.

> Does `rbook` (or a `quick-xml` build) round-trip real EPUBs when nothing is
> edited, **preserving every byte of every archive entry**?

This directory holds the instrument, not yet the answer. What is built and
verified: the frozen taxonomy, the classifier, the result manifest, the producer
extractor, and the fixtures that validate the classifier against inputs whose
class is known by construction.

What is not built: the round-trip itself. The reader under test is ADR-F011's
open question, and classifying is the part that has to be right *first* — a
percentage computed before divergence is classified measures the writer's zip
library rather than its fidelity to the book (ADR-F036).

## Running it

```bash
cargo test                          # the instrument validates itself
cargo run -- self-test              # same fixtures, readable output
cargo run -- compare a.epub b.epub  # classify one round-trip pair
cargo run -- scan ~/Books --tier=b  # manifest a corpus: hashes and producers
```

`scan` is the Tier B entry point and writes `f2-manifest.json`. It records
hashes, producer strings, normalization signals, EPUB versions, and divergence
classes — never content, never Tier B paths — which is what makes the manifest
publishable (ADR-F038).

**Run `scan` before committing to a corpus.** It answers R27, which is the
sharpest threat to F2's validity: a producer string is self-reported and gets
overwritten, so a library curated with calibre reports calibre as the producer of
everything it has touched, erasing the toolchain that actually built each file.
Such a corpus can show healthy producer diversity while measuring one writer —
the failure D12's gate exists to prevent, wearing the gate's own passing signal.

`scan` therefore reports two producer counts: one over all files, and one over
files carrying **no** normalization evidence. Only the second is D12's gate
(ADR-F046). Normalization is detected independently of the producer string —
`calibre:*` meta names, `Sigil version`, contributor fields naming a manager,
`META-INF/calibre_bookmarks.txt`, and sidecar `metadata.opf` / `cover.jpg` in the
same directory — and the run warns outright when the un-normalized population has
fewer than two distinct producers.

## The taxonomy is frozen

`src/taxonomy.rs` is the calibration, frozen 2026-07-27 at classifier version
`1.0.0`. Three groups per ADR-F036:

| Group | Classes | Bears on ADR-F012 |
|---|---|---|
| **A — entry content** | `content-byte-diff`, `entry-missing`, `entry-added`, `xml-canonicalization`, `text-encoding`, `line-endings` | **yes** |
| **B — container metadata** | `entry-order`, `timestamp`, `compression-method`, `compression-level`, `extra-field`, `comment` | no |
| **C — declared vs actual** | `mimetype-not-first`, `mimetype-compressed`, `declared-size-mismatch`, `manifest-mismatch` | no |
| **U** | `unclassified` | **yes** |

The same shape recurs in the normalization detector, and its negative test
earned its place on the first run: a book titled *Mastering calibre: a guide*
contains the literal string `calibre:` and was flagged until detection was
anchored to the attribute position. A detector that fires on everything cannot
separate the populations it exists to separate — which is why "stays silent on a
clean file" is a test rather than an assumption.

Two properties are load-bearing and both are enforced by tests:

- **`unclassified` counts as a failure.** An observation the instrument cannot
  name must not be scored harmless — that is ADR-F042's error wearing different
  clothes. A non-empty bucket is a finding, and under ADR-F041 it is the only
  thing Tier B is allowed to add.
- **Narrower classes are tried first**, so `content-byte-diff` means "differs,
  and none of the specific explanations fit" rather than merely "differs".

Changing a class boundary is a `CLASSIFIER_VERSION` bump and a re-run, never an
edit. The version is stamped on every manifest record so any result can be
reproduced against the instrument that produced it.

## Why fixtures rather than a corpus

D13 resolved this: a classifier is a measuring instrument, and instruments are
validated against inputs whose class is known by construction. Validating against
Tier A would encode one toolchain's habits as the definition of normal —
Gutenberg is all ebookmaker output and Standard Ebooks is a single pipeline.
Validating against Tier B would fit the instrument to the very data it is meant
to measure.

Each fixture in `src/fixtures.rs` is a pair differing in exactly one known way,
holding constant everything it can — including timestamps, which is why the
writer pins them.

Two fixtures are irreducibly multi-class, and saying so was the point of the
first self-test failure: `mimetype` cannot stop being the first entry without the
entry order changing, and cannot be compressed without the compression method
changing. Rather than weaken the classifier to report one class, the fixture
declares the complete expected set and the test asserts set equality — which
still catches both directions, a missing class being under-classification and an
extra one over-classification.

## What comes next

1. **Tier A acquisition** — local, not an agent task. Agent environments reach
   `index.crates.io` only; Gutenberg, Standard Ebooks, and GitHub are
   proxy-denied (D7).
2. **The round-trip stage** — read with `rbook`, write back, feed both to
   `classify`. This is where ADR-F011 gets its answer.
3. **The D12 coverage gate** — `scan` already reports both producer
   distributions, so the corpus can be judged on distinct *un-normalized*
   producers before a single round-trip is run.

## Layout

| Path | Role |
|---|---|
| `src/taxonomy.rs` | The frozen classes. The instrument's calibration. |
| `src/classify.rs` | Two archives in, classified divergences out. Never counts. |
| `src/archive.rs` | Reads a container into a comparable model, storage metadata included. |
| `src/xmlcmp.rs` | Infoset comparison, for telling canonicalisation from content loss. |
| `src/producer.rs` | Producer string and OPF facts. |
| `src/normalization.rs` | R27: was this file rewritten by a manager? Detected without the producer string. |
| `src/manifest.rs` | The publishable artifact (ADR-F038). Carries no book bytes. |
| `src/fixtures.rs` | One known class per fixture. How the instrument is validated. |
| `tests/instrument.rs` | Runs the fixtures in CI so drift fails the build. |
| `tests/normalization.rs` | Both directions: the detector fires on rewritten files and stays silent on clean ones. |

Spike code: outside the Cargo workspace (note the empty `[workspace]` table in
`Cargo.toml`), and nothing in `crates/` may depend on it.
