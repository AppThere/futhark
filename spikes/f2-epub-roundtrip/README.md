<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F2 — EPUB round-trip divergence classifier

Phase 0 spike. Spec 00 §10, R5, ADR-F011, ADR-F012.

> Does `rbook` (or a `quick-xml` build) round-trip real EPUBs when nothing is
> edited, **preserving every byte of every archive entry**?

**Screening result returned: `rbook` 0.7.10 does not round-trip losslessly.**
Findings in
[`docs/spikes/SPIKE_F2_EPUB_ROUNDTRIP.md`](../../docs/spikes/SPIKE_F2_EPUB_ROUNDTRIP.md).
It injects `dc:date` and `dcterms:modified` stamped with the current clock into
files nothing edited, so consecutive saves of an untouched book differ from each
other. That is structural — it parses to a model and serialises from it — not a
bug to wrap around. ADR-F011 is reversed in spec 0.12.0 on that evidence, which
is twelve synthetic containers and zero real books; the ADR records the gap.

## Running it

```bash
cargo test                          # the instrument validates itself
cargo run -- self-test              # same fixtures, readable output
cargo run -- compare a.epub b.epub  # classify one round-trip pair
cargo run -- scan ~/Books --tier=b  # manifest a corpus: hashes and producers
cargo run -- roundtrip ~/Books      # the spike proper: read, write back, judge
cargo run -- characterise ~/Books   # F2b: enumerate what the population contains
cargo run --example emit_fixtures -- /tmp/f2   # synthetic containers
cargo run --example rt_one -- book.epub        # before/after OPF, for diagnosis
```

`roundtrip` judges the run under ADR-F047 and **has no plain "pass"**. A clean
result on a normalized or narrow corpus is `INCONCLUSIVE`, not success: a
manager's writer strips exactly the constructs that break parsers, so such a
corpus screens the easy population. Only `QUALIFYING` — clean *and* diverse —
supports adoption, and `tests/verdict.rs` enforces that a screening pass can
never reach it. The exit status follows whether the run resolved the question,
not whether anything failed.

The coverage gate counts producer **families**, not strings (ADR-F050) —
"InDesign 17" and "InDesign 19" are one toolchain, and a string count is the R27
shape one level up. D12 set it at ≥8 families with no family above 40% of the
un-normalized population, because twenty families where one holds 95% is a worse
corpus than six held evenly and a count cannot tell them apart. The verdict
**names the families it counted**, so coverage can be argued with.

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

## F2b: `characterise`, and why its output is not a requirements list

F2b writes nothing back and judges no reader. It asks what real EPUBs *contain*
that `futhark-epub` must preserve, and its problem is that it has no verdict to
hang a caveat on — a thin result on a normalized corpus looks exactly like a
thin ecosystem. Two types carry the caveat instead.

**ADR-F052 — a fixed catalogue, three states per feature.** `Observed`,
`CheckedAndAbsent`, `NotCovered`. The third is the one that matters: it means
*no detector exists*, and it is evidence about the harness rather than about the
corpus. Four entries in `src/catalogue.rs` deliberately have no detector, so
`NotCovered` is non-empty by construction — a catalogue where everything is
covered cannot demonstrate the difference between the two silences, and quietly
becomes a two-state list the first time someone reads a report off it. The
output is arithmetic rather than rhetoric: *26 catalogued, k observed, m
explicitly absent, j never looked at.*

A fifth entry joined them while this was being built. `directory-entry` had a
detector, and `Archive::read` skips `is_dir()` entries — so it would have
reported `CheckedAndAbsent` on every corpus ever scanned, a clean signal
produced by the reader's filter rather than by the books. It is now `NotCovered`
with that as its reason, and `tests/catalogue.rs` holds it there.

**ADR-F051 — the output is a floor and the type says so.** `FeatureFloor` has no
method that yields a requirements list. The only route to `Requirements` is
`widen()`, which fails unless every feature the corpus could not settle carries
an explicit disposition *with a reason*, and which stamps the result with the
corpus it came from. `Requirements` has private fields and deliberately does not
derive `Deserialize`, because parsing one from JSON would be a back door around
the widening step.

Which features count as unsettled depends on the corpus rather than on taste. On
a normalized corpus, `CheckedAndAbsent` is worth no more than `NotCovered` — a
manager's writer strips exactly the constructs being enumerated, so its silence
is about the writer. That is ADR-F047's asymmetry applied to enumeration, and
`CorpusProvenance::absence_is_evidence` is where it lives.

Two detectors were reporting `Observed` off a substring rather than a construct.
Every EPUB declares the OCF namespace as
`urn:oasis:names:tc:opendocument:xmlns:container`, so a bare `xmlns:` scan
reported an exotic namespace prefix in every book ever made; and prose split on
whitespace yields "attribute names" that are never alphabetical, so
`<p>the quick brown fox` scored as non-alphabetical attribute order. `Observed`
is the state nothing downstream questions — it needs no disposition and goes
straight into `must_preserve` — which is what makes a false one expensive.

Both reached the right variant for the wrong reason, which no reachability
witness can see. `tests/detectors.rs` is the answer (ADR-F055): each of the 21
detectors is paired with a true negative, and the assertion is **differential**
rather than one-sided — the set of features `Observed` in the positive case,
minus those `Observed` in the negative, must be *exactly* the feature under
test. An empty difference means the detector already fired on the negative,
which is the shape both bugs had; an extra element means the pair is not
minimal and proves nothing about specificity.

The base container is the control, and it is deliberately realistic rather than
minimal: it carries the OCF namespace URI, an XML declaration, a stored
`mimetype`, and a sentence of prose — each of them something a detector has
already mistaken for a construct. `the_base_container_observes_nothing` runs
before any difference is computed, for the same reason `check-self-test` runs
each check on the unperturbed tree first. Both bugs were re-introduced to
confirm the test fails on them; both fail in the empty-difference form.

## Every variant needs a witness

ADR-F053, and it came out of a bug in this crate. `FeatureState` declared three
states and reached two — `accumulate` used `or_insert`, which could never
promote `NotCovered` to `CheckedAndAbsent`, so a working detector and an absent
one produced the same state. Every test passed throughout. Encoding the
distinction as a type was not enough: an unreachable variant is documentation
wearing a type's clothes, and inherits every weakness this project has been
encoding *away* from documentation.

`tests/reachability.rs` reaches each variant through the real path —
`accumulate`, `judge`, `classify`, `roundtrip`, `widen` — never by constructing
it, since a directly-constructed variant witnesses the `enum` keyword.

The audit found three genuine gaps, and they are named rather than counted:

- **Three divergence classes have no fixture, and the reasons differ in kind**
  (ADR-F056). `timestamp` and `compression-level` are *beyond the harness*: the
  fixture writer pins timestamps so every other class is measured against a
  constant, and closing that gap means giving up the control. `unclassified` is
  unwitnessable *by nature* — a fixture for it would be a fixture for the
  classifier's own blind spot. The test asserts the unreached set equals the
  declared set exactly, so the gap cannot change size quietly in either
  direction.

  Three others were listed here and are not any more, because their fixtures got
  written rather than reclassified (ADR-F057). `extra-field` writes a `0x5455`
  extended-timestamp field on one side; `declared-size-mismatch` patches the
  central directory to claim a size the entry does not have, leaving the CRC
  correct so the archive still opens; `manifest-mismatch` declares a phantom
  `<item href>` on both sides, which isolates the class from the
  `content-byte-diff` that changing one OPF would produce.

  **`manifest-mismatch` needed a detector before it could have a fixture.** The
  class had been frozen since classifier 1.0.0 with no code path — `producer.rs`
  had been collecting `manifest_hrefs` "for the `ManifestMismatch` check" the
  whole time — so every run reported `manifest-mismatch: 0` and the zero was
  produced by the missing check rather than by the containers. A histogram
  bucket that can never fill is the `Tier::Fixture` shape wearing a
  measurement's clothes.
- **`Outcome::WriteFailed` and `Outcome::Panicked` have no witness.** Both need
  an input that defeats `rbook` in a specific way, and one invented for the test
  would witness the invention rather than the reader. `ReadFailed` does have one
  — hostile bytes, refused, and `produced_output()` false, so an absent output
  is never scored as zero divergences.
- **`Tier::Fixture` is declared and never constructed.** A manifest reader would
  take the schema to mean fixture-sourced records exist and can be told from
  Tier A ones. None do.

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

1. **F2b against a real corpus** — `characterise` is built and tested; what it
   has not seen is a book nobody generated. Needed in Phase 6 whatever happened
   to ADR-F011, which it confirms as a side effect.
2. **Tier A acquisition** — local, not an agent task. Agent environments reach
   `index.crates.io` only; Gutenberg, Standard Ebooks, and GitHub are
   proxy-denied (D7).
3. **The `quick-xml` alternative**, if ADR-F011 reverses. The classifier is
   directly reusable as its conformance check.

## Layout

| Path | Role |
|---|---|
| `src/taxonomy.rs` | The frozen classes. The instrument's calibration. |
| `src/classify.rs` | Two archives in, classified divergences out. Never counts. |
| `src/archive.rs` | Reads a container into a comparable model, storage metadata included. |
| `src/xmlcmp.rs` | Infoset comparison, for telling canonicalisation from content loss. |
| `src/producer.rs` | Producer string and OPF facts. |
| `src/family.rs` | ADR-F050: producer families, not strings. Unrecognised producers get their own family rather than being merged. |
| `src/normalization.rs` | R27: was this file rewritten by a manager? Detected without the producer string. |
| `src/manifest.rs` | The publishable artifact (ADR-F038). Carries no book bytes. |
| `src/fixtures.rs` | The container builder: plans, zip options, and the byte-level patch that makes a header lie. |
| `src/fixture_cases.rs` | The 15 cases themselves — the part a reviewer checks against the taxonomy. |
| `tests/instrument.rs` | Runs the fixtures in CI so drift fails the build. |
| `src/roundtrip.rs` | Open with rbook, write back untouched. A panic is a finding, not a crash. |
| `src/verdict.rs` | ADR-F047 as a type. There is no `Verdict::Pass`. |
| `src/catalogue.rs` | ADR-F052: the fixed feature list and its three states. Some entries have no detector on purpose. |
| `src/detect.rs` | The detectors behind `Observed` and `CheckedAndAbsent`. Scans, never parses. |
| `src/floor.rs` | ADR-F051: `FeatureFloor`, `widen()`, and the only type that may be read as a specification. |
| `src/provenance.rs` | What a floor rests on, and what widening one costs. |
| `src/characterise.rs` | F2b itself: walk, enumerate, emit the floor. |
| `tests/catalogue.rs` | `NotCovered` is not `CheckedAndAbsent`, and stays non-empty. |
| `tests/floor.rs` | A floor cannot become a specification without a reasoned widening. |
| `tests/reachability.rs` | ADR-F053/F056: every variant reached through the real code path, or named as lacking a witness — sorted by kind. |
| `tests/detectors.rs` | ADR-F055: every detector paired with a true negative. The assertion is differential, not one-sided. |
| `tests/normalization.rs` | Both directions: the detector fires on rewritten files and stays silent on clean ones. |
| `tests/verdict.rs` | A screening pass can never support adoption. |

Spike code: outside the Cargo workspace (note the empty `[workspace]` table in
`Cargo.toml`), and nothing in `crates/` may depend on it.
