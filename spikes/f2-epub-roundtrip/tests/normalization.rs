// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! R27 / ADR-F046: normalization must be detectable without the producer string.
//!
//! The failure this guards against is a corpus that reports healthy producer
//! diversity while measuring one writer, because every file has been rewritten
//! by the same manager. The detector is what separates those populations, so it
//! needs both directions proven: it fires on rewritten files, and it stays
//! silent on clean ones.
//!
//! A detector that never fires and a corpus that is genuinely clean produce the
//! same output — which is the ADR-F042 problem again, and why the negative case
//! is a test rather than an assumption.

use futhark_f2::archive::Archive;
use futhark_f2::fixtures;
use futhark_f2::normalization::detect;

fn read(bytes: &[u8], tag: &str) -> Result<Archive, futhark_f2::error::F2Error> {
    let mut path = std::env::temp_dir();
    path.push(format!("f2-norm-{tag}.zip"));
    std::fs::write(&path, bytes).map_err(|e| futhark_f2::error::F2Error::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let a = Archive::read(&path);
    let _ = std::fs::remove_file(&path);
    a
}

const CALIBRE_OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Managed</dc:title>
    <meta name="calibre:timestamp" content="2019-01-01T00:00:00+00:00"/>
  </metadata>
</package>"#;

const SIGIL_OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Edited</dc:title>
    <meta name="Sigil version" content="1.9.10"/>
  </metadata>
</package>"#;

const CONTRIBUTOR_OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Converted</dc:title>
    <dc:contributor opf:role="bkp">calibre (5.44.0) [https://calibre-ebook.com]</dc:contributor>
  </metadata>
</package>"#;

/// A book *about* calibre must not be flagged for saying so in its title. The
/// detector is over-eager by design, but not arbitrary.
const INNOCENT_OPF: &str = r#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Mastering calibre: a guide</dc:title>
    <dc:creator>A. Writer</dc:creator>
  </metadata>
</package>"#;

#[test]
fn calibre_metadata_is_detected() {
    let bytes = fixtures::container_with_opf(CALIBRE_OPF).expect("build");
    let a = read(&bytes, "calibre").expect("read");
    let n = detect(&a, None);
    assert!(n.is_normalized(), "calibre:* meta must be detected");
    assert!(
        n.labels().contains(&"calibre-meta".to_owned()),
        "{:?}",
        n.labels()
    );
}

#[test]
fn sigil_metadata_is_detected() {
    let bytes = fixtures::container_with_opf(SIGIL_OPF).expect("build");
    let a = read(&bytes, "sigil").expect("read");
    assert!(detect(&a, None).labels().contains(&"sigil-meta".to_owned()));
}

#[test]
fn contributor_naming_a_manager_is_detected() {
    let bytes = fixtures::container_with_opf(CONTRIBUTOR_OPF).expect("build");
    let a = read(&bytes, "contrib").expect("read");
    let labels = detect(&a, None).labels();
    assert!(
        labels.contains(&"contributor:calibre".to_owned()),
        "{labels:?}"
    );
}

#[test]
fn a_clean_container_reports_no_normalization() {
    let bytes = fixtures::clean_container().expect("build");
    let a = read(&bytes, "clean").expect("read");
    let n = detect(&a, None);
    assert!(
        !n.is_normalized(),
        "a detector that fires on everything cannot separate the populations: {:?}",
        n.labels(),
    );
}

#[test]
fn a_book_about_calibre_is_not_flagged() {
    let bytes = fixtures::container_with_opf(INNOCENT_OPF).expect("build");
    let a = read(&bytes, "innocent").expect("read");
    let n = detect(&a, None);
    assert!(
        !n.is_normalized(),
        "title text must not count as normalization evidence: {:?}",
        n.labels(),
    );
}
