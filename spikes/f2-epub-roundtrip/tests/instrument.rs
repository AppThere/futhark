// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The instrument validating itself (ADR-F041).
//!
//! `cargo test` must fail if the classifier drifts, because a classifier that
//! is never shown a known answer is an opinion rather than an instrument. These
//! run the same fixtures the `self-test` subcommand does, so CI enforces what a
//! human would otherwise have to remember to check.

use futhark_f2::archive::Archive;
use futhark_f2::classify::classify;
use futhark_f2::fixtures;
use futhark_f2::taxonomy::{Class, Group};

/// Returns a `Result` rather than unwrapping: this helper sits outside any
/// `#[test]` function, where the crate's allow-expect-in-tests does not apply.
/// Each test unwraps in its own body.
fn read(bytes: &[u8], tag: &str) -> Result<Archive, futhark_f2::error::F2Error> {
    let mut path = std::env::temp_dir();
    path.push(format!("f2-test-{tag}.zip"));
    std::fs::write(&path, bytes).map_err(|e| futhark_f2::error::F2Error::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let a = Archive::read(&path);
    let _ = std::fs::remove_file(&path);
    a
}

#[test]
fn every_fixture_classifies_to_exactly_its_declared_set() {
    for (i, f) in fixtures::all()
        .expect("build fixtures")
        .into_iter()
        .enumerate()
    {
        let source = read(&f.source, &format!("{i}-a")).expect("read source fixture");
        let roundtrip = read(&f.roundtrip, &format!("{i}-b")).expect("read roundtrip fixture");

        let mut got: Vec<Class> = classify(&source, &roundtrip)
            .iter()
            .map(|d| d.class)
            .collect();
        got.sort_unstable();
        got.dedup();

        let mut want = f.expect.to_vec();
        want.sort_unstable();

        assert_eq!(got, want, "fixture {}", f.id);
    }
}

#[test]
fn identical_containers_produce_no_divergence() {
    let fixture = fixtures::all()
        .expect("build fixtures")
        .into_iter()
        .find(|f| f.id == "identical")
        .expect("identity fixture present");
    let a = read(&fixture.source, "id-a").expect("read source");
    let b = read(&fixture.roundtrip, "id-b").expect("read roundtrip");
    assert!(
        classify(&a, &b).is_empty(),
        "a classifier that reports divergence between a file and itself is not measuring anything",
    );
}

#[test]
fn only_entry_content_and_unclassified_bear_on_losslessness() {
    // ADR-F036: container-level divergence is reported, never fatal. ADR-F041:
    // an unnameable observation must not be scored harmless.
    for c in Class::ALL {
        let expected = matches!(c.group(), Group::EntryContent | Group::Unclassified);
        assert_eq!(c.bears_on_losslessness(), expected, "{}", c.slug());
    }
}

#[test]
fn unclassified_counts_against_the_pass_criterion() {
    assert!(
        Class::Unclassified.bears_on_losslessness(),
        "scoring an unnameable divergence as harmless is the ADR-F042 error in a different suit",
    );
}
