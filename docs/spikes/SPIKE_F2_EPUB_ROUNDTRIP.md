<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F2 — EPUB round-trip fidelity

| Field | Value |
|---|---|
| Document | `SPIKE_F2_EPUB_ROUNDTRIP.md` |
| Spike ID | F2 |
| Status | **Screening result returned on synthetic input. ADR-F011 reversed in spec 0.12.0 on this evidence. F2b instrument built; not yet run against real books.** |
| Version | 0.3.1 |
| Date | 2026-07-27 |
| Depends on | `FUTHARK_PROGRAM_SPEC.md` 0.13.1 — §10, R5, ADR-F012, ADR-F036, ADR-F041, ADR-F047, ADR-F048, ADR-F050, ADR-F051, ADR-F052 |
| Harness | `spikes/f2-epub-roundtrip/` |
| Raw results | `f2-manifest.json`, regenerated per run |

---

## 1. The question

> Does `rbook` round-trip real EPUBs when nothing is edited, **preserving every
> byte of every archive entry**?

ADR-F012 is the promise under test: saving a file the user did not touch must
produce byte-identical output. The no-op save is that promise at its weakest and
most testable point.

## 2. Answer, so far

**`rbook` 0.7.10 does not round-trip losslessly, and the reason is structural
rather than a bug.** Twelve synthetic containers, nothing edited between read and
write, **0 of 12 byte-preserved**.

This is a **screening result under ADR-F047**, and the asymmetry runs in the
useful direction: a failure on easy input is decisive, because the wild
population is harder. The corpus here is easier still than a normalized one —
minimal, valid, machine-generated EPUB 3.

### 2.1 A note on how this document was read

Version 0.1.0 said "ADR-F011 is not resolved by this document" in §2 and opened
§6 with the imperative "Reverse ADR-F011". Spec 0.12.0 closed the ADR, citing
this spike.

That is not a misreading. A section headed **Recommendation** beginning with an
imperative *is* a conclusion, whatever a caveat three sections earlier says, and
splitting the two across a document is how the caveat gets lost. It is the same
"probable → established" slide this project has been watching for at the
measurement layer, occurring one layer up, in the prose.

The spec now records the ADR as **Reversed — on synthetic evidence**, with the
gap visible. §6 below no longer separates the recommendation from what it rests
on.

## 3. What `rbook` does to an untouched file

Read with `Epub::open`, written back with `epub.write().save`, nothing in
between:

| Entry | Class | What changed |
|---|---|---|
| `OEBPS/package.opf` | `content-byte-diff` | 706 → 864 bytes |
| `OEBPS/nav.xhtml` | `content-byte-diff` | 265 → 358 bytes |
| `OEBPS/toc.ncx` | `content-byte-diff` | 351 → 547 bytes |
| `META-INF/container.xml` | `xml-canonicalization` | infoset preserved, serialisation differs |
| — | `entry-order` | central directory reordered |
| `mimetype` | `timestamp` | rewritten to the current time |

The OPF diff is the one that matters:

```diff
-  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
+  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
+            xmlns:opf="http://www.idpf.org/2007/opf">
     <dc:identifier id="id">urn:uuid:f2-fixture</dc:identifier>
     <dc:title>Fixture</dc:title>
     <dc:language>en</dc:language>
     <meta name="generator" content="futhark-f2-fixtures"/>
+    <dc:date>2026-07-27T12:14:40Z</dc:date>
+    <meta property="dcterms:modified">2026-07-27T12:14:40Z</meta>
   </metadata>
```

Three separate things, in descending order of severity:

1. **It writes metadata the user did not author, into a file the user did not
   edit** — a `dc:date` and a `dcterms:modified` stamped with the current clock.
2. **That output is not even self-consistent across runs.** Two consecutive saves
   of the same untouched file differ from each other, because the injected
   timestamp moves. A byte-comparison against "the last save" can never settle.
3. **It re-serialises rather than preserves.** Namespace declarations are added,
   whitespace and indentation are normalised, `container.xml` is reformatted with
   its infoset intact. Comments, attribute order, and unknown elements are at the
   mercy of the same path.

### 3.1 This is not a bug in `rbook`

`dcterms:modified` is *required* by EPUB 3, and a library whose stated purpose
includes building and editing packages is right to maintain it. `rbook` parses to
a structured model and serialises from that model — which is the correct design
for a builder and the wrong one for a byte-preserving editor.

So the finding is not "`rbook` is broken". It is that **`rbook`'s model is
incompatible with ADR-F012**, and no wrapper fixes it: the original bytes are
gone by the time the model exists. Adopting `rbook` would mean either abandoning
byte-lossless saves or re-implementing the read path underneath it, at which
point little of `rbook` remains.

## 4. What this run does *not* establish

- **Nothing about real books.** Twelve synthetic containers from one producer.
  The classifier's own gate says so: one producer family against a floor of
  eight, and the verdict machinery would have reported `INCONCLUSIVE` had the run
  come back clean. It came back disqualifying instead, which is the one
  direction a weak corpus can still support (ADR-F047).
- **Nothing about a `quick-xml` build.** F2's question names an alternative that
  has not been built. What this run establishes is that the *default* starting
  point in ADR-F011 does not meet ADR-F012, not that the alternative does.
- **Nothing about `rbook` as a reader.** Every file opened, parsed, and wrote
  without error or panic. `Outcome::RoundTripped` on all twelve. If ADR-F012 were
  dropped, none of this would count against it.

## 5. A confound caught on the first run, and worth recording

The first round-trip reported `entry-added` on all twelve files: `rbook` was
emitting `toc.ncx` and `toc.xhtml` that the sources did not contain. Read as a
finding, that is severe — a reader inventing spine documents.

It was an artifact of the fixtures. They declared no nav document, so they were
not valid EPUB 3, and `rbook` was *repairing* them. **A reader that repairs
invalid input and one that corrupts valid input produce the same signal**, and
the fixtures could not tell them apart.

Adding a nav document and an NCX to the fixture base removed `entry-added`
entirely and left the genuine divergence behind. The standing review question
applies to red results as readily as green ones: *what else could produce this
exact signal?* Here, an invalid corpus could.

## 6. What this supports, and on what

**Claim:** `rbook`'s architecture cannot satisfy ADR-F012, so `futhark-epub`
should be bespoke over `quick-xml` + `zip`, with a read path that retains the
original bytes of every entry and writes untouched entries back verbatim
(ADR-F048).

**Evidence:** twelve synthetic containers, zero real books. Every one diverged,
and the mechanism is visible in the diff rather than inferred from the count.

**Why the thin evidence still carries the claim:** the finding is architectural.
`rbook` builds a model and serialises from it, so the original bytes are gone
before any writer runs. That is a property of the design, observable in one file,
and a larger corpus would restate it rather than strengthen it. The screening
asymmetry (ADR-F047) points the same way — a failure on the easy population is
the informative direction.

**Where it could still be wrong:** if some `rbook` API preserves entries the
`write()`/`save()` path does not, the claim is about that path rather than the
crate. Nothing in the crate's surface suggests one, and the timestamp injection
would survive it regardless.

Two notes on cost, since they bear on the decision rather than the evidence:

- The usual escapes — wrap it, patch it, upstream a fix — do not apply to a
  design property. Byte retention has to be in the read path from the start.
- R5 priced this at "adds a phase". That estimate stands, and the classifier
  built for F2 is directly reusable as the bespoke writer's conformance check.

## 7. What replaces the confirmation run

Version 0.1.0 recommended re-running against real books to move the conclusion
from inferred to measured. That was the right instinct pointed at the wrong
target: **`rbook` is no longer the subject.** Confirming a library that will not
be used is low-value work.

The question worth the same command is **F2b**: *what infoset and container
features do real EPUBs actually contain that `futhark-epub` must preserve?* Its
answer is needed in Phase 6 regardless of what happened to ADR-F011, which it
confirms as a side effect. The instrument is built (`cargo run -- characterise
<dir>`); what it has not seen is a book nobody generated.

### 7.1 F2b has no verdict, so it has two types

`roundtrip` could put its caveat in `Verdict`, and that worked. F2b cannot: it
produces an enumeration, and a thin enumeration on a normalized corpus is
indistinguishable from a thin ecosystem. The caveat has nowhere to live except
prose — which is precisely the arrangement that failed in §2.1 of this document.

**ADR-F052 — a fixed catalogue, three states.** `Observed`,
`CheckedAndAbsent`, `NotCovered`. The third means *no detector exists*: evidence
about the harness, never about the corpus. Four catalogue entries have no
detector deliberately, so the state is non-empty by construction — a catalogue
where everything is covered cannot demonstrate the difference between the two
silences. The report is arithmetic: *26 catalogued, k observed, m explicitly
absent, j never looked at.*

A fifth entry joined them during the build, and it is the standing review
question landing inside the instrument written to answer it. `directory-entry`
had a detector; `Archive::read` skips `is_dir()` entries. It would have reported
`CheckedAndAbsent` on every corpus ever scanned — a clean signal produced by the
reader's filter rather than by the books.

Two detectors had the same shape in the other direction, over-reporting
`Observed` from a substring rather than a construct. Every EPUB declares the OCF
namespace as `urn:oasis:names:tc:opendocument:xmlns:container`, so scanning for
`xmlns:` found an exotic prefix in every book ever made; and prose split on
whitespace yields tokens that are never alphabetical, so `<p>the quick brown
fox` scored as non-alphabetical attribute order. `Observed` is the state nothing
downstream questions — it needs no disposition and goes straight into the
requirements — which is what makes a false one expensive. Both are now
regression tests.

**ADR-F051 — the output is a floor, and the type is named one.** `FeatureFloor`
exposes no requirements list. The only route to `Requirements` is `widen()`,
which fails unless every feature the corpus could not settle carries a
disposition *with a reason*, and which stamps the result with the corpus behind
it. `Requirements` has private fields and no `Deserialize`, because parsing one
from JSON would be a back door around the widening step.

What counts as unsettled is a property of the corpus, not of taste. On a
normalized corpus `CheckedAndAbsent` is worth no more than `NotCovered`: a
manager's writer strips exactly the constructs being enumerated, so its silence
is the writer's. `CorpusProvenance::absence_is_evidence` requires zero
normalization evidence *and* D12's gate in both directions before a corpus's
silence is believed at all. On the synthetic fixtures — twelve books, one
family — 23 of 26 features come back unsettled, which is the instrument
reporting the corpus rather than the ecosystem.

### 7.2 The coverage gate changed purpose

It no longer adjudicates `rbook`; it validates `futhark-epub` once built, which
makes the bar more important rather than less. D12 set it at **≥8 distinct
producer families, no family above 40%** of the un-normalized population,
counted by family rather than by string because "InDesign 17" and "InDesign 19"
are one toolchain. The verdict names the families it counted: **a threshold can
only be trusted or not; a named list can be argued with.**

### 7.3 The catalogue ADR is F052

Spec 0.13.0 minted two ADRs numbered F050. Resolved at 0.13.1: the
producer-family rule is older and keeps the number; the feature catalogue is
**ADR-F052**, and this document, `catalogue.rs`, `floor.rs`, `characterise.rs`,
and the F2 README cite it there. `family.rs`, `manifest.rs`, and `verdict.rs`
still cite F050 and are correct to.

The number is the interesting part rather than the sweep. An ADR identifier is a
citation target, and a duplicate makes every citation to it ambiguous while both
rows still read correctly in isolation — there is no reading of either row that
looks wrong. `scripts/check-adr-numbers` now fails the build on one and reports
the next free identifier; run against 0.13.0 it names ADR-F052, which is the
number the fix uses. That control matters more than the check passing
afterwards: a checker written against an already-repaired file has never been
shown to detect anything.

## 8. Reproducing

```bash
cd spikes/f2-epub-roundtrip
cargo test                                   # instrument self-validation
cargo run --example emit_fixtures -- /tmp/f2 # synthetic containers
cargo run -- roundtrip /tmp/f2 --tier=a      # this result
cargo run -- characterise ~/Books            # F2b: enumerate the wild population
cargo run --example rt_one -- <file.epub>    # before/after, for diagnosis
```

`roundtrip` exits non-zero unless the verdict resolves ADR-F011 in either
direction — an inconclusive run is not a success. `characterise` exits non-zero
on an empty corpus, because an all-`NotCovered` floor from zero books is not a
thin result about the ecosystem; it is no result.
