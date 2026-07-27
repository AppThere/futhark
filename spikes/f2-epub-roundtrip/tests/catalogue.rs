// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F050's three states, and the one that has to stay non-empty.
//!
//! The catalogue's whole value is that `CheckedAndAbsent` and `NotCovered` are
//! different claims — one is about the corpus, one is about the harness. A
//! catalogue where every entry has a detector could never demonstrate the
//! difference, and would quietly become a two-state list again the first time
//! someone read a report off it.

use futhark_f2::archive::{Archive, Entry};
use futhark_f2::catalogue::{self, CATALOGUE, FeatureState};

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

/// A minimal container with one content document, whose body is supplied.
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

/// Deliberately returns `Option` rather than defaulting a missing id to
/// `NotCovered`. Defaulting would make an id that fell out of the catalogue
/// read as an uncovered feature, which is the one confusion this whole module
/// exists to prevent.
fn state_of(
    states: &std::collections::BTreeMap<String, FeatureState>,
    id: &str,
) -> Option<FeatureState> {
    states.get(id).copied()
}

#[test]
fn observed_and_checked_and_absent_are_distinguishable() {
    let mut with = catalogue::initial_states();
    catalogue::accumulate(&mut with, &book("<!-- a comment --><html/>"));
    assert_eq!(state_of(&with, "xml-comment"), Some(FeatureState::Observed));

    let mut without = catalogue::initial_states();
    catalogue::accumulate(&mut without, &book("<html/>"));
    assert_eq!(
        state_of(&without, "xml-comment"),
        Some(FeatureState::CheckedAndAbsent),
        "a detector ran and found none — which is a claim about the corpus",
    );
}

#[test]
fn a_feature_with_no_detector_stays_not_covered_however_many_books_are_scanned() {
    let mut states = catalogue::initial_states();
    for _ in 0..50 {
        catalogue::accumulate(&mut states, &book("<!-- c --><html/>"));
    }
    assert_eq!(
        state_of(&states, "zip64-container"),
        Some(FeatureState::NotCovered),
        "fifty books cannot cover a feature nothing looks for; volume is not coverage",
    );
}

#[test]
fn the_reader_dropping_directory_entries_reads_as_not_covered_not_as_absent() {
    // `Archive::read` skips `is_dir()` entries, so a directory detector would
    // have reported `CheckedAndAbsent` on every corpus ever scanned — a clean
    // signal produced by the reader's filter rather than by the books. That is
    // the standing review question, caught inside the instrument built for it.
    let mut states = catalogue::initial_states();
    let mut a = book("<html/>");
    a.entries.push(entry("OEBPS/images/", b""));
    catalogue::accumulate(&mut states, &a);
    assert_eq!(
        state_of(&states, "directory-entry"),
        Some(FeatureState::NotCovered),
    );
}

#[test]
fn the_catalogue_has_uncovered_entries_by_construction() {
    let mut states = catalogue::initial_states();
    catalogue::accumulate(&mut states, &book("<!-- c --><?pi?><![CDATA[x]]><html/>"));
    let uncovered = CATALOGUE
        .iter()
        .filter(|f| state_of(&states, f.id) == Some(FeatureState::NotCovered))
        .count();
    assert!(
        uncovered > 0,
        "a catalogue with a detector for every entry cannot show the difference \
         between CheckedAndAbsent and NotCovered, which is why it has three states",
    );
    assert_eq!(
        states.len(),
        CATALOGUE.len(),
        "every entry resolves to a state"
    );
}

#[test]
fn observed_in_one_book_survives_every_later_book_that_lacks_it() {
    let mut states = catalogue::initial_states();
    catalogue::accumulate(&mut states, &book("<!-- once --><html/>"));
    for _ in 0..10 {
        catalogue::accumulate(&mut states, &book("<html/>"));
    }
    assert_eq!(
        state_of(&states, "xml-comment"),
        Some(FeatureState::Observed),
        "one book containing a comment proves comments must survive; ten that \
         do not prove nothing",
    );
}

#[test]
fn an_empty_corpus_reports_the_harness_rather_than_an_empty_ecosystem() {
    let states = catalogue::initial_states();
    assert!(
        states.values().all(|s| *s == FeatureState::NotCovered),
        "zero books must not report every feature as checked and absent",
    );
}

/// The OCF namespace URI ends in `xmlns:container`, so a bare substring scan
/// reports an exotic namespace prefix in every EPUB ever made — an `Observed`
/// produced by a URI rather than by a declaration. `Observed` is the state
/// nothing downstream questions, which is what makes a false one expensive.
#[test]
fn the_ocf_namespace_uri_is_not_an_exotic_prefix() {
    let mut states = catalogue::initial_states();
    let mut a = book("<html/>");
    a.entries.push(entry(
        "META-INF/container.xml",
        br#"<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/package.opf"/></rootfiles>
</container>"#,
    ));
    catalogue::accumulate(&mut states, &a);
    assert_eq!(
        state_of(&states, "uncommon-namespace-prefix"),
        Some(FeatureState::CheckedAndAbsent),
    );

    let mut declared = catalogue::initial_states();
    catalogue::accumulate(
        &mut declared,
        &book(r#"<html xmlns:calibre="http://calibre.example/"/>"#),
    );
    assert_eq!(
        state_of(&declared, "uncommon-namespace-prefix"),
        Some(FeatureState::Observed),
        "a real declaration must still be seen — the fix must not blind the detector",
    );
}

/// Prose split on whitespace produces "attribute names" that are almost never
/// alphabetical, so scanning past `>` reports this feature in every book
/// containing a sentence.
#[test]
fn character_data_is_not_attribute_ordering() {
    let mut prose = catalogue::initial_states();
    catalogue::accumulate(
        &mut prose,
        &book("<html><body><p>the quick brown fox jumps</p></body></html>"),
    );
    assert_eq!(
        state_of(&prose, "non-alphabetical-attribute-order"),
        Some(FeatureState::CheckedAndAbsent),
    );

    let mut real = catalogue::initial_states();
    catalogue::accumulate(
        &mut real,
        &book(r#"<html><img src="a.png" alt="a"/></html>"#),
    );
    assert_eq!(
        state_of(&real, "non-alphabetical-attribute-order"),
        Some(FeatureState::Observed),
        "src before alt is genuinely out of order and must still be seen",
    );
}

#[test]
fn the_xml_declaration_is_not_a_processing_instruction() {
    let mut decl = catalogue::initial_states();
    catalogue::accumulate(
        &mut decl,
        &book("<?xml version=\"1.0\" encoding=\"UTF-8\"?><html/>"),
    );
    assert_eq!(
        state_of(&decl, "processing-instruction"),
        Some(FeatureState::CheckedAndAbsent),
    );

    let mut pi = catalogue::initial_states();
    catalogue::accumulate(
        &mut pi,
        &book("<?xml version=\"1.0\"?><?xml-stylesheet href=\"a.css\"?><html/>"),
    );
    assert_eq!(
        state_of(&pi, "processing-instruction"),
        Some(FeatureState::Observed),
        "xml-stylesheet is a processing instruction and shares the declaration's prefix",
    );
}
