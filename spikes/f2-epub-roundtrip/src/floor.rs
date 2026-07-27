// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F051: F2b's output is a **floor**, and the type says so.
//!
//! The failure this exists to prevent has already happened once in this
//! project, in prose: §2 of the F2 findings said "this does not resolve
//! ADR-F011" and §6 opened with "Reverse ADR-F011", and the heading won. A
//! caveat that lives in a document separates from the claim it qualifies.
//!
//! So the caveat is a type instead. [`FeatureFloor`] has no method that yields
//! a requirements list for `futhark-epub`. The only route to [`Requirements`]
//! is [`FeatureFloor::widen`], which fails unless every feature the corpus
//! could not settle carries an explicit [`Disposition`] with a reason — and
//! which stamps the resulting `Requirements` with the corpus that produced it
//! and whether that corpus was normalized.
//!
//! Which features count as unsettled depends on the corpus, not on taste. On a
//! normalized corpus, `CheckedAndAbsent` is worth no more than `NotCovered`: a
//! manager's writer strips exactly the constructs being enumerated, so its
//! silence is about the writer. That is ADR-F047's asymmetry applied to
//! enumeration rather than to round-tripping.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::catalogue::{Area, CATALOGUE, CATALOGUE_VERSION, FeatureState};
use crate::manifest::Summary;
use crate::provenance::{CorpusProvenance, Counts, Disposition, WideningGap};

/// What F2b measured: a lower bound on what `futhark-epub` must preserve.
///
/// Named for what it is. There is deliberately no `requirements()`,
/// `into_spec()`, or `Deref` to a feature list — see [`FeatureFloor::widen`].
#[derive(Debug, Clone, Serialize)]
pub struct FeatureFloor {
    /// Catalogue this was measured against (ADR-F050). A floor from a smaller
    /// catalogue is not comparable to one from a larger.
    pub catalogue_version: String,
    /// The corpus behind it.
    pub provenance: CorpusProvenance,
    /// State per catalogue id.
    pub states: BTreeMap<String, FeatureState>,
}

impl FeatureFloor {
    /// Assemble a floor from accumulated states and a run summary.
    pub fn new(states: BTreeMap<String, FeatureState>, summary: &Summary) -> Self {
        Self {
            catalogue_version: CATALOGUE_VERSION.to_owned(),
            provenance: CorpusProvenance::from_summary(summary),
            states,
        }
    }

    fn state_of(&self, id: &str) -> FeatureState {
        self.states
            .get(id)
            .copied()
            .unwrap_or(FeatureState::NotCovered)
    }

    /// Features seen. These are requirements on any corpus: one book containing
    /// a comment is enough to prove comments must survive.
    pub fn observed(&self) -> Vec<&'static str> {
        CATALOGUE
            .iter()
            .filter(|f| self.state_of(f.id) == FeatureState::Observed)
            .map(|f| f.id)
            .collect()
    }

    /// Features this corpus cannot speak to — always the `NotCovered` ones, and
    /// the `CheckedAndAbsent` ones too unless the corpus earns the right to
    /// have its silence believed.
    pub fn unsettled(&self) -> Vec<&'static str> {
        let trust_absence = self.provenance.absence_is_evidence();
        CATALOGUE
            .iter()
            .filter(|f| match self.state_of(f.id) {
                FeatureState::Observed => false,
                FeatureState::CheckedAndAbsent => !trust_absence,
                FeatureState::NotCovered => true,
            })
            .map(|f| f.id)
            .collect()
    }

    /// State tally.
    pub fn counts(&self) -> Counts {
        let mut c = Counts {
            catalogued: CATALOGUE.len(),
            observed: 0,
            checked_and_absent: 0,
            not_covered: 0,
        };
        for f in CATALOGUE {
            match self.state_of(f.id) {
                FeatureState::Observed => c.observed += 1,
                FeatureState::CheckedAndAbsent => c.checked_and_absent += 1,
                FeatureState::NotCovered => c.not_covered += 1,
            }
        }
        c
    }

    /// The one route from a floor to something consumable as a specification.
    ///
    /// Every feature in [`Self::unsettled`] must carry a reasoned
    /// [`Disposition`]. The result records the corpus it came from, so the
    /// widening cannot be laundered into an unqualified list further downstream.
    pub fn widen(
        &self,
        dispositions: &BTreeMap<String, Disposition>,
    ) -> Result<Requirements, WideningGap> {
        let known: BTreeSet<&str> = CATALOGUE.iter().map(|f| f.id).collect();
        let unknown: Vec<String> = dispositions
            .keys()
            .filter(|k| !known.contains(k.as_str()))
            .cloned()
            .collect();
        if !unknown.is_empty() {
            return Err(WideningGap::UnknownFeature { unknown });
        }
        for (id, d) in dispositions {
            if d.because().trim().is_empty() {
                return Err(WideningGap::EmptyReason { id: id.clone() });
            }
        }

        let outstanding: Vec<String> = self
            .unsettled()
            .into_iter()
            .filter(|id| !dispositions.contains_key(*id))
            .map(str::to_owned)
            .collect();
        if !outstanding.is_empty() {
            return Err(WideningGap::Unsettled { outstanding });
        }

        let mut must_preserve: Vec<String> =
            self.observed().into_iter().map(str::to_owned).collect();
        let mut excluded: Vec<Exclusion> = Vec::new();
        for (id, d) in dispositions {
            match d {
                Disposition::RequireAnyway { .. } => must_preserve.push(id.clone()),
                Disposition::Excluded { because } => excluded.push(Exclusion {
                    id: id.clone(),
                    because: because.clone(),
                }),
            }
        }
        must_preserve.sort();
        must_preserve.dedup();

        Ok(Requirements {
            catalogue_version: self.catalogue_version.clone(),
            provenance: self.provenance.clone(),
            must_preserve,
            excluded,
        })
    }

    /// Render the floor, with the arithmetic ADR-F050 exists to produce.
    pub fn render(&self) -> String {
        let c = self.counts();
        let mut s = format!(
            "FEATURE FLOOR — catalogue {}, {} books ({} normalized)\n\
             This is a lower bound. It is not a specification for futhark-epub\n\
             and cannot become one without widening (ADR-F051).\n\n\
             {} catalogued: {} observed, {} checked and absent, {} never looked at\n\
             absence in this corpus is evidence: {}\n\n",
            self.catalogue_version,
            self.provenance.books,
            self.provenance.normalized,
            c.catalogued,
            c.observed,
            c.checked_and_absent,
            c.not_covered,
            self.provenance.absence_is_evidence(),
        );
        for area in [Area::Infoset, Area::Container, Area::Package] {
            s.push_str(&format!("{area:?}\n"));
            for f in CATALOGUE.iter().filter(|f| f.area == area) {
                s.push_str(&format!(
                    "  {:<20} {:<42} {}\n",
                    format!("{:?}", self.state_of(f.id)),
                    f.id,
                    f.why
                ));
            }
        }
        s.push_str(&format!(
            "\nunsettled by this corpus ({}): {}\n",
            self.unsettled().len(),
            self.unsettled().join(", ")
        ));
        s
    }
}

/// A feature deliberately left out, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Exclusion {
    /// Catalogue id.
    pub id: String,
    /// The reason given at widening time.
    pub because: String,
}

/// A widened floor: the only form of F2b's output that may be read as a
/// specification for `futhark-epub`.
///
/// Fields are private and there is no public constructor, so the sole way to
/// obtain one is [`FeatureFloor::widen`]. `Deserialize` is deliberately not
/// derived — a `serde_json::from_str::<Requirements>` would be a back door
/// around the widening step, and the point of ADR-F051 is that there is no back
/// door.
#[derive(Debug, Clone, Serialize)]
pub struct Requirements {
    catalogue_version: String,
    provenance: CorpusProvenance,
    must_preserve: Vec<String>,
    excluded: Vec<Exclusion>,
}

impl Requirements {
    /// Features `futhark-epub` must preserve.
    pub fn must_preserve(&self) -> &[String] {
        &self.must_preserve
    }

    /// Features excluded, with reasons.
    pub fn excluded(&self) -> &[Exclusion] {
        &self.excluded
    }

    /// The corpus this rests on. Not droppable: it is carried by the type.
    pub fn provenance(&self) -> &CorpusProvenance {
        &self.provenance
    }

    /// Catalogue version behind it.
    pub fn catalogue_version(&self) -> &str {
        &self.catalogue_version
    }
}
