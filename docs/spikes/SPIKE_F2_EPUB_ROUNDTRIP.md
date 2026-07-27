<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F2 — EPUB round-trip fidelity

| Field | Value |
|---|---|
| Document | `SPIKE_F2_EPUB_ROUNDTRIP.md` |
| Spike ID | F2 |
| Status | **Screening result returned on synthetic input. ADR-F011 reversed in spec 0.12.0 on this evidence.** |
| Version | 0.2.0 |
| Date | 2026-07-27 |
| Depends on | `FUTHARK_PROGRAM_SPEC.md` 0.12.0 — §10, R5, ADR-F012, ADR-F036, ADR-F041, ADR-F047, ADR-F048, ADR-F050 |
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
features do real EPUBs actually contain that `futhark-epub` must preserve?*
Comments, processing instructions, attribute ordering, exotic namespaces, zip
structure quirks, EPUB 2 survivals. The deliverable is a requirements list for
the build — and it confirms the reversal as a side effect, which is the part of
the old recommendation worth keeping.

Its answer is needed in Phase 6 regardless of what happens to ADR-F011, which is
what makes it the durable version of the run.

The coverage gate has shifted purpose along with it. It no longer adjudicates
`rbook`; it validates `futhark-epub` once built — which makes the bar more
important, not less. D12 set it at **≥8 distinct producer families, no family
above 40%** of the un-normalized population, counted by family rather than by
string (ADR-F050) because "InDesign 17" and "InDesign 19" are one toolchain. The
verdict names the families it counted, so coverage can be argued with rather than
trusted to a threshold.

## 8. Reproducing

```bash
cd spikes/f2-epub-roundtrip
cargo test                                   # instrument self-validation
cargo run --example emit_fixtures -- /tmp/f2 # synthetic containers
cargo run -- roundtrip /tmp/f2 --tier=a      # this result
cargo run -- roundtrip ~/Books               # F2b: characterise the wild population
cargo run --example rt_one -- <file.epub>    # before/after, for diagnosis
```

`roundtrip` exits non-zero unless the verdict resolves ADR-F011 in either
direction — an inconclusive run is not a success.
