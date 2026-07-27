// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The classifier: two archives in, classified divergences out.
//!
//! It never counts. Counting is the caller's job, and keeping the two apart is
//! what ADR-F036 asks for — a percentage computed before classification would
//! be measuring the writer's zip library rather than its fidelity.
//!
//! Narrower classes are tried before wider ones, so `ContentByteDiff` means
//! "differs, and none of the specific explanations fit" rather than "differs".

use crate::archive::{Archive, Entry};
use crate::taxonomy::{Class, Divergence};
use crate::xmlcmp;

/// Compare a source container against its round-trip.
pub fn classify(source: &Archive, roundtrip: &Archive) -> Vec<Divergence> {
    let mut out = Vec::new();
    compare_membership(source, roundtrip, &mut out);
    compare_common_entries(source, roundtrip, &mut out);
    compare_order(source, roundtrip, &mut out);
    compare_archive_metadata(source, roundtrip, &mut out);
    check_ocf_invariants(roundtrip, &mut out);
    out.sort_by(|a, b| (a.class, &a.entry).cmp(&(b.class, &b.entry)));
    out
}

fn compare_membership(source: &Archive, roundtrip: &Archive, out: &mut Vec<Divergence>) {
    for e in &source.entries {
        if roundtrip.get(&e.name).is_none() {
            out.push(Divergence::entry(
                Class::EntryMissing,
                &e.name,
                format!("{} bytes in source, absent from round-trip", e.data.len()),
            ));
        }
    }
    for e in &roundtrip.entries {
        if source.get(&e.name).is_none() {
            out.push(Divergence::entry(
                Class::EntryAdded,
                &e.name,
                format!("{} bytes, not present in source", e.data.len()),
            ));
        }
    }
}

fn compare_common_entries(source: &Archive, roundtrip: &Archive, out: &mut Vec<Divergence>) {
    for a in &source.entries {
        let Some(b) = roundtrip.get(&a.name) else {
            continue;
        };
        if a.data == b.data {
            compare_storage(a, b, out);
        } else {
            out.push(content_divergence(a, b));
        }
    }
}

/// Identical bytes: anything left is how the bytes were stored, which is group B.
fn compare_storage(a: &Entry, b: &Entry, out: &mut Vec<Divergence>) {
    if a.method != b.method {
        out.push(Divergence::entry(
            Class::CompressionMethod,
            &a.name,
            format!("{} -> {}", a.method, b.method),
        ));
    } else if a.compressed_size != b.compressed_size {
        // Same method, same input, different output size: a level difference.
        out.push(Divergence::entry(
            Class::CompressionLevel,
            &a.name,
            format!(
                "compressed {} -> {} bytes, content identical",
                a.compressed_size, b.compressed_size
            ),
        ));
    }
    if a.modified != b.modified {
        out.push(Divergence::entry(
            Class::Timestamp,
            &a.name,
            format!("{:?} -> {:?}", a.modified, b.modified),
        ));
    }
    if a.extra != b.extra {
        out.push(Divergence::entry(
            Class::ExtraField,
            &a.name,
            format!(
                "{} -> {} bytes of extra field",
                a.extra.len(),
                b.extra.len()
            ),
        ));
    }
    if a.comment != b.comment {
        out.push(Divergence::entry(
            Class::Comment,
            &a.name,
            "entry comment differs",
        ));
    }
}

/// Bytes differ. Find the narrowest explanation that fits.
fn content_divergence(a: &Entry, b: &Entry) -> Divergence {
    let detail = format!("{} -> {} bytes", a.data.len(), b.data.len());

    if strip_bom(&a.data) == strip_bom(&b.data) {
        return Divergence::entry(
            Class::TextEncoding,
            &a.name,
            format!("byte-order mark differs; {detail}"),
        );
    }
    if normalise_eol(&a.data) == normalise_eol(&b.data) {
        return Divergence::entry(
            Class::LineEndings,
            &a.name,
            format!("line endings differ; {detail}"),
        );
    }
    if looks_like_xml(&a.name) && xmlcmp::infoset_equal(&a.data, &b.data) {
        return Divergence::entry(
            Class::XmlCanonicalization,
            &a.name,
            format!("XML infoset preserved, serialisation differs; {detail}"),
        );
    }
    Divergence::entry(Class::ContentByteDiff, &a.name, detail)
}

fn compare_order(source: &Archive, roundtrip: &Archive, out: &mut Vec<Divergence>) {
    let (a, b) = (source.order(), roundtrip.order());
    // Only meaningful when membership matches; otherwise the membership classes
    // already carry the finding and an order report would double-count it.
    let mut sa = a.clone();
    let mut sb = b.clone();
    sa.sort_unstable();
    sb.sort_unstable();
    if sa == sb && a != b {
        out.push(Divergence::archive(
            Class::EntryOrder,
            format!(
                "same {} entries, different central-directory order",
                a.len()
            ),
        ));
    }
}

fn compare_archive_metadata(source: &Archive, roundtrip: &Archive, out: &mut Vec<Divergence>) {
    if source.comment != roundtrip.comment {
        out.push(Divergence::archive(
            Class::Comment,
            "archive comment differs",
        ));
    }
}

/// OCF's structural requirements, checked against the round-trip alone. These
/// are claims the package makes about itself, so the source's own compliance is
/// beside the point — a writer that breaks them has produced an invalid file.
fn check_ocf_invariants(roundtrip: &Archive, out: &mut Vec<Divergence>) {
    match roundtrip.entries.first() {
        Some(first) if first.name == "mimetype" => {
            if first.method != "Stored" {
                out.push(Divergence::entry(
                    Class::MimetypeCompressed,
                    "mimetype",
                    format!("stored as {}; OCF requires uncompressed", first.method),
                ));
            }
        }
        Some(first) => out.push(Divergence::entry(
            Class::MimetypeNotFirst,
            &first.name,
            format!("first entry is {:?}; OCF requires mimetype", first.name),
        )),
        None => {}
    }

    for e in &roundtrip.entries {
        if e.declared_size != e.data.len() as u64 {
            out.push(Divergence::entry(
                Class::DeclaredSizeMismatch,
                &e.name,
                format!(
                    "header declares {} bytes, entry holds {}",
                    e.declared_size,
                    e.data.len()
                ),
            ));
        }
    }
}

fn strip_bom(data: &[u8]) -> &[u8] {
    data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data)
}

fn normalise_eol(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        match data.get(i) {
            Some(&b'\r') => {
                out.push(b'\n');
                if data.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            Some(&b) => out.push(b),
            None => break,
        }
        i += 1;
    }
    out
}

fn looks_like_xml(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".xhtml", ".xml", ".opf", ".ncx", ".html", ".svg"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}
