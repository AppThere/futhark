// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F047's asymmetry, enforced rather than documented.
//!
//! The property under test is that a clean run on a normalized corpus cannot
//! produce a result that supports adoption. If that ever becomes expressible,
//! the screening/qualifying distinction has collapsed back into a caveat, which
//! is the form it was in when it was easy to lose.

use futhark_f2::manifest::{Manifest, Record, Tier};
use futhark_f2::taxonomy::{Class, Divergence};
use futhark_f2::verdict::{PROVISIONAL_PRODUCER_FLOOR, Verdict};

fn record(producer: &str, normalized: bool, divergences: Vec<Divergence>) -> Record {
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
        divergences,
        outcome: None,
        classifier_version: "test".to_owned(),
    }
}

fn manifest_of(records: Vec<Record>) -> Manifest {
    let mut m = Manifest::new();
    m.records = records;
    m
}

#[test]
fn a_clean_run_on_a_normalized_corpus_is_not_a_pass() {
    let m = manifest_of(
        (0..20)
            .map(|i| record(&format!("calibre {i}"), true, Vec::new()))
            .collect(),
    );
    let v = Verdict::judge(&m.summary(), PROVISIONAL_PRODUCER_FLOOR);

    assert!(matches!(v, Verdict::Inconclusive { .. }), "{v:?}");
    assert!(
        !v.supports_adoption(),
        "a screening pass must never support adoption — that is the whole of ADR-F047",
    );
    assert!(
        !v.resolves_adr_f011(),
        "ADR-F011 stays open on a screening pass"
    );
}

#[test]
fn failure_is_decisive_even_on_a_normalized_corpus() {
    // One clean book, one that lost content, all normalized. The corpus is the
    // easy case and the reader still failed it.
    let mut records = vec![record("calibre", true, Vec::new())];
    records.push(record(
        "calibre",
        true,
        vec![Divergence::entry(
            Class::ContentByteDiff,
            "ch1.xhtml",
            "differs",
        )],
    ));
    let v = Verdict::judge(&manifest_of(records).summary(), PROVISIONAL_PRODUCER_FLOOR);

    assert!(matches!(v, Verdict::Disqualifying { .. }), "{v:?}");
    assert!(
        v.resolves_adr_f011(),
        "a failure on the easy population is decisive; that is the useful half of the asymmetry",
    );
}

#[test]
fn a_diverse_un_normalized_corpus_can_qualify() {
    let m = manifest_of(
        (0..8)
            .map(|i| record(&format!("publisher-{i}"), false, Vec::new()))
            .collect(),
    );
    let v = Verdict::judge(&m.summary(), PROVISIONAL_PRODUCER_FLOOR);

    assert!(matches!(v, Verdict::Qualifying { .. }), "{v:?}");
    assert!(v.supports_adoption());
}

#[test]
fn container_level_divergence_alone_still_qualifies() {
    // ADR-F036: reordered entries with every byte preserved satisfies ADR-F012.
    let m = manifest_of(
        (0..8)
            .map(|i| {
                record(
                    &format!("publisher-{i}"),
                    false,
                    vec![Divergence::archive(Class::EntryOrder, "reordered")],
                )
            })
            .collect(),
    );
    assert!(matches!(
        Verdict::judge(&m.summary(), PROVISIONAL_PRODUCER_FLOOR),
        Verdict::Qualifying { .. }
    ));
}

#[test]
fn an_empty_corpus_is_not_a_pass() {
    let v = Verdict::judge(
        &manifest_of(Vec::new()).summary(),
        PROVISIONAL_PRODUCER_FLOOR,
    );
    assert_eq!(v, Verdict::NothingMeasured);
    assert!(
        !v.supports_adoption(),
        "zero failures out of zero books is the emptiest version of the standing review question",
    );
}

#[test]
fn many_producers_but_all_normalized_still_fails_the_gate() {
    // R27 in one assertion: twenty distinct producer strings, every file
    // rewritten. The overall count looks healthy and means nothing.
    let m = manifest_of(
        (0..20)
            .map(|i| record(&format!("producer-{i}"), true, Vec::new()))
            .collect(),
    );
    let s = m.summary();
    assert_eq!(s.distinct_producers, 20, "the misleading number");
    assert_eq!(
        s.distinct_unnormalized_producers, 0,
        "the number that counts"
    );
    assert!(!Verdict::judge(&s, PROVISIONAL_PRODUCER_FLOOR).supports_adoption());
}
