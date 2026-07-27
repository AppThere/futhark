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
    pub producer: Option<String>,
    /// EPUB version declared in the OPF, if readable.
    pub epub_version: Option<String>,
    /// Number of archive entries in the source.
    pub entry_count: usize,
    /// Every divergence observed.
    pub divergences: Vec<Divergence>,
    /// The instrument that produced this record (ADR-F041).
    pub classifier_version: String,
}

/// Which corpus tier a record came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Redistributable, in the repository.
    A,
    /// The developer's own library. Never committed; referenced by hash.
    B,
    /// Synthetic fixture, generated to validate the instrument.
    Fixture,
}

impl Record {
    /// Whether this book round-tripped losslessly in the sense ADR-F012 means.
    pub fn lossless(&self) -> bool {
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

        for r in &self.records {
            for c in r.classes() {
                *by_class.entry(c.slug().to_owned()).or_insert(0) += 1;
            }
            let p = r
                .producer
                .clone()
                .unwrap_or_else(|| "(undeclared)".to_owned());
            *producers.entry(p).or_insert(0) += 1;
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
            by_class,
            producers,
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
    /// Distinct producer strings. D12 gates on this, not on file count.
    pub distinct_producers: usize,
    /// Occurrences per class, every class present.
    pub by_class: BTreeMap<String, usize>,
    /// Books per producer.
    pub producers: BTreeMap<String, usize>,
}

impl Summary {
    /// Render the human-facing report.
    pub fn render(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "books: {}  lossless: {} ({:.2}%)  entry-content failures: {}\n\
             container-only divergence: {}  distinct producers: {}\n\n",
            self.total,
            self.lossless,
            self.lossless_pct,
            self.entry_content_failures,
            self.container_only,
            self.distinct_producers,
        ));
        s.push_str("divergence classes (books affected):\n");
        for c in Class::ALL {
            let n = self.by_class.get(c.slug()).copied().unwrap_or(0);
            let group = match c.group() {
                Group::EntryContent => "A content ",
                Group::ContainerMetadata => "B container",
                Group::DeclaredActual => "C declared",
                Group::Unclassified => "U unnamed ",
            };
            s.push_str(&format!("  {group}  {:<24} {n}\n", c.slug()));
        }
        s.push_str("\nproducers:\n");
        for (p, n) in &self.producers {
            s.push_str(&format!("  {n:>4}  {p}\n"));
        }
        s
    }
}
