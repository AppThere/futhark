// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F055: every detector carries a paired true negative.
//!
//! A true positive alone says the detector fires on the construct. It does not
//! say the detector fires *only* on it, and both false-`Observed` bugs in this
//! crate reached the right variant for the wrong reason: `xmlns:` matched the
//! mandatory OCF namespace URI, and prose split on whitespace scored as
//! non-alphabetical attribute order. ADR-F053's reachability witness cannot see
//! either — the variant was reachable, just not for its stated reason.
//!
//! So the assertion here is differential, in the same shape
//! `scripts/check-self-test` uses on the repository checks: pass clean, fail
//! dirty. Each case is [`base`] plus exactly one construct, and the test
//! requires that
//!
//! > the set of features `Observed` in the case, minus those `Observed` in the
//! > base, is **exactly** the one feature under test.
//!
//! That is stricter than "the detector fired". A detector that also fires on
//! the base contributes nothing to the difference and fails; a construct that
//! trips a second detector shows up as an extra element and fails. Both bugs
//! fail this test in the first form.
//!
//! The base itself is the control. If it observes anything, every difference
//! computed against it is unreliable, so that is asserted before any case runs.

use std::collections::BTreeSet;

use futhark_f2::archive::{Archive, Entry};
use futhark_f2::catalogue::{self, FeatureState};
use futhark_f2::detect::detect;

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

fn stored(name: &str, data: &[u8]) -> Entry {
    Entry {
        method: "Stored".to_owned(),
        ..entry(name, data)
    }
}

const CONTAINER: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/package.opf"/></rootfiles>
</container>
"#;

const OPF: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<package version="3.0">
  <metadata><dc:title>Base</dc:title></metadata>
  <manifest><item href="ch1.xhtml" id="ch1"/></manifest>
</package>
"#;

const CH1: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body><p>the quick brown fox jumps over the lazy dog</p></body>
</html>
"#;

/// A container carrying none of the catalogued constructs.
///
/// Deliberately realistic rather than minimal: it has the mandatory OCF
/// namespace declaration whose URI ends in `xmlns:container`, an XML
/// declaration, a `mimetype` entry stored rather than deflated, and a sentence
/// of prose. Each of those is something a detector has already mistaken for a
/// construct, and keeping them in the base means a regression turns the
/// difference set empty rather than going unnoticed.
fn base() -> Archive {
    Archive {
        entries: vec![
            stored("mimetype", b"application/epub+zip"),
            entry("META-INF/container.xml", CONTAINER),
            entry("OEBPS/package.opf", OPF),
            entry("OEBPS/ch1.xhtml", CH1),
        ],
        comment: String::new(),
    }
}

/// Replace one entry's bytes, keeping everything else identical.
fn with_data(name: &str, data: Vec<u8>) -> Archive {
    let mut a = base();
    for e in &mut a.entries {
        if e.name == name {
            e.data = data.clone();
        }
    }
    a
}

fn ch1_with(extra: &str) -> Archive {
    let mut body = String::from_utf8_lossy(CH1).into_owned();
    body.push_str(extra);
    with_data("OEBPS/ch1.xhtml", body.into_bytes())
}

fn with_entry(e: Entry) -> Archive {
    let mut a = base();
    a.entries.push(e);
    a
}

/// One case: the feature under test, the container *without* the construct, and
/// the same container *with* it.
///
/// The negative is [`base`] for all but one case. `internal-entity` needs a
/// `DOCTYPE` to declare an entity inside, so its negative is a container with an
/// empty `DOCTYPE` — otherwise the pair differs in two constructs and the
/// difference set says so. That is the discipline working rather than an
/// exception to it: a pair that is not minimal cannot show a detector is
/// specific.
struct Case {
    id: &'static str,
    negative: Archive,
    positive: Archive,
}

fn case(id: &'static str, positive: Archive) -> Case {
    Case {
        id,
        negative: base(),
        positive,
    }
}

fn cases() -> Vec<Case> {
    let mut commented = base();
    commented.comment = "archive comment".to_owned();

    let mut bom = vec![0xEF, 0xBB, 0xBF];
    bom.extend_from_slice(CH1);

    vec![
        case("xml-comment", ch1_with("<!-- a comment -->")),
        case(
            "processing-instruction",
            ch1_with("<?xml-stylesheet href=\"a.css\"?>"),
        ),
        case("cdata-section", ch1_with("<![CDATA[raw]]>")),
        case("doctype-declaration", ch1_with("<!DOCTYPE html []>")),
        // Paired against a DOCTYPE that declares nothing, so the difference is
        // the entity rather than the doctype that has to carry it.
        Case {
            id: "internal-entity",
            negative: ch1_with("<!DOCTYPE html []>"),
            positive: ch1_with("<!DOCTYPE html [<!ENTITY a \"b\">]>"),
        },
        // Replaces the declaration rather than appending one: an appended
        // `<?... standalone=?>` is also a processing instruction, and the pair
        // would then differ in two constructs.
        case(
            "xml-standalone-declaration",
            with_data(
                "OEBPS/ch1.xhtml",
                String::from_utf8_lossy(CH1)
                    .replace(
                        "encoding=\"UTF-8\"?>",
                        "encoding=\"UTF-8\" standalone=\"yes\"?>",
                    )
                    .into(),
            ),
        ),
        case(
            "uncommon-namespace-prefix",
            ch1_with("<x xmlns:zz=\"http://example.invalid/\"/>"),
        ),
        case(
            "non-alphabetical-attribute-order",
            ch1_with("<img src=\"a.png\" alt=\"a\"/>"),
        ),
        case("scripted-content", ch1_with("<script>void 0;</script>")),
        case(
            "remote-resource",
            ch1_with("<img src=\"https://example.invalid/a.png\"/>"),
        ),
        case("byte-order-mark", with_data("OEBPS/ch1.xhtml", bom)),
        case(
            "crlf-line-endings",
            with_data(
                "OEBPS/ch1.xhtml",
                String::from_utf8_lossy(CH1).replace('\n', "\r\n").into(),
            ),
        ),
        case(
            "stored-entry-beyond-mimetype",
            with_entry(stored("OEBPS/raw.bin", b"raw")),
        ),
        case(
            "zip-entry-comment",
            with_entry(Entry {
                comment: "a comment".to_owned(),
                ..entry("OEBPS/extra.bin", b"x")
            }),
        ),
        case(
            "zip-extra-field",
            with_entry(Entry {
                extra: vec![0x55, 0x54, 0x05, 0x00],
                ..entry("OEBPS/extra.bin", b"x")
            }),
        ),
        case(
            "non-ascii-entry-name",
            with_entry(entry("OEBPS/café.bin", b"x")),
        ),
        case("archive-comment", commented),
        case(
            "epub2-package",
            with_data(
                "OEBPS/package.opf",
                String::from_utf8_lossy(OPF)
                    .replace("version=\"3.0\"", "version=\"2.0\"")
                    .into(),
            ),
        ),
        case(
            "opf-guide-element",
            with_data(
                "OEBPS/package.opf",
                String::from_utf8_lossy(OPF)
                    .replace("</package>", "<guide/></package>")
                    .into(),
            ),
        ),
        case("ncx-present", with_entry(entry("OEBPS/toc.ncx", b"<ncx/>"))),
        case(
            "encryption-xml",
            with_entry(entry("META-INF/encryption.xml", b"<encryption/>")),
        ),
    ]
}

fn observed(archive: &Archive) -> BTreeSet<&'static str> {
    let mut states = catalogue::initial_states();
    catalogue::accumulate(&mut states, archive);
    states
        .iter()
        .filter(|(_, s)| **s == FeatureState::Observed)
        .filter_map(|(id, _)| {
            catalogue::CATALOGUE
                .iter()
                .find(|f| f.id == id.as_str())
                .map(|f| f.id)
        })
        .collect()
}

/// The control, run before any difference is computed. A base that already
/// observes something makes every difference against it meaningless — the same
/// reason `check-self-test` runs each check on the unperturbed tree first.
#[test]
fn the_base_container_observes_nothing() {
    let seen = observed(&base());
    assert!(
        seen.is_empty(),
        "the base is not clean, so no difference computed against it means \
         anything: {seen:?}",
    );
}

#[test]
fn each_detector_fires_on_its_construct_and_only_on_its_construct() {
    // Every case is reported, not just the first. A run that stops at the first
    // failure hides how far the problem spreads, which matters when the cause
    // is one detector everything else is measured against.
    let mut wrong: Vec<String> = Vec::new();
    for c in cases() {
        let before = observed(&c.negative);
        let after = observed(&c.positive);
        let added: BTreeSet<&str> = after.difference(&before).copied().collect();
        let want: BTreeSet<&str> = [c.id].into_iter().collect();
        if added != want {
            wrong.push(format!("  {:<34} added {added:?}", c.id));
        }
    }
    assert!(
        wrong.is_empty(),
        "each case must add exactly its own feature. An empty difference means \
         the detector already fired on the negative — the false-`Observed` \
         shape both bugs had. An extra element means the construct trips a \
         second detector and the pair is not minimal.\n{}",
        wrong.join("\n"),
    );
}

/// ADR-F055 says *every* detector, so the case list is checked against the
/// detectors rather than against a count someone maintains by hand.
#[test]
fn every_detector_has_a_case() {
    let covered: BTreeSet<&str> = detect(&base()).keys().copied().collect();
    let tested: BTreeSet<&str> = cases().into_iter().map(|c| c.id).collect();
    assert_eq!(
        tested, covered,
        "a detector without a paired true negative is a detector nothing has \
         shown to be specific (ADR-F055)",
    );
}
