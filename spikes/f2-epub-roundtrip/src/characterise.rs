// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Spike F2b — what do real EPUBs actually contain?
//!
//! F2 asked whether a reader round-trips. That is settled and `rbook` is no
//! longer the subject. F2b asks the question whose answer is needed in Phase 6
//! regardless: **which infoset and container constructs must `futhark-epub`
//! preserve?**
//!
//! It has no verdict to hang a caveat on, which is why it has two types
//! instead. Every catalogue feature resolves to one of three states (ADR-F050),
//! and the result is a [`FeatureFloor`] rather than a requirements list
//! (ADR-F051). Nothing here can produce a list of requirements; only an
//! explicit widening can, and it must say what corpus it widened from.

use std::path::Path;

use crate::archive::Archive;
use crate::catalogue;
use crate::corpus::find_epubs;
use crate::error::{F2Error, Result};
use crate::floor::FeatureFloor;
use crate::manifest::{Manifest, Record, Tier};
use crate::normalization;
use crate::producer::read_package;
use crate::taxonomy::CLASSIFIER_VERSION;

/// Walk a corpus, enumerate features against the catalogue, and emit the floor.
///
/// Returns `false` on an empty corpus. Zero books producing an all-`NotCovered`
/// floor is not a thin result about the ecosystem; it is no result, and the
/// exit status says which.
pub fn characterise(dir: &Path, tier: Tier) -> Result<bool> {
    let mut manifest = Manifest::new();
    let mut states = catalogue::initial_states();

    for path in find_epubs(dir)? {
        let source = Archive::read(&path)?;
        catalogue::accumulate(&mut states, &source);

        let pkg = read_package(&source);
        let bytes = std::fs::metadata(&path)
            .map_err(|e| F2Error::Io {
                path: path.display().to_string(),
                source: e,
            })?
            .len();
        manifest.records.push(Record {
            source_hash: Archive::file_hash(&path)?,
            source_bytes: bytes,
            tier,
            path: matches!(tier, Tier::A).then(|| path.display().to_string()),
            producer: pkg.producer,
            normalized_by: normalization::detect(&source, Some(&path)).labels(),
            epub_version: pkg.version,
            entry_count: source.entries.len(),
            // F2b writes nothing back. The question is what the population
            // contains, not what a writer does to it.
            divergences: Vec::new(),
            outcome: None,
            classifier_version: CLASSIFIER_VERSION.to_owned(),
        });
    }

    let summary = manifest.summary();
    let floor = FeatureFloor::new(states, &summary);

    print!("{}", summary.render());
    println!();
    print!("{}", floor.render());

    if floor.provenance.books > 0 && !floor.provenance.absence_is_evidence() {
        println!(
            "\nEvery `CheckedAndAbsent` above counts as unsettled, because this\n\
             corpus has not earned the right to have its silence believed: {} of\n\
             {} files were rewritten by a manager, across {} un-normalized\n\
             families with the largest at {:.0}%. A manager's writer strips the\n\
             constructs being enumerated, so absence here is a fact about the\n\
             sample first (ADR-F047, applied to enumeration).",
            floor.provenance.normalized,
            floor.provenance.books,
            floor.provenance.families.len(),
            floor.provenance.largest_family_share_pct,
        );
    }

    std::fs::write("f2b-floor.json", serde_json::to_string_pretty(&floor)?).map_err(|e| {
        F2Error::Io {
            path: "f2b-floor.json".into(),
            source: e,
        }
    })?;
    std::fs::write(
        "f2b-manifest.json",
        serde_json::to_string_pretty(&manifest)?,
    )
    .map_err(|e| F2Error::Io {
        path: "f2b-manifest.json".into(),
        source: e,
    })?;
    println!(
        "\nwrote f2b-floor.json and f2b-manifest.json ({} records)",
        manifest.records.len()
    );

    Ok(floor.provenance.books > 0)
}
