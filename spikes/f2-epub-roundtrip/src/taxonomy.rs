// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The frozen divergence taxonomy (ADR-F036, ADR-F041).
//!
//! This file is the instrument's calibration. It is frozen before Tier B runs,
//! and Tier B may only add [`Class::Unclassified`] observations — never move an
//! existing boundary. Fitting the classes to the corpus they are measuring would
//! make the F2 result meaningless, so a change here is a version bump and a
//! re-run, not an edit.
//!
//! Three top-level groups, per ADR-F036:
//!
//! - **A — entry content.** The only group that bears on ADR-F012. A save that
//!   changes any byte of any archive entry has broken the lossless promise.
//! - **B — container metadata.** Reported, never fatal. A reader that reorders
//!   entries or rewrites timestamps while preserving every entry byte-for-byte
//!   satisfies ADR-F012 completely.
//! - **C — declared versus actual.** Structural claims the package makes about
//!   itself that the bytes contradict. Not a fidelity question; a validity one.

use serde::{Deserialize, Serialize};

/// Bumped whenever a class is added or its boundary changes. Recorded per
/// manifest entry (ADR-F038) so a run can be reproduced against the exact
/// instrument that produced it.
pub const CLASSIFIER_VERSION: &str = "1.0.0";

/// The date the taxonomy was frozen, for the record.
pub const TAXONOMY_FROZEN: &str = "2026-07-27";

/// Which of ADR-F036's three groups a class belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Group {
    /// Entry content. Bears on ADR-F012.
    EntryContent,
    /// Archive container metadata. Reported, not fatal.
    ContainerMetadata,
    /// Declared-versus-actual structural mismatch.
    DeclaredActual,
    /// Observed, named by no existing class. ADR-F041's escape hatch.
    Unclassified,
}

/// A single named divergence class. Adding a variant is a `CLASSIFIER_VERSION`
/// bump; removing or redefining one invalidates every prior manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    // --- A: entry content -------------------------------------------------
    /// Entry bytes differ in a way no narrower class explains.
    ContentByteDiff,
    /// Present in the source, absent from the round-trip.
    EntryMissing,
    /// Present in the round-trip, absent from the source.
    EntryAdded,
    /// Bytes differ but the XML infoset does not: attribute order, quote style,
    /// self-closing form, insignificant whitespace inside tags. Still an
    /// ADR-F012 violation — the editor promises bytes, not infosets — but a
    /// materially different one from losing content, and the fix is different.
    XmlCanonicalization,
    /// Differs only by a byte-order mark or an encoding declaration.
    TextEncoding,
    /// Differs only by CRLF versus LF.
    LineEndings,

    // --- B: container metadata --------------------------------------------
    /// Same entries, different order in the central directory.
    EntryOrder,
    /// Entry modification timestamps differ.
    Timestamp,
    /// Stored versus deflated, with identical uncompressed bytes.
    CompressionMethod,
    /// Same method and same uncompressed bytes, different compressed size.
    CompressionLevel,
    /// Zip extra fields differ.
    ExtraField,
    /// Archive or entry comment differs.
    Comment,

    // --- C: declared versus actual ----------------------------------------
    /// OCF requires `mimetype` to be the first entry.
    MimetypeNotFirst,
    /// OCF requires `mimetype` to be stored uncompressed.
    MimetypeCompressed,
    /// A local header's declared size or CRC contradicts the entry's bytes.
    DeclaredSizeMismatch,
    /// The OPF manifest and the container disagree about what exists.
    ManifestMismatch,

    // --- Escape hatch -----------------------------------------------------
    /// Something real was observed that no class above names. ADR-F041 permits
    /// Tier B to land here; it does not permit silently widening a class to
    /// swallow it. A non-empty bucket is a finding, not a failure.
    Unclassified,
}

impl Class {
    /// The group this class belongs to.
    pub const fn group(self) -> Group {
        match self {
            Self::ContentByteDiff
            | Self::EntryMissing
            | Self::EntryAdded
            | Self::XmlCanonicalization
            | Self::TextEncoding
            | Self::LineEndings => Group::EntryContent,

            Self::EntryOrder
            | Self::Timestamp
            | Self::CompressionMethod
            | Self::CompressionLevel
            | Self::ExtraField
            | Self::Comment => Group::ContainerMetadata,

            Self::MimetypeNotFirst
            | Self::MimetypeCompressed
            | Self::DeclaredSizeMismatch
            | Self::ManifestMismatch => Group::DeclaredActual,

            Self::Unclassified => Group::Unclassified,
        }
    }

    /// Whether this class counts against F2's pass criterion.
    ///
    /// Only entry-content divergence does. That is the whole point of
    /// classifying before counting: a bare byte-difference percentage over a
    /// zip archive measures the writer's zip library, not its fidelity.
    ///
    /// `Unclassified` counts as failing. An observation the instrument cannot
    /// name must not be scored as harmless — that is the same error as scoring
    /// an unobservable sandbox result as a pass (ADR-F042).
    pub const fn bears_on_losslessness(self) -> bool {
        matches!(self.group(), Group::EntryContent | Group::Unclassified)
    }

    /// Stable slug for manifests and reports.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::ContentByteDiff => "content-byte-diff",
            Self::EntryMissing => "entry-missing",
            Self::EntryAdded => "entry-added",
            Self::XmlCanonicalization => "xml-canonicalization",
            Self::TextEncoding => "text-encoding",
            Self::LineEndings => "line-endings",
            Self::EntryOrder => "entry-order",
            Self::Timestamp => "timestamp",
            Self::CompressionMethod => "compression-method",
            Self::CompressionLevel => "compression-level",
            Self::ExtraField => "extra-field",
            Self::Comment => "comment",
            Self::MimetypeNotFirst => "mimetype-not-first",
            Self::MimetypeCompressed => "mimetype-compressed",
            Self::DeclaredSizeMismatch => "declared-size-mismatch",
            Self::ManifestMismatch => "manifest-mismatch",
            Self::Unclassified => "unclassified",
        }
    }

    /// Every class, for reporting a full histogram including empty buckets —
    /// an absent class and a zero-count class are different claims.
    pub const ALL: [Self; 17] = [
        Self::ContentByteDiff,
        Self::EntryMissing,
        Self::EntryAdded,
        Self::XmlCanonicalization,
        Self::TextEncoding,
        Self::LineEndings,
        Self::EntryOrder,
        Self::Timestamp,
        Self::CompressionMethod,
        Self::CompressionLevel,
        Self::ExtraField,
        Self::Comment,
        Self::MimetypeNotFirst,
        Self::MimetypeCompressed,
        Self::DeclaredSizeMismatch,
        Self::ManifestMismatch,
        Self::Unclassified,
    ];
}

/// One observed divergence: the class, where it was seen, and enough detail to
/// act on without holding the book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Divergence {
    /// Which class this observation falls into.
    pub class: Class,
    /// Archive entry the divergence was observed in, where applicable.
    pub entry: Option<String>,
    /// Human-readable specifics. Never the entry's contents — ADR-F038 permits
    /// publishing the manifest precisely because it carries no book bytes.
    pub detail: String,
}

impl Divergence {
    /// Construct a divergence attached to a named entry.
    pub fn entry(class: Class, entry: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            class,
            entry: Some(entry.into()),
            detail: detail.into(),
        }
    }

    /// Construct a divergence about the archive as a whole.
    pub fn archive(class: Class, detail: impl Into<String>) -> Self {
        Self {
            class,
            entry: None,
            detail: detail.into(),
        }
    }
}
