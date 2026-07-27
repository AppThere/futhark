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

/// Distinct producer *families* a corpus needs before a clean run qualifies.
///
/// Set by D12 at 0.12.0: the wild population runs to roughly ten or twelve
/// families, and eight is defensible coverage of it. Chosen for what makes the
/// result meaningful rather than for what any particular library contains — a
/// corpus that cannot clear the bar produces [`Verdict::Inconclusive`], which
/// is the gate working rather than failing.
pub const FAMILY_FLOOR: usize = 8;

/// No single family may exceed this share of the un-normalized population.
///
/// The second half of the gate, and the reason a bare count was the wrong
/// instrument: twenty families where one holds 95% is a worse corpus than six
/// held evenly, and a count cannot tell them apart.
pub const MAX_FAMILY_SHARE_PCT: f64 = 40.0;

/// Which half of the coverage gate a corpus failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shortfall {
    /// Too few distinct families.
    TooFewFamilies,
    /// Enough families, but one dominates the population.
    TooConcentrated,
}

/// What a run supports concluding. Deliberately lacks a plain `Pass`.
///
/// `Eq` is not derived: a share percentage is a float. Comparisons in tests go
/// through `matches!` on the variant, which is the property that matters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Clean, but on a corpus too normalized, too narrow, or too concentrated
    /// to support adoption. **Not a pass.**
    Inconclusive {
        /// Why the corpus fell short.
        reason: Shortfall,
        /// Families found, named. A threshold can be trusted or not; a named
        /// list can be argued with, which is the point (ADR-F050).
        families: Vec<String>,
        /// Share held by the largest family.
        largest_family_share_pct: f64,
        /// Files showing normalization evidence.
        normalized: usize,
        /// Books examined.
        total: usize,
    },
    /// Clean, on a corpus with genuine family diversity. The only result that
    /// supports confirming a reader.
    Qualifying {
        /// Families found, named.
        families: Vec<String>,
        /// Share held by the largest family.
        largest_family_share_pct: f64,
        /// Books examined.
        total: usize,
    },
    /// Nothing was measured. Distinguished from a pass on purpose (ADR-F042):
    /// an empty corpus and a clean corpus produce the same failure count.
    NothingMeasured,
}

impl Verdict {
    /// Read a summary against ADR-F047's asymmetry and D12's coverage gate.
    pub fn judge(summary: &Summary, family_floor: usize) -> Self {
        if summary.total == 0 {
            return Self::NothingMeasured;
        }
        if summary.lossless_pct < LOSSLESS_THRESHOLD_PCT {
            return Self::Disqualifying {
                failures: summary.entry_content_failures,
                total: summary.total,
            };
        }

        let families: Vec<String> = summary.unnormalized_families.keys().cloned().collect();
        let shortfall = if summary.distinct_unnormalized_families < family_floor {
            Some(Shortfall::TooFewFamilies)
        } else if summary.largest_family_share_pct > MAX_FAMILY_SHARE_PCT {
            Some(Shortfall::TooConcentrated)
        } else {
            None
        };

        match shortfall {
            Some(reason) => Self::Inconclusive {
                reason,
                families,
                largest_family_share_pct: summary.largest_family_share_pct,
                normalized: summary.normalized,
                total: summary.total,
            },
            None => Self::Qualifying {
                families,
                largest_family_share_pct: summary.largest_family_share_pct,
                total: summary.total,
            },
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
                reason,
                families,
                largest_family_share_pct,
                normalized,
                total,
            } => {
                let why = match reason {
                    Shortfall::TooFewFamilies => format!(
                        "{} distinct producer families, against a floor of {FAMILY_FLOOR}",
                        families.len()
                    ),
                    Shortfall::TooConcentrated => format!(
                        "{} families, but the largest holds {largest_family_share_pct:.0}% \
                         against a ceiling of {MAX_FAMILY_SHARE_PCT:.0}%",
                        families.len()
                    ),
                };
                format!(
                    "INCONCLUSIVE — no entry-content divergence, but this corpus cannot\n\
                     support adoption: {why}.\n\
                     {normalized} of {total} files show normalization evidence.\n\n\
                     families counted: {}\n\n\
                     A manager's writer strips exactly what breaks parsers, so this run\n\
                     screened the easy population and found nothing. That is not the\n\
                     same as passing (ADR-F047).",
                    if families.is_empty() {
                        "(none)".to_owned()
                    } else {
                        families.join(", ")
                    },
                )
            }
            Self::Qualifying {
                families,
                largest_family_share_pct,
                total,
            } => format!(
                "QUALIFYING — no entry-content divergence across {total} books from\n\
                 {} producer families, largest holding {largest_family_share_pct:.0}%.\n\n\
                 families counted: {}\n\n\
                 This result can support confirming the reader under test.",
                families.len(),
                families.join(", "),
            ),
            Self::NothingMeasured => {
                "NOTHING MEASURED — the corpus was empty. This is not a pass; \
                 zero failures out of zero books is not evidence."
                    .to_owned()
            }
        }
    }
}
