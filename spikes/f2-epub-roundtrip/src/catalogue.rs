// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! F2b's fixed feature catalogue and its three states.
//!
//! ADR-F050 (feature catalogue): an open-ended "what did we find" list cannot
//! distinguish a construct the corpus lacked from one the harness never looked
//! for. Both come back as silence. A fixed catalogue with three states can:
//!
//! - [`FeatureState::Observed`] — present in this corpus.
//! - [`FeatureState::CheckedAndAbsent`] — a detector ran and found none.
//! - [`FeatureState::NotCovered`] — **no detector exists.** Says nothing about
//!   the corpus, only about the harness.
//!
//! That turns the ceiling problem from rhetoric into arithmetic: *N features
//! checked, k observed, m explicitly absent, j never looked at.* A reader can
//! compute the floor rather than being asked to remember a caveat.
//!
//! It is `Class::Unclassified` and ADR-F042 a third time — absence of a signal
//! must never be scored as absence of the thing — applied to a deliverable that
//! has no verdict to carry the distinction.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::archive::Archive;
use crate::detect::detect;

/// Version of the catalogue itself. Adding a feature is a bump, because a floor
/// computed against a smaller catalogue is not comparable to a later one.
pub const CATALOGUE_VERSION: &str = "1.0.0";

/// What is known about one feature, in this corpus, with this harness.
///
/// Deliberately not `Ord`. There is a lattice here — see [`FeatureState::merge`]
/// — but it is not the declaration order, and a derived `Ord` would offer
/// `max()` as a plausible-looking way to combine two books that silently
/// prefers `Observed` to nothing and `NotCovered` to `CheckedAndAbsent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureState {
    /// Seen in at least one book.
    Observed,
    /// A detector ran across the corpus and found none. Evidence about the
    /// corpus — and, if the corpus is normalized, weak evidence at that.
    CheckedAndAbsent,
    /// No detector exists. Evidence about the *harness*, not the corpus. Must
    /// never be read as "the ecosystem does not use this".
    NotCovered,
}

impl FeatureState {
    /// Fold in what another book showed. Knowledge only increases:
    /// `NotCovered` → `CheckedAndAbsent` → `Observed`, never back down.
    ///
    /// The middle step is the one worth stating: a corpus starts entirely
    /// `NotCovered`, and a detector running and finding nothing is a real
    /// promotion — it is the difference between "we did not look" and "we
    /// looked". Leaving it at `NotCovered` would make a working detector
    /// indistinguishable from an absent one.
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Observed, _) | (_, Self::Observed) => Self::Observed,
            (Self::CheckedAndAbsent, _) | (_, Self::CheckedAndAbsent) => Self::CheckedAndAbsent,
            (Self::NotCovered, Self::NotCovered) => Self::NotCovered,
        }
    }
}

/// Where a feature lives, for grouping the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Area {
    /// XML serialisation detail inside a content document or the OPF.
    Infoset,
    /// Zip container structure.
    Container,
    /// EPUB package-level structure.
    Package,
}

/// One catalogued feature `futhark-epub` may have to preserve.
#[derive(Debug, Clone, Copy)]
pub struct Feature {
    /// Stable slug.
    pub id: &'static str,
    /// Where it lives.
    pub area: Area,
    /// Why preserving it matters.
    pub why: &'static str,
}

/// The catalogue. Fixed, versioned, and deliberately containing entries with no
/// detector — a catalogue whose every entry is covered cannot demonstrate the
/// difference between `CheckedAndAbsent` and `NotCovered`, which is the whole
/// reason it has three states.
pub const CATALOGUE: &[Feature] = &[
    // --- Infoset -----------------------------------------------------------
    Feature {
        id: "xml-comment",
        area: Area::Infoset,
        why: "ADR-F012 names comments explicitly",
    },
    Feature {
        id: "processing-instruction",
        area: Area::Infoset,
        why: "Dropped silently by most serialisers",
    },
    Feature {
        id: "cdata-section",
        area: Area::Infoset,
        why: "Re-serialises as escaped text, changing bytes",
    },
    Feature {
        id: "doctype-declaration",
        area: Area::Infoset,
        why: "Legacy EPUB 2 content usually carries one",
    },
    Feature {
        id: "internal-entity",
        area: Area::Infoset,
        why: "A DTD-declared entity expands and never comes back",
    },
    Feature {
        id: "byte-order-mark",
        area: Area::Infoset,
        why: "Present or absent, and both must survive",
    },
    Feature {
        id: "crlf-line-endings",
        area: Area::Infoset,
        why: "Windows toolchains emit them; normalising is a byte change",
    },
    Feature {
        id: "non-alphabetical-attribute-order",
        area: Area::Infoset,
        why: "ADR-F012 names attribute order",
    },
    Feature {
        id: "uncommon-namespace-prefix",
        area: Area::Infoset,
        why: "Prefixes beyond the EPUB set must round-trip verbatim",
    },
    Feature {
        id: "xml-standalone-declaration",
        area: Area::Infoset,
        why: "Part of the declaration, easily dropped",
    },
    // --- Container ---------------------------------------------------------
    Feature {
        id: "stored-entry-beyond-mimetype",
        area: Area::Container,
        why: "Some writers store more than mimetype",
    },
    Feature {
        id: "zip-entry-comment",
        area: Area::Container,
        why: "Rare, and lost by every naive rewriter",
    },
    Feature {
        id: "archive-comment",
        area: Area::Container,
        why: "Same",
    },
    Feature {
        id: "zip-extra-field",
        area: Area::Container,
        why: "Unix permissions and timestamps ride here",
    },
    Feature {
        id: "non-ascii-entry-name",
        area: Area::Container,
        why: "Encoding flag handling differs between writers",
    },
    // --- Package -----------------------------------------------------------
    Feature {
        id: "epub2-package",
        area: Area::Package,
        why: "OPF 2 paths differ from EPUB 3 throughout",
    },
    Feature {
        id: "ncx-present",
        area: Area::Package,
        why: "EPUB 2 navigation, still common in EPUB 3 for compatibility",
    },
    Feature {
        id: "opf-guide-element",
        area: Area::Package,
        why: "Deprecated but widespread",
    },
    Feature {
        id: "encryption-xml",
        area: Area::Package,
        why: "Font obfuscation; ADR-F021 stops at DRM, not obfuscation",
    },
    Feature {
        id: "scripted-content",
        area: Area::Package,
        why: "ADR-F006 strips it, but it must be detected to be stripped",
    },
    Feature {
        id: "remote-resource",
        area: Area::Package,
        why: "Sandbox denies it; the reader still has to see it",
    },
    // --- Deliberately uncovered -------------------------------------------
    // These have no detector. They are in the catalogue so the gap is counted
    // rather than invisible, which is the point of the third state.
    Feature {
        id: "whitespace-significant-in-mixed-content",
        area: Area::Infoset,
        why: "Needs a round-trip diff, not a scan",
    },
    Feature {
        id: "duplicate-entry-names",
        area: Area::Container,
        why: "The zip reader collapses them before we see them",
    },
    Feature {
        id: "zip64-container",
        area: Area::Container,
        why: "No large-file corpus to hand",
    },
    Feature {
        id: "data-descriptor-entries",
        area: Area::Container,
        why: "Not surfaced by the zip reader in use",
    },
    // `Archive::read` skips `is_dir()` entries, so a detector here would have
    // reported `CheckedAndAbsent` on every corpus ever scanned — a clean signal
    // produced by the reader's filter rather than by the books. The standing
    // review question, caught inside the instrument built to answer it.
    Feature {
        id: "directory-entry",
        area: Area::Container,
        why: "Explicit directory records change the entry list",
    },
];

/// Fold one book's detections into the running state.
pub fn accumulate(states: &mut BTreeMap<String, FeatureState>, archive: &Archive) {
    let detected = detect(archive);
    for f in CATALOGUE {
        let state = match detected.get(f.id) {
            Some(true) => FeatureState::Observed,
            Some(false) => FeatureState::CheckedAndAbsent,
            None => FeatureState::NotCovered,
        };
        let entry = states
            .entry(f.id.to_owned())
            .or_insert(FeatureState::NotCovered);
        *entry = entry.merge(state);
    }
}

/// Start every feature at `NotCovered`, so a corpus of zero books reports the
/// harness's coverage rather than an empty ecosystem.
pub fn initial_states() -> BTreeMap<String, FeatureState> {
    CATALOGUE
        .iter()
        .map(|f| (f.id.to_owned(), FeatureState::NotCovered))
        .collect()
}
