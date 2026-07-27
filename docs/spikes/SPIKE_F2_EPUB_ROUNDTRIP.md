<!--
SPDX-FileCopyrightText: 2026 Kevin Carlson
SPDX-License-Identifier: Apache-2.0
-->

# Spike F2 — EPUB round-trip fidelity

| Field | Value |
|---|---|
| Document | `SPIKE_F2_EPUB_ROUNDTRIP.md` |
| Spike ID | F2 |
| Status | **Screening result returned on synthetic input. Disqualifying. Corpus run still outstanding.** |
| Version | 0.1.0 |
| Date | 2026-07-27 |
| Depends on | `FUTHARK_PROGRAM_SPEC.md` 0.10.0 — §10, R5, ADR-F011, ADR-F012, ADR-F036, ADR-F041, ADR-F047 |
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

**ADR-F011 is not resolved by this document.** The evidence points one way and
§6 recommends, but recording the decision is the human's.

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
  The classifier's own gate says so: 1 distinct un-normalized producer against a
  floor of 5, and the verdict machinery would have reported `INCONCLUSIVE` had
  the run come back clean. It came back disqualifying instead, which is the one
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

## 6. Recommendation

**Reverse ADR-F011.** Build `futhark-epub` bespoke over `quick-xml` + `zip`,
with a read path that retains the original bytes of every entry and writes
untouched entries back verbatim.

Two supporting points, both about cost rather than principle:

- The failure is structural, so the usual escape — wrap it, patch it, upstream a
  fix — does not apply. Byte preservation has to be designed into the read path.
- R5 priced this at "adds a phase". That estimate stands; nothing here makes it
  larger, and the classifier built for F2 is directly reusable as the conformance
  check for the bespoke writer.

**Confirm on real files before recording the decision.** The run that matters is
the same command against a corpus of real EPUBs — which needs no new code, only
the files. Given the mechanism (timestamps injected at save time), the outcome is
not in much doubt, but "not in much doubt" and "measured" are different claims,
and the retraction earlier in this project came from treating one as the other.

## 7. Reproducing

```bash
cd spikes/f2-epub-roundtrip
cargo test                                   # instrument self-validation
cargo run --example emit_fixtures -- /tmp/f2 # synthetic containers
cargo run -- roundtrip /tmp/f2 --tier=a      # this result
cargo run -- roundtrip ~/Books               # the run that matters
cargo run --example rt_one -- <file.epub>    # before/after, for diagnosis
```

`roundtrip` exits non-zero unless the verdict resolves ADR-F011 in either
direction — an inconclusive run is not a success.
