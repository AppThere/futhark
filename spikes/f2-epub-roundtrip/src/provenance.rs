// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! What a floor rests on, and what it takes to widen one (ADR-F051).
//!
//! Split from `floor.rs` so the widening step and the type it produces stay in
//! one file with private fields between them. These are the inputs to that
//! step: the corpus behind a measurement, the decisions a human has to supply
//! for whatever the corpus could not settle, and the ways supplying them can
//! fail.

use serde::Serialize;

use crate::manifest::Summary;
use crate::verdict::{FAMILY_FLOOR, MAX_FAMILY_SHARE_PCT};

/// What corpus produced a floor. Travels with the floor and into anything
/// derived from it, because "which books was this measured on" is the first
/// question a requirements list has to answer and the first one a summary drops.
#[derive(Debug, Clone, Serialize)]
pub struct CorpusProvenance {
    /// Books examined.
    pub books: usize,
    /// Books showing evidence of having been rewritten by a manager (ADR-F046).
    pub normalized: usize,
    /// Producer families among un-normalized files, named.
    pub families: Vec<String>,
    /// Share held by the largest such family.
    pub largest_family_share_pct: f64,
}

impl CorpusProvenance {
    /// Read provenance off a run's summary.
    pub fn from_summary(summary: &Summary) -> Self {
        Self {
            books: summary.total,
            normalized: summary.normalized,
            families: summary.unnormalized_families.keys().cloned().collect(),
            largest_family_share_pct: summary.largest_family_share_pct,
        }
    }

    /// Whether *absence* in this corpus says anything about the ecosystem.
    ///
    /// It does only if nothing was normalized and the un-normalized population
    /// clears D12's gate in both directions. Anything less and a feature no
    /// book contained is a fact about the sample, not about EPUB — a manager's
    /// writer strips exactly the constructs being enumerated, so on a
    /// normalized corpus `CheckedAndAbsent` is worth no more than `NotCovered`.
    pub fn absence_is_evidence(&self) -> bool {
        self.books > 0
            && self.normalized == 0
            && self.families.len() >= FAMILY_FLOOR
            && self.largest_family_share_pct <= MAX_FAMILY_SHARE_PCT
    }
}

/// What a human decided about a feature the corpus could not settle.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case", tag = "disposition")]
pub enum Disposition {
    /// Preserve it anyway. The corpus's silence is not trusted.
    RequireAnyway {
        /// Why, in words that are not "we did not see it".
        because: String,
    },
    /// Out of scope for `futhark-epub`.
    Excluded {
        /// Why.
        because: String,
    },
}

impl Disposition {
    /// The reason given, whichever way the decision went.
    pub fn because(&self) -> &str {
        match self {
            Self::RequireAnyway { because } | Self::Excluded { because } => because,
        }
    }
}

/// Why a floor could not be widened. Widening fails loudly rather than
/// producing a thinner requirements list, because a short list and a complete
/// one look identical once either is pasted into a design document.
#[derive(Debug, Clone, thiserror::Error)]
pub enum WideningGap {
    /// Features the corpus could not settle and nobody dispositioned.
    #[error("{} feature(s) unsettled by this corpus and undispositioned: {}", .outstanding.len(), .outstanding.join(", "))]
    Unsettled {
        /// Catalogue ids still outstanding.
        outstanding: Vec<String>,
    },
    /// A disposition naming something not in the catalogue. A typo'd id would
    /// otherwise read as coverage.
    #[error("disposition names feature(s) absent from the catalogue: {}", .unknown.join(", "))]
    UnknownFeature {
        /// The unrecognised ids.
        unknown: Vec<String>,
    },
    /// A disposition with an empty reason, which is the silence again wearing a
    /// decision's clothes.
    #[error("disposition for {id} carries no reason")]
    EmptyReason {
        /// The feature whose reason was blank.
        id: String,
    },
}

/// How many features landed in each state. The arithmetic that replaces
/// "we looked at a lot of books".
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Counts {
    /// Catalogue size.
    pub catalogued: usize,
    /// Seen in at least one book.
    pub observed: usize,
    /// A detector ran and found none.
    pub checked_and_absent: usize,
    /// No detector exists.
    pub not_covered: usize,
}
