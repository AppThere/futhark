// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Normalization detection (ADR-F046, R27).
//!
//! The producer string is self-reported and gets overwritten. A file that has
//! passed through calibre carries calibre as its producer *and* calibre's zip
//! writer characteristics, erasing whatever toolchain actually built it. A
//! library curated with a management tool can therefore report healthy producer
//! diversity while measuring exactly one writer — which is the failure D12's
//! gate exists to catch, wearing the gate's own passing signal.
//!
//! So normalization is detected on evidence *other than* the producer string,
//! and reported as a separate axis. D12's coverage gate then counts producers
//! among files carrying no normalization signal, which is the only population
//! where the producer string still means what it says.
//!
//! Detection is deliberately over-eager. A false positive costs one file's
//! exclusion from the gate; a false negative silently inflates the diversity
//! number the gate is reading.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::archive::Archive;

/// One piece of evidence that a file has been rewritten by a manager.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Signal {
    /// `<meta name="calibre:*">` in the OPF. Calibre writes these on every save.
    CalibreMeta,
    /// `META-INF/calibre_bookmarks.txt` inside the container.
    CalibreBookmarks,
    /// `<meta name="Sigil version">`.
    SigilMeta,
    /// A `dc:contributor` or generator naming a known management or editing tool.
    /// Named separately from `producer` because the same string can appear in
    /// either field, and only one of them is the axis D12 reads.
    ContributorTool(String),
    /// A sibling `metadata.opf` — calibre's library layout writes one next to
    /// every book, so the file lives in a managed library even if its own bytes
    /// were left alone.
    SidecarMetadataOpf,
    /// A sibling `cover.jpg` alongside a `metadata.opf`-style layout.
    SidecarCover,
}

impl Signal {
    /// Stable label for the manifest and the report.
    pub fn label(&self) -> String {
        match self {
            Self::CalibreMeta => "calibre-meta".to_owned(),
            Self::CalibreBookmarks => "calibre-bookmarks".to_owned(),
            Self::SigilMeta => "sigil-meta".to_owned(),
            Self::ContributorTool(t) => format!("contributor:{t}"),
            Self::SidecarMetadataOpf => "sidecar-metadata-opf".to_owned(),
            Self::SidecarCover => "sidecar-cover".to_owned(),
        }
    }
}

/// Tools whose presence in a contributor or generator field means the file has
/// been through a manager or editor rather than an original publisher pipeline.
const MANAGER_TOOLS: [&str; 5] = ["calibre", "sigil", "epubcheck", "pandoc", "ebook-convert"];

/// What normalization evidence a file carries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Normalization {
    /// Every signal found. Empty means the file looks untouched by a manager.
    pub signals: Vec<Signal>,
}

impl Normalization {
    /// Whether any evidence of rewriting was found.
    pub fn is_normalized(&self) -> bool {
        !self.signals.is_empty()
    }

    /// Labels, for the manifest.
    pub fn labels(&self) -> Vec<String> {
        self.signals.iter().map(Signal::label).collect()
    }
}

/// Inspect a container and its surroundings for normalization evidence.
///
/// `path` is the file's location on disk, used only for sidecar detection. It
/// is never recorded for Tier B — the manifest carries the *signal*, not the
/// path that produced it.
pub fn detect(archive: &Archive, path: Option<&Path>) -> Normalization {
    let mut signals = Vec::new();

    if archive.get("META-INF/calibre_bookmarks.txt").is_some() {
        signals.push(Signal::CalibreBookmarks);
    }

    if let Some(opf) = archive
        .entries
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".opf"))
    {
        scan_opf(&opf.data, &mut signals);
    }

    if let Some(dir) = path.and_then(Path::parent) {
        if dir.join("metadata.opf").is_file() {
            signals.push(Signal::SidecarMetadataOpf);
        }
        if dir.join("cover.jpg").is_file() || dir.join("cover.jpeg").is_file() {
            signals.push(Signal::SidecarCover);
        }
    }

    signals.sort();
    signals.dedup();
    Normalization { signals }
}

/// Text-scan the OPF rather than parsing it.
///
/// The evidence here is the *presence* of marker strings, and a manager that
/// writes a malformed OPF is still a manager — an XML parse failure must not
/// read as "no normalization", which would be the R27 failure inside the R27
/// detector.
fn scan_opf(data: &[u8], signals: &mut Vec<Signal>) {
    let text = String::from_utf8_lossy(data).to_ascii_lowercase();

    // Anchored to the attribute position, not free text. A book titled
    // "Mastering calibre: a guide" contains the literal string `calibre:` and
    // must not be flagged for it — over-eager is the right bias, arbitrary is
    // not, and the negative test is what draws the line.
    if meta_name_starts_with(&text, "calibre:") {
        signals.push(Signal::CalibreMeta);
    }
    if meta_name_starts_with(&text, "sigil version") {
        signals.push(Signal::SigilMeta);
    }
    for tool in MANAGER_TOOLS {
        // Only inside a contributor or generator field, so a book *about*
        // calibre does not get flagged for saying so in its title.
        if field_mentions(&text, "contributor", tool) || field_mentions(&text, "generator", tool) {
            signals.push(Signal::ContributorTool(tool.to_owned()));
        }
    }
}

/// Whether any `name=` or `property=` attribute value starts with `prefix`.
/// Both quote styles, because the OPF was written by whoever wrote it.
fn meta_name_starts_with(text: &str, prefix: &str) -> bool {
    for attr in ["name=", "property="] {
        for quote in ['"', '\''] {
            let needle = format!("{attr}{quote}{prefix}");
            if text.contains(&needle) {
                return true;
            }
        }
    }
    false
}

/// Whether `tool` appears within a few hundred characters after a mention of
/// `field`. Crude on purpose: this is evidence-gathering, not parsing, and the
/// cost asymmetry favours over-detection.
fn field_mentions(text: &str, field: &str, tool: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text.get(from..).and_then(|t| t.find(field)) {
        let start = from + i;
        let end = (start + 400).min(text.len());
        if text.get(start..end).is_some_and(|w| w.contains(tool)) {
            return true;
        }
        from = start + field.len();
        if from >= text.len() {
            break;
        }
    }
    false
}
