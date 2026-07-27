// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F047: F2 on a normalized corpus is a screening test, not a qualifying one.
//!
//! A manager's writer normalizes exactly the constructs that break parsers —
//! odd namespaces, comments, processing instructions, irregular zip structure —
//! so a normalized corpus is systematically *easier* than the wild population.
//! That makes the result one-directional:
//!
//! - **Failure is decisive.** The reader could not handle even the easy case.
//! - **A pass is nearly uninformative.** The hard cases were not in the sample.
//!
//! This is encoded as a type rather than left to the reader of a report,
//! because "the pass was on an easy corpus" is precisely the kind of caveat
//! that survives in a findings document and evaporates in a summary. There is
//! no `Verdict::Pass`; a clean run on a screening corpus is
//! [`Verdict::Inconclusive`], and only a corpus with real producer diversity
//! can produce [`Verdict::Qualifying`].
//!
//! Which is the standing review question applied to F2's own green result:
//! what else could produce a clean round-trip? An easy corpus could.

use serde::{Deserialize, Serialize};

use crate::manifest::Summary;

/// F2's pass criterion: entry-content divergence ≥99% clean (spec §10).
pub const LOSSLESS_THRESHOLD_PCT: f64 = 99.0;

/// How many distinct un-normalized producers a corpus needs before a clean run
/// counts as qualifying rather than screening.
///
/// **Provisional — not a resolved decision.** D12 settled that the gate is
/// coverage rather than file count, but did not set the number. This is a
/// placeholder so the tool has a defined behaviour; override with
/// `--min-producers` and record the real figure in the spec when it is chosen.
pub const PROVISIONAL_PRODUCER_FLOOR: usize = 5;

/// What a run supports concluding. Deliberately lacks a plain `Pass`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "verdict")]
pub enum Verdict {
    /// Entry-content divergence exceeded the threshold. Decisive on any corpus:
    /// a reader that cannot round-trip the easy population will not do better
    /// on the hard one. ADR-F011 resolves against the reader.
    Disqualifying {
        /// Books that lost entry content.
        failures: usize,
        /// Books examined.
        total: usize,
    },
    /// Clean, but on a corpus too normalized or too narrow to support adoption.
    /// **Not a pass.** ADR-F011 stays open.
    Inconclusive {
        /// Distinct producers among un-normalized files.
        unnormalized_producers: usize,
        /// What that count would have to reach.
        floor: usize,
        /// Files showing normalization evidence.
        normalized: usize,
        /// Books examined.
        total: usize,
    },
    /// Clean, on a corpus with genuine producer diversity. This is the only
    /// result that supports confirming ADR-F011.
    Qualifying {
        /// Distinct producers among un-normalized files.
        unnormalized_producers: usize,
        /// Books examined.
        total: usize,
    },
    /// Nothing was measured. Distinguished from a pass on purpose (ADR-F042):
    /// an empty corpus and a clean corpus produce the same failure count.
    NothingMeasured,
}

impl Verdict {
    /// Read a summary against ADR-F047's asymmetry.
    pub fn judge(summary: &Summary, producer_floor: usize) -> Self {
        if summary.total == 0 {
            return Self::NothingMeasured;
        }
        if summary.lossless_pct < LOSSLESS_THRESHOLD_PCT {
            return Self::Disqualifying {
                failures: summary.entry_content_failures,
                total: summary.total,
            };
        }
        if summary.distinct_unnormalized_producers < producer_floor {
            return Self::Inconclusive {
                unnormalized_producers: summary.distinct_unnormalized_producers,
                floor: producer_floor,
                normalized: summary.normalized,
                total: summary.total,
            };
        }
        Self::Qualifying {
            unnormalized_producers: summary.distinct_unnormalized_producers,
            total: summary.total,
        }
    }

    /// Whether this result may be cited in support of adopting the reader.
    /// Only one variant can.
    pub fn supports_adoption(&self) -> bool {
        matches!(self, Self::Qualifying { .. })
    }

    /// Whether ADR-F011 can be resolved on this evidence, either way.
    pub fn resolves_adr_f011(&self) -> bool {
        matches!(self, Self::Disqualifying { .. } | Self::Qualifying { .. })
    }

    /// The finding, stated so it cannot be misread as something stronger.
    pub fn render(&self) -> String {
        match self {
            Self::Disqualifying { failures, total } => format!(
                "DISQUALIFYING — {failures} of {total} books lost entry content.\n\
                 Decisive regardless of corpus quality: a normalized corpus is the\n\
                 easy case, and the reader failed it. ADR-F011 resolves against\n\
                 rbook; futhark-epub becomes a bespoke build over quick-xml.",
            ),
            Self::Inconclusive {
                unnormalized_producers,
                floor,
                normalized,
                total,
            } => format!(
                "INCONCLUSIVE — no entry-content divergence, but this corpus cannot\n\
                 support adoption. {normalized} of {total} files show normalization\n\
                 evidence, leaving {unnormalized_producers} distinct un-normalized\n\
                 producer(s) against a floor of {floor}.\n\n\
                 A manager's writer strips exactly what breaks parsers, so this run\n\
                 screened the easy population and found nothing. That is not the\n\
                 same as passing (ADR-F047). ADR-F011 stays open pending a corpus\n\
                 with real producer diversity.",
            ),
            Self::Qualifying {
                unnormalized_producers,
                total,
            } => format!(
                "QUALIFYING — no entry-content divergence across {total} books from\n\
                 {unnormalized_producers} distinct un-normalized producers. This\n\
                 result can support confirming ADR-F011.",
            ),
            Self::NothingMeasured => {
                "NOTHING MEASURED — the corpus was empty. This is not a pass; \
                 zero failures out of zero books is not evidence."
                    .to_owned()
            }
        }
    }
}
