// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F051's floor property, enforced rather than documented.
//!
//! The property under test is that F2b's output cannot be read as a
//! specification for `futhark-epub` without someone explicitly saying what the
//! corpus could not settle and why. If a `Requirements` value ever becomes
//! reachable without passing through `widen`, the floor has quietly become a
//! ceiling — which is the failure that already happened once in this project in
//! prose, when a §6 heading outran a §2 caveat.
//!
//! `Requirements` also deliberately does not derive `Deserialize`: a
//! `serde_json::from_str::<Requirements>` would be a back door around the
//! widening step. That property is enforced by the compiler and so has no test
//! here — there is nothing to call.

use std::collections::BTreeMap;

use futhark_f2::archive::{Archive, Entry};
use futhark_f2::catalogue::FeatureState;
use futhark_f2::floor::FeatureFloor;
use futhark_f2::manifest::{Manifest, Record, Summary, Tier};
use futhark_f2::provenance::{Disposition, WideningGap};

fn record(producer: &str, normalized: bool) -> Record {
    Record {
        source_hash: format!("hash-{producer}-{normalized}"),
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
        divergences: Vec::new(),
        outcome: None,
        classifier_version: "test".to_owned(),
    }
}

fn summary_of(records: Vec<Record>) -> Summary {
    let mut m = Manifest::new();
    m.records = records;
    m.summary()
}

/// Eight evenly-held families, nothing rewritten: the only corpus whose silence
/// is worth anything.
fn diverse_un_normalized() -> Summary {
    summary_of(
        (0..8)
            .map(|i| record(&format!("publisher-{i}"), false))
            .collect(),
    )
}

/// Twenty books, every one through a library manager.
fn normalized() -> Summary {
    summary_of(
        (0..20)
            .map(|i| record(&format!("producer-{i}"), true))
            .collect(),
    )
}

/// The states a corpus of featureless books produces — the most flattering
/// possible floor, and the one most likely to be mistaken for a short list of
/// requirements.
///
/// Built by running the real detectors rather than by asserting
/// `CheckedAndAbsent` across the board, because that state is not reachable for
/// a feature with no detector and a fixture that pretends otherwise tests a
/// floor the harness can never produce.
fn nothing_observed() -> BTreeMap<String, FeatureState> {
    let plain = Archive {
        entries: vec![Entry {
            name: "mimetype".to_owned(),
            data: b"application/epub+zip".to_vec(),
            method: "Stored".to_owned(),
            compressed_size: 20,
            modified: None,
            declared_crc: 0,
            declared_size: 20,
            comment: String::new(),
            extra: Vec::new(),
        }],
        comment: String::new(),
    };
    let mut states = futhark_f2::catalogue::initial_states();
    futhark_f2::catalogue::accumulate(&mut states, &plain);
    states
}

#[test]
fn absence_is_evidence_only_on_an_un_normalized_diverse_corpus() {
    assert!(
        FeatureFloor::new(nothing_observed(), &diverse_un_normalized())
            .provenance
            .absence_is_evidence()
    );
    assert!(
        !FeatureFloor::new(nothing_observed(), &normalized())
            .provenance
            .absence_is_evidence(),
        "a manager's writer strips exactly the constructs being enumerated, so \
         its silence is about the writer",
    );
    assert!(
        !FeatureFloor::new(nothing_observed(), &summary_of(Vec::new()))
            .provenance
            .absence_is_evidence(),
        "an empty corpus finds nothing, which is not the same as there being nothing",
    );
}

#[test]
fn on_a_normalized_corpus_checked_and_absent_is_as_unsettled_as_not_covered() {
    let floor = FeatureFloor::new(nothing_observed(), &normalized());
    assert_eq!(
        floor.unsettled().len(),
        floor.counts().catalogued,
        "every feature is unsettled: none was observed, and this corpus has not \
         earned the right to have its silence believed",
    );
    assert!(matches!(
        floor.widen(&BTreeMap::new()),
        Err(WideningGap::Unsettled { .. })
    ));
}

#[test]
fn a_floor_cannot_become_a_specification_without_widening() {
    // The clean corpus, the flattering result, and still no requirements list:
    // the four features with no detector are unsettled on any corpus at all.
    let floor = FeatureFloor::new(nothing_observed(), &diverse_un_normalized());
    let unsettled = floor.unsettled();
    assert!(
        !unsettled.is_empty(),
        "the uncovered entries are unsettled however good the corpus is",
    );
    let err = floor
        .widen(&BTreeMap::new())
        .expect_err("an undispositioned floor must not widen");
    match err {
        WideningGap::Unsettled { outstanding } => {
            assert_eq!(outstanding.len(), unsettled.len());
        }
        other => panic!("wrong gap: {other:?}"),
    }
}

#[test]
fn widening_records_the_corpus_it_widened_from() {
    let floor = FeatureFloor::new(nothing_observed(), &normalized());
    let dispositions: BTreeMap<String, Disposition> = floor
        .unsettled()
        .into_iter()
        .map(|id| {
            (
                id.to_owned(),
                Disposition::RequireAnyway {
                    because: "unsettled by a normalized corpus; preserve rather than guess"
                        .to_owned(),
                },
            )
        })
        .collect();

    let reqs = floor.widen(&dispositions).expect("fully dispositioned");
    assert_eq!(reqs.must_preserve().len(), floor.counts().catalogued);
    assert_eq!(
        reqs.provenance().normalized,
        20,
        "the corpus travels with the requirements — it is carried by the type, \
         not by a sentence someone has to remember to repeat",
    );
    assert_eq!(reqs.catalogue_version(), floor.catalogue_version);
}

#[test]
fn an_excluded_feature_keeps_its_reason() {
    let floor = FeatureFloor::new(nothing_observed(), &diverse_un_normalized());
    let dispositions: BTreeMap<String, Disposition> = floor
        .unsettled()
        .into_iter()
        .map(|id| {
            (
                id.to_owned(),
                Disposition::Excluded {
                    because: "out of scope for the first release".to_owned(),
                },
            )
        })
        .collect();
    let reqs = floor.widen(&dispositions).expect("fully dispositioned");
    assert!(reqs.must_preserve().is_empty());
    assert_eq!(reqs.excluded().len(), dispositions.len());
    assert!(reqs.excluded().iter().all(|e| !e.because.is_empty()));
}

#[test]
fn a_disposition_without_a_reason_is_rejected() {
    let floor = FeatureFloor::new(nothing_observed(), &diverse_un_normalized());
    let mut d = BTreeMap::new();
    d.insert(
        "zip64-container".to_owned(),
        Disposition::Excluded {
            because: "   ".to_owned(),
        },
    );
    assert!(
        matches!(floor.widen(&d), Err(WideningGap::EmptyReason { .. })),
        "a blank reason is the same silence wearing a decision's clothes",
    );
}

#[test]
fn a_typo_in_a_feature_id_is_an_error_rather_than_coverage() {
    let floor = FeatureFloor::new(nothing_observed(), &diverse_un_normalized());
    let mut d = BTreeMap::new();
    d.insert(
        "zip64-containers".to_owned(),
        Disposition::Excluded {
            because: "no large-file corpus".to_owned(),
        },
    );
    assert!(
        matches!(floor.widen(&d), Err(WideningGap::UnknownFeature { .. })),
        "an id nobody recognises must not be counted as a feature dispositioned",
    );
}

#[test]
fn observed_features_are_requirements_without_anyone_deciding_anything() {
    let mut states = nothing_observed();
    states.insert("xml-comment".to_owned(), FeatureState::Observed);
    let floor = FeatureFloor::new(states, &diverse_un_normalized());

    assert_eq!(floor.observed(), vec!["xml-comment"]);
    assert!(
        !floor.unsettled().contains(&"xml-comment"),
        "one book containing a comment settles it; nothing to decide",
    );
    assert_eq!(floor.counts().observed, 1);
}
