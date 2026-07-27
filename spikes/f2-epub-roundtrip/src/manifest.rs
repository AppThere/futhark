// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The result manifest (ADR-F038) — the publishable artifact.
//!
//! F2's deliverable is not the corpus and not a single pass/fail. It is this:
//! for each book, what it was, who produced it, and how the round-trip diverged.
//! That is what makes Tier B usable at all — the manifest can be committed and
//! reviewed without redistributing a copyrighted byte, and it is what tells you
//! whether `rbook` is fixable or replaceable rather than merely whether it
//! passed.
//!
//! Nothing here may carry book content. Hashes, counts, class names, and
//! byte-length deltas only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::family;
use crate::taxonomy::{CLASSIFIER_VERSION, Class, Divergence, Group, TAXONOMY_FROZEN};

/// One book's result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// SHA-256 of the source file. The book's identity, not its content.
    pub source_hash: String,
    /// Size in bytes, as a weak sanity check alongside the hash.
    pub source_bytes: u64,
    /// Which tier this file came from. Tier B files never carry a path.
    pub tier: Tier,
    /// Path, for Tier A only. Tier B is identified by hash alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Producer string as declared by the package, if any (D12).
    ///
    /// Self-reported and overwritable. Read it together with `normalized_by`:
    /// on a rewritten file this names the last tool to touch it, not the
    /// toolchain that built it (R27).
    pub producer: Option<String>,
    /// Evidence the file was rewritten by a manager, detected independently of
    /// the producer string (ADR-F046). Empty means no evidence found.
    pub normalized_by: Vec<String>,
    /// EPUB version declared in the OPF, if readable.
    pub epub_version: Option<String>,
    /// Number of archive entries in the source.
    pub entry_count: usize,
    /// Every divergence observed.
    pub divergences: Vec<Divergence>,
    /// What the reader did when pointed at this book. `None` when the file was
    /// only scanned, not round-tripped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<crate::roundtrip::Outcome>,
    /// The instrument that produced this record (ADR-F041).
    pub classifier_version: String,
}

/// Which corpus tier a record came from.
///
/// There was a third variant, `Fixture`. Nothing ever constructed it, and a
/// reader of the manifest schema would have taken it as a promise that
/// fixture-sourced records exist and can be told apart from Tier A ones. Under
/// ADR-F056 that is *dead* rather than merely unwitnessed, and the response to
/// dead is deletion — documenting it would have preserved exactly the claim it
/// could not support. Fixture runs use `Tier::A`, which is what they are: files
/// in the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Redistributable, in the repository.
    A,
    /// The developer's own library. Never committed; referenced by hash.
    B,
}

impl Record {
    /// Whether this file shows evidence of having been rewritten by a manager.
    /// Only un-normalized files carry a trustworthy producer string, so only
    /// they count toward D12's diversity gate.
    pub fn normalized(&self) -> bool {
        !self.normalized_by.is_empty()
    }

    /// Whether this book round-tripped losslessly in the sense ADR-F012 means.
    ///
    /// A reader that could not open, write, or survive the file is not lossless
    /// — it produced nothing to compare. Scoring an absent output as "no
    /// divergence found" is the standing review question's exact failure.
    pub fn lossless(&self) -> bool {
        if self.outcome.as_ref().is_some_and(|o| !o.produced_output()) {
            return false;
        }
        !self
            .divergences
            .iter()
            .any(|d| d.class.bears_on_losslessness())
    }

    /// Classes present, deduplicated.
    pub fn classes(&self) -> Vec<Class> {
        let mut v: Vec<Class> = self.divergences.iter().map(|d| d.class).collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// The whole run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Instrument version, repeated at the top for readers who never open a record.
    pub classifier_version: String,
    /// When the taxonomy was frozen (ADR-F041).
    pub taxonomy_frozen: String,
    /// One entry per book.
    pub records: Vec<Record>,
}

impl Manifest {
    /// Start an empty manifest stamped with the current instrument.
    pub fn new() -> Self {
        Self {
            classifier_version: CLASSIFIER_VERSION.to_owned(),
            taxonomy_frozen: TAXONOMY_FROZEN.to_owned(),
            records: Vec::new(),
        }
    }

    /// Aggregate counts, including classes that occurred zero times — an absent
    /// class and a zero-count class are different claims, and only one of them
    /// is evidence.
    pub fn summary(&self) -> Summary {
        let mut by_class: BTreeMap<String, usize> = Class::ALL
            .iter()
            .map(|c| (c.slug().to_owned(), 0))
            .collect();
        let mut producers: BTreeMap<String, usize> = BTreeMap::new();
        let mut unnormalized_producers: BTreeMap<String, usize> = BTreeMap::new();
        let mut unnormalized_families: BTreeMap<String, usize> = BTreeMap::new();
        let mut signals: BTreeMap<String, usize> = BTreeMap::new();

        for r in &self.records {
            for c in r.classes() {
                *by_class.entry(c.slug().to_owned()).or_insert(0) += 1;
            }
            let p = r
                .producer
                .clone()
                .unwrap_or_else(|| "(undeclared)".to_owned());
            *producers.entry(p.clone()).or_insert(0) += 1;
            if r.normalized() {
                for sig in &r.normalized_by {
                    *signals.entry(sig.clone()).or_insert(0) += 1;
                }
            } else {
                // Families, not strings (ADR-F050). Two InDesign releases are
                // one toolchain, and counting them as two overstates coverage.
                *unnormalized_families
                    .entry(family::classify(r.producer.as_deref()))
                    .or_insert(0) += 1;
                *unnormalized_producers.entry(p).or_insert(0) += 1;
            }
        }

        let total = self.records.len();
        let lossless = self.records.iter().filter(|r| r.lossless()).count();
        let container_only = self
            .records
            .iter()
            .filter(|r| r.lossless() && !r.divergences.is_empty())
            .count();

        Summary {
            total,
            lossless,
            container_only,
            entry_content_failures: total - lossless,
            lossless_pct: if total == 0 {
                0.0
            } else {
                (lossless as f64 / total as f64) * 100.0
            },
            distinct_producers: producers.len(),
            normalized: self.records.iter().filter(|r| r.normalized()).count(),
            // The number D12's gate actually reads. Producer strings on
            // rewritten files name the manager, not the original toolchain.
            distinct_unnormalized_producers: unnormalized_producers.len(),
            distinct_unnormalized_families: unnormalized_families.len(),
            largest_family_share_pct: family::largest_share_pct(&unnormalized_families),
            by_class,
            producers,
            unnormalized_producers,
            unnormalized_families,
            normalization_signals: signals,
        }
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Aggregate view of a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Books examined.
    pub total: usize,
    /// Books with no entry-content divergence — what ADR-F012 actually promises.
    pub lossless: usize,
    /// Lossless, but with container-level divergence worth reporting.
    pub container_only: usize,
    /// Books that failed on entry content.
    pub entry_content_failures: usize,
    /// Lossless share, the number F2's ≥99% criterion is read against.
    pub lossless_pct: f64,
    /// Distinct producer strings across every file. Read with care: on a
    /// normalized corpus this counts managers, not toolchains (R27).
    pub distinct_producers: usize,
    /// Files carrying at least one normalization signal.
    pub normalized: usize,
    /// Distinct producer *strings* among un-normalized files. Reported for
    /// transparency; the gate does not read it (ADR-F050).
    pub distinct_unnormalized_producers: usize,
    /// Distinct producer *families* among un-normalized files. **This is what
    /// D12's coverage gate reads** (ADR-F050).
    pub distinct_unnormalized_families: usize,
    /// Share of the largest family, as a percentage of the un-normalized
    /// population. A count cannot distinguish twenty families where one holds
    /// 95% from six held evenly; this is the half that can.
    pub largest_family_share_pct: f64,
    /// Occurrences per class, every class present.
    pub by_class: BTreeMap<String, usize>,
    /// Books per producer, all files.
    pub producers: BTreeMap<String, usize>,
    /// Books per producer string, un-normalized files only.
    pub unnormalized_producers: BTreeMap<String, usize>,
    /// Books per producer family, un-normalized files only. The gate's input,
    /// and named in the verdict so coverage can be argued with rather than
    /// trusted to a threshold.
    pub unnormalized_families: BTreeMap<String, usize>,
    /// How often each normalization signal fired.
    pub normalization_signals: BTreeMap<String, usize>,
}

/// Classes whose fixture gap is a property of the *synthetic writer*, not of
/// the instrument (ADR-F061).
///
/// The fixture builder pins timestamps and cannot vary deflate level, so
/// neither class can have a fixture — but real libraries carry both freely, and
/// the first `rbook` run observed `timestamp` on `mimetype` immediately. So the
/// gap is contingent, and it must dissolve on the first real corpus.
///
/// Written down now, while it is a prediction. Read after the run it would be a
/// rationalisation, and the two are indistinguishable once the number is on the
/// screen.
pub const CONTINGENT_GAPS: &[&str] = &["timestamp", "compression-level"];

/// Books below which a zero for a contingent gap says nothing.
///
/// Timestamp variation between a source and a rewritten copy is near-universal
/// — any writer that does not deliberately pin them produces it. Fifty books
/// from a real library all agreeing is not a quiet corpus; it is a writer or a
/// classifier normalising, and either is a finding.
pub const CONTINGENT_GAP_MIN_BOOKS: usize = 50;

impl Summary {
    /// Contingent gaps that failed to dissolve (ADR-F061).
    ///
    /// Empty below [`CONTINGENT_GAP_MIN_BOOKS`], because a small corpus that
    /// happens not to exercise a class is the ordinary case and reporting it
    /// would train the reader to skip the line.
    pub fn undissolved_contingent_gaps(&self) -> Vec<&'static str> {
        if self.total < CONTINGENT_GAP_MIN_BOOKS {
            return Vec::new();
        }
        CONTINGENT_GAPS
            .iter()
            .copied()
            .filter(|slug| self.by_class.get(*slug).copied().unwrap_or(0) == 0)
            .collect()
    }

    /// Render the human-facing report.
    pub fn render(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "books: {}  lossless: {} ({:.2}%)  entry-content failures: {}\n\
             container-only divergence: {}\n\
             normalized (rewritten by a manager): {} of {}\n\
             producers: {} strings overall, {} among un-normalized files\n\
             families among un-normalized files: {} (largest holds {:.0}%)\n\n",
            self.total,
            self.lossless,
            self.lossless_pct,
            self.entry_content_failures,
            self.container_only,
            self.normalized,
            self.total,
            self.distinct_producers,
            self.distinct_unnormalized_producers,
            self.distinct_unnormalized_families,
            self.largest_family_share_pct,
        ));
        if self.normalized > 0 {
            s.push_str(
                "normalization signals (R27 — these files' producer strings name the manager):\n",
            );
            for (sig, n) in &self.normalization_signals {
                s.push_str(&format!("  {n:>4}  {sig}\n"));
            }
            s.push('\n');
        }
        s.push_str("divergence classes (books affected):\n");
        for c in Class::ALL {
            let n = self.by_class.get(c.slug()).copied().unwrap_or(0);
            let group = match c.group() {
                Group::EntryContent => "A content ",
                Group::ContainerMetadata => "B container",
                Group::DeclaredActual => "C declared",
                Group::Unclassified => "U unnamed ",
            };
            // ADR-F058: a reported zero is a measurement only where something
            // can produce a non-zero. Printing `0` for a class the classifier
            // cannot emit states a result nothing measured — which is exactly
            // what `manifest-mismatch: 0` did for the whole life of this
            // instrument, from inside a histogram designed to be honest about
            // empty buckets.
            let count = if n == 0 && !c.classifier_emitted() {
                "— hand-entered only; not measured (ADR-F041)".to_owned()
            } else {
                n.to_string()
            };
            s.push_str(&format!("  {group}  {:<24} {count}\n", c.slug()));
        }
        s.push_str("\nproducers (all files):\n");
        for (p, n) in &self.producers {
            s.push_str(&format!("  {n:>4}  {p}\n"));
        }
        s.push_str("\nproducer families among un-normalized files — the gate reads this:\n");
        for (f, n) in &self.unnormalized_families {
            s.push_str(&format!("  {n:>4}  {f}\n"));
        }
        s.push_str("\nraw producer strings among un-normalized files:\n");
        if self.unnormalized_producers.is_empty() {
            s.push_str(
                "  (none — every file shows normalization evidence, so this\n\
                       \x20  corpus measures one writer regardless of what the\n\
                       \x20  producer strings above say)\n",
            );
        }
        for (p, n) in &self.unnormalized_producers {
            s.push_str(&format!("  {n:>4}  {p}\n"));
        }
        s
    }
}
