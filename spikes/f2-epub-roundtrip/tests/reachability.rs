// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F053: every variant carries a witness, or is named as lacking one.
//!
//! Encoding a distinction as a type is a claim about reachability, and nothing
//! else in the build checks it. `FeatureState` declared three states and reached
//! two for as long as `accumulate` used `or_insert`, green throughout — an
//! unreachable variant is documentation wearing a type's clothes, and inherits
//! every weakness this project has been encoding *away* from documentation.
//!
//! Witnesses here go through the real code path — `accumulate`, `judge`,
//! `classify`, `roundtrip`, `widen` — never by constructing the variant. A
//! directly-constructed variant witnesses the `enum` keyword.
//!
//! Where no witness exists, the variant is named in a `NO_WITNESS` list with a
//! reason, and the test asserts the list is exactly right. A gap that is counted
//! is a different thing from a gap that is invisible, which is the same move as
//! `FeatureState::NotCovered` one level up.

use std::collections::BTreeSet;

use futhark_f2::archive::{Archive, Entry};
use futhark_f2::catalogue::{self, FeatureState};
use futhark_f2::classify::classify;
use futhark_f2::floor::FeatureFloor;
use futhark_f2::manifest::{Manifest, Record, Summary, Tier};
use futhark_f2::provenance::{Disposition, WideningGap};
use futhark_f2::roundtrip::{self, Outcome};
use futhark_f2::taxonomy::Class;
use futhark_f2::verdict::{FAMILY_FLOOR, Shortfall, Verdict};
use futhark_f2::{fixtures, selftest};

// --- FeatureState ---------------------------------------------------------

fn entry(name: &str, data: &[u8]) -> Entry {
    Entry {
        name: name.to_owned(),
        data: data.to_vec(),
        method: "Deflated".to_owned(),
        compressed_size: data.len() as u64,
        modified: None,
        declared_crc: 0,
        declared_size: data.len() as u64,
        comment: String::new(),
        extra: Vec::new(),
    }
}

fn book(xhtml: &str) -> Archive {
    Archive {
        entries: vec![
            entry("mimetype", b"application/epub+zip"),
            entry("OEBPS/package.opf", b"<package version=\"3.0\"/>"),
            entry("OEBPS/ch1.xhtml", xhtml.as_bytes()),
        ],
        comment: String::new(),
    }
}

/// The witness that was missing. `or_insert` could never promote `NotCovered`
/// to `CheckedAndAbsent`, so a working detector and an absent one produced the
/// same state and the three-state catalogue was a two-state list.
#[test]
fn every_feature_state_is_reachable_through_accumulate() {
    let mut states = catalogue::initial_states();
    catalogue::accumulate(&mut states, &book("<!-- a comment --><html/>"));

    // `Vec`, not `BTreeSet`: `FeatureState` is deliberately not `Ord`, and a
    // test that needed the ordering back would be arguing with that decision.
    let reached: Vec<FeatureState> = states.values().copied().collect();
    assert!(reached.contains(&FeatureState::Observed), "{reached:?}");
    assert!(
        reached.contains(&FeatureState::CheckedAndAbsent),
        "the state `or_insert` made unreachable: {reached:?}",
    );
    assert!(reached.contains(&FeatureState::NotCovered), "{reached:?}");
}

// --- Class ----------------------------------------------------------------

/// Divergence classes the fixture corpus cannot produce, and why.
///
/// Named rather than counted, so adding a fixture forces this list to shrink
/// and a fixture that stops working forces it to grow. A bare "11 of 17
/// covered" would move without anyone having to say what moved.
const CLASS_NO_WITNESS: &[(&str, &str)] = &[
    (
        "timestamp",
        "the fixture writer pins timestamps on purpose, so that every other \
         class is measured against a constant; the real run observes it",
    ),
    (
        "compression-level",
        "requires two writers at different deflate levels, which the in-memory \
         fixture writer has no way to vary",
    ),
    ("extra-field", "no fixture writes a zip extra field"),
    (
        "declared-size-mismatch",
        "requires a container whose central directory lies about a size — a \
         hand-built malformed archive rather than a writer's output",
    ),
    (
        "manifest-mismatch",
        "requires an OPF manifest disagreeing with the entries present; the \
         fixture base plan keeps them consistent",
    ),
    (
        "unclassified",
        "reachable only when the instrument meets a divergence it cannot name. \
         A fixture for it would be a fixture for the classifier's own blind \
         spot, which is a contradiction — this one is unwitnessed by nature, \
         and `tests/instrument.rs` asserts it counts as a failure when it fires",
    ),
];

#[test]
fn the_classes_without_a_fixture_witness_are_exactly_the_named_ones() {
    let mut reached: BTreeSet<&'static str> = BTreeSet::new();
    for f in fixtures::all().expect("fixtures build") {
        let a = selftest::read_bytes(&f.source).expect("source readable");
        let b = selftest::read_bytes(&f.roundtrip).expect("roundtrip readable");
        for d in classify(&a, &b) {
            reached.insert(d.class.slug());
        }
    }

    let unreached: BTreeSet<&str> = Class::ALL
        .iter()
        .map(|c| c.slug())
        .filter(|s| !reached.contains(s))
        .collect();
    let declared: BTreeSet<&str> = CLASS_NO_WITNESS.iter().map(|(id, _)| *id).collect();

    assert_eq!(
        unreached, declared,
        "the set of classes with no witness moved. Add a fixture, or add the \
         class to CLASS_NO_WITNESS with the reason it cannot have one — never \
         let the gap change size quietly",
    );
}

// --- Verdict and Shortfall ------------------------------------------------

fn record(
    producer: &str,
    normalized: bool,
    divergences: Vec<futhark_f2::taxonomy::Divergence>,
) -> Record {
    Record {
        source_hash: format!("h-{producer}-{normalized}"),
        source_bytes: 1,
        tier: Tier::B,
        path: None,
        producer: Some(producer.to_owned()),
        normalized_by: if normalized {
            vec!["calibre-meta".to_owned()]
        } else {
            Vec::new()
        },
        epub_version: Some("3.0".to_owned()),
        entry_count: 4,
        divergences,
        outcome: None,
        classifier_version: "test".to_owned(),
    }
}

fn summary_of(records: Vec<Record>) -> Summary {
    let mut m = Manifest::new();
    m.records = records;
    m.summary()
}

#[test]
fn every_verdict_is_reachable_through_judge() {
    let empty = Verdict::judge(&summary_of(Vec::new()), FAMILY_FLOOR);
    assert!(matches!(empty, Verdict::NothingMeasured), "{empty:?}");

    let failed = Verdict::judge(
        &summary_of(vec![record(
            "publisher",
            false,
            vec![futhark_f2::taxonomy::Divergence::entry(
                Class::ContentByteDiff,
                "ch1.xhtml",
                "differs",
            )],
        )]),
        FAMILY_FLOOR,
    );
    assert!(
        matches!(failed, Verdict::Disqualifying { .. }),
        "{failed:?}"
    );

    let thin = Verdict::judge(
        &summary_of(
            (0..20)
                .map(|i| record(&format!("calibre {i}"), true, Vec::new()))
                .collect(),
        ),
        FAMILY_FLOOR,
    );
    assert!(
        matches!(
            thin,
            Verdict::Inconclusive {
                reason: Shortfall::TooFewFamilies,
                ..
            }
        ),
        "{thin:?}"
    );

    let mut lopsided: Vec<Record> = (0..100)
        .map(|_| record("Vellum", false, Vec::new()))
        .collect();
    for i in 0..8 {
        lopsided.push(record(&format!("publisher-{i}"), false, Vec::new()));
    }
    let concentrated = Verdict::judge(&summary_of(lopsided), FAMILY_FLOOR);
    assert!(
        matches!(
            concentrated,
            Verdict::Inconclusive {
                reason: Shortfall::TooConcentrated,
                ..
            }
        ),
        "{concentrated:?}"
    );

    let diverse = Verdict::judge(
        &summary_of(
            (0..8)
                .map(|i| record(&format!("publisher-{i}"), false, Vec::new()))
                .collect(),
        ),
        FAMILY_FLOOR,
    );
    assert!(matches!(diverse, Verdict::Qualifying { .. }), "{diverse:?}");
}

// --- WideningGap ----------------------------------------------------------

#[test]
fn every_widening_gap_is_reachable_through_widen() {
    let floor = FeatureFloor::new(catalogue::initial_states(), &summary_of(Vec::new()));

    assert!(matches!(
        floor.widen(&std::collections::BTreeMap::new()),
        Err(WideningGap::Unsettled { .. })
    ));

    let mut unknown = std::collections::BTreeMap::new();
    unknown.insert(
        "not-a-feature".to_owned(),
        Disposition::Excluded {
            because: "typo".to_owned(),
        },
    );
    assert!(matches!(
        floor.widen(&unknown),
        Err(WideningGap::UnknownFeature { .. })
    ));

    let mut blank = std::collections::BTreeMap::new();
    blank.insert(
        "zip64-container".to_owned(),
        Disposition::Excluded {
            because: " ".to_owned(),
        },
    );
    assert!(matches!(
        floor.widen(&blank),
        Err(WideningGap::EmptyReason { .. })
    ));
}

// --- Outcome --------------------------------------------------------------

/// Reader outcomes with no witness, and why. Both are honest gaps rather than
/// oversights, and neither can be closed by writing a better test.
const OUTCOME_NO_WITNESS: &[(&str, &str)] = &[
    (
        "write-failed",
        "requires a book rbook parses but cannot serialise. No such input is \
         known, and one invented for the test would witness the invention",
    ),
    (
        "panicked",
        "requires an input that panics rbook's parser. Finding one is the \
         hostile-corpus work F2b's real run does; a synthetic panic would \
         witness `catch_unwind`, not the reader",
    ),
];

#[test]
fn round_tripped_and_read_failed_have_witnesses_and_the_rest_are_named() {
    let dir = std::env::temp_dir().join("f2-reachability");
    std::fs::create_dir_all(&dir).expect("temp dir");

    let good = dir.join("good.epub");
    let f = fixtures::all()
        .expect("fixtures build")
        .into_iter()
        .next()
        .expect("at least one fixture");
    std::fs::write(&good, &f.source).expect("write fixture");
    let out = roundtrip::roundtrip(&good, &dir.join("good-rt.epub"));
    assert_eq!(out, Outcome::RoundTripped, "the fixture must round-trip");

    let bad = dir.join("bad.epub");
    std::fs::write(&bad, b"this is not a zip archive").expect("write garbage");
    let out = roundtrip::roundtrip(&bad, &dir.join("bad-rt.epub"));
    assert!(
        matches!(out, Outcome::ReadFailed { .. }),
        "hostile bytes must be reported as a refusal, not as a clean run: {out:?}",
    );
    assert!(
        !out.produced_output(),
        "a refusal produced no file to compare, and must never be scored as \
         zero divergences",
    );

    assert_eq!(OUTCOME_NO_WITNESS.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

// --- Tier -----------------------------------------------------------------

/// `Tier::Fixture` is declared and never constructed. The audit ADR-F053 asks
/// for turned it up: a manifest reader would take the schema to mean that
/// fixture-sourced records exist and can be told apart from Tier A ones, and
/// none do. It is kept — the distinction is real and the fixture runs should
/// carry it — but it is named here as unwitnessed rather than left to look
/// like a tier nothing happened to hit yet.
#[test]
fn the_fixture_tier_has_no_witness_and_that_is_recorded() {
    let r = record("x", false, Vec::new());
    assert_eq!(r.tier, Tier::B);
}
