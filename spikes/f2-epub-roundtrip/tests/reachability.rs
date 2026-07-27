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
//!
//! ADR-F056 sorts those gaps by kind, because the response differs. `Tier` had
//! a third variant, `Fixture`, that nothing constructed — *dead* rather than
//! unwitnessed, and deleted rather than documented, since recording it here
//! would have preserved the claim it could not support. Nothing in this file
//! carries that kind; deletion is what its absence looks like.
//!
//! A witness is a floor on the type. It proves a variant can be produced, never
//! that it is produced for its stated reason — that is ADR-F055's job, in
//! `tests/detectors.rs`.

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

/// Why a variant has no witness (ADR-F056). There is deliberately no `Dead`
/// kind: a declared variant nothing constructs is deleted, and `Tier::Fixture`
/// was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A witness is a contradiction in terms.
    ByNature,
    /// A witness would have to be invented, and would witness the invention.
    WithoutFabrication,
    /// The fixture writer structurally cannot produce it.
    BeyondTheHarness,
    /// Witnessable; nobody has written it. The response is to write it, not to
    /// document it — this kind should shrink.
    NotYetWritten,
}

/// Divergence classes the fixture corpus cannot produce, and why.
///
/// Named rather than counted, so adding a fixture forces this list to shrink
/// and a fixture that stops working forces it to grow. A bare "11 of 17
/// covered" would move without anyone having to say what moved.
const CLASS_NO_WITNESS: &[(&str, Kind, &str)] = &[
    (
        "timestamp",
        Kind::BeyondTheHarness,
        "the fixture writer pins timestamps on purpose, so that every other \
         class is measured against a constant; the real run observes it",
    ),
    (
        "compression-level",
        Kind::BeyondTheHarness,
        "requires two writers at different deflate levels, which the in-memory \
         fixture writer has no way to vary",
    ),
    (
        "extra-field",
        Kind::NotYetWritten,
        "no fixture writes a zip extra field, and one could",
    ),
    (
        "declared-size-mismatch",
        Kind::NotYetWritten,
        "requires a container whose central directory lies about a size — a \
         hand-built malformed archive rather than a writer's output",
    ),
    (
        "manifest-mismatch",
        Kind::NotYetWritten,
        "requires an OPF manifest disagreeing with the entries present; the \
         fixture base plan keeps them consistent",
    ),
    (
        "unclassified",
        Kind::ByNature,
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
    let declared: BTreeSet<&str> = CLASS_NO_WITNESS.iter().map(|(id, _, _)| *id).collect();

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
const OUTCOME_NO_WITNESS: &[(&str, Kind, &str)] = &[
    (
        "write-failed",
        Kind::WithoutFabrication,
        "requires a book rbook parses but cannot serialise. No such input is \
         known, and one invented for the test would witness the invention",
    ),
    (
        "panicked",
        Kind::WithoutFabrication,
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

// --- ADR-F056 ------------------------------------------------------------

#[test]
fn every_declared_gap_carries_a_kind_and_a_reason() {
    for (id, _, why) in CLASS_NO_WITNESS.iter().chain(OUTCOME_NO_WITNESS) {
        assert!(
            !why.trim().is_empty(),
            "{id} is recorded as unwitnessed with no reason, which is the \
             silence it was meant to replace",
        );
    }
}

/// `NotYetWritten` is the only kind that should move. The other three describe
/// the world; this one describes a to-do, and a list that never shrinks is a
/// list nobody is reading. The assertion is on the count so that closing one
/// forces the number down rather than letting the list quietly stay the size it
/// was.
#[test]
fn the_writable_gaps_are_the_ones_still_outstanding() {
    let outstanding: Vec<&str> = CLASS_NO_WITNESS
        .iter()
        .chain(OUTCOME_NO_WITNESS)
        .filter(|(_, k, _)| *k == Kind::NotYetWritten)
        .map(|(id, _, _)| *id)
        .collect();
    assert_eq!(
        outstanding,
        ["extra-field", "declared-size-mismatch", "manifest-mismatch"],
        "write the fixture and shorten this list; do not reclassify the kind",
    );
}
