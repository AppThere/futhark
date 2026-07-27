// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The detectors behind the catalogue's first two states.
//!
//! Split from `catalogue.rs` so the list of what F2b claims to look for stays
//! readable next to the list of what it looks *at*. Everything here is a scan
//! rather than a parse: the catalogue's question is "does this corpus contain
//! any", not "where", and a parser would fail on exactly the malformed input
//! most worth counting.
//!
//! Scanning has a specific hazard, and both fixes below are instances of it: a
//! substring can match for a reason other than the construct being looked for,
//! and the result is an `Observed` produced by the wrong mechanism. `Observed`
//! is the state nothing downstream questions — it needs no disposition and goes
//! straight into `must_preserve` — so a false one is the quietest possible way
//! to be wrong here.

use std::collections::BTreeMap;

use crate::archive::Archive;

/// Features this harness can actually detect. Anything in [`CATALOGUE`] but not
/// here resolves to [`FeatureState::NotCovered`].
pub fn detect(archive: &Archive) -> BTreeMap<&'static str, bool> {
    let mut seen: BTreeMap<&'static str, bool> = BTreeMap::new();
    let mut mark = |id: &'static str, hit: bool| {
        let e = seen.entry(id).or_insert(false);
        *e = *e || hit;
    };

    let xmlish: Vec<&crate::archive::Entry> = archive
        .entries
        .iter()
        .filter(|e| {
            let n = e.name.to_ascii_lowercase();
            [".xhtml", ".xml", ".opf", ".ncx", ".html", ".svg"]
                .iter()
                .any(|x| n.ends_with(x))
        })
        .collect();

    for e in &xmlish {
        let text = String::from_utf8_lossy(&e.data);
        mark("xml-comment", text.contains("<!--"));
        mark("processing-instruction", has_pi(&text));
        mark("cdata-section", text.contains("<![CDATA["));
        mark("doctype-declaration", text.contains("<!DOCTYPE"));
        mark("internal-entity", text.contains("<!ENTITY"));
        mark("byte-order-mark", e.data.starts_with(&[0xEF, 0xBB, 0xBF]));
        mark("crlf-line-endings", e.data.windows(2).any(|w| w == b"\r\n"));
        mark("xml-standalone-declaration", text.contains("standalone="));
        mark("uncommon-namespace-prefix", uncommon_prefix(&text));
        mark(
            "non-alphabetical-attribute-order",
            attrs_out_of_order(&text),
        );
        mark("scripted-content", text.contains("<script"));
        mark(
            "remote-resource",
            text.contains("src=\"http") || text.contains("href=\"http"),
        );
    }

    for e in &archive.entries {
        mark(
            "stored-entry-beyond-mimetype",
            e.method == "Stored" && e.name != "mimetype",
        );
        mark("zip-entry-comment", !e.comment.is_empty());
        mark("zip-extra-field", !e.extra.is_empty());
        mark("non-ascii-entry-name", !e.name.is_ascii());
    }
    mark("archive-comment", !archive.comment.is_empty());

    let opf = xmlish
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".opf"));
    if let Some(opf) = opf {
        let text = String::from_utf8_lossy(&opf.data);
        mark("epub2-package", text.contains("version=\"2."));
        mark(
            "opf-guide-element",
            text.contains("<guide") || text.contains(":guide"),
        );
    }
    mark(
        "ncx-present",
        archive
            .entries
            .iter()
            .any(|e| e.name.to_ascii_lowercase().ends_with(".ncx")),
    );
    mark(
        "encryption-xml",
        archive.get("META-INF/encryption.xml").is_some(),
    );

    seen
}

fn has_pi(text: &str) -> bool {
    // `<?xml ...?>` is the declaration, not a processing instruction.
    // `<?xml-stylesheet` is a processing instruction and must still count,
    // which is why the space is part of the comparison.
    text.match_indices("<?").any(|(i, _)| {
        let head: String = text[i..].chars().take(6).collect();
        !head.eq_ignore_ascii_case("<?xml ")
    })
}

const KNOWN_PREFIXES: [&str; 8] = [
    "xmlns:dc",
    "xmlns:opf",
    "xmlns:epub",
    "xmlns:xsi",
    "xmlns:xml",
    "xmlns:m",
    "xmlns:svg",
    "xmlns:ncx",
];

/// A namespace declaration with a prefix outside the EPUB set.
///
/// The match has to open an attribute — preceded by whitespace or `<`, and
/// followed by `=`. Without both, every EPUB in existence reports one, because
/// `container.xml` declares the OCF namespace as
/// `urn:oasis:names:tc:opendocument:xmlns:container` and the literal `xmlns:`
/// inside that *URI* reads as a prefix declaration named `container">`. An
/// `Observed` for a construct no book declared, produced by a substring.
fn uncommon_prefix(text: &str) -> bool {
    text.match_indices("xmlns:").any(|(i, _)| {
        if i > 0 && !text[..i].ends_with(|c: char| c.is_whitespace() || c == '<') {
            return false;
        }
        let tail = &text[i..];
        let decl: String = tail
            .chars()
            .take_while(|c| *c != '=' && !c.is_whitespace())
            .collect();
        tail[decl.len()..].starts_with('=') && !KNOWN_PREFIXES.contains(&decl.as_str())
    })
}

/// A crude proxy: an element whose attribute names are not in ascending order.
/// Enough to answer "does this corpus contain any", which is the catalogue's
/// question, without parsing.
///
/// Bounded to the tag itself. Character data split on whitespace yields
/// "attribute names" that are essentially never alphabetical — `<p>the quick
/// brown fox` scores as out-of-order attributes — so scanning past `>` reports
/// this feature `Observed` in every book containing a sentence.
fn attrs_out_of_order(text: &str) -> bool {
    for chunk in text.split('<').take(400) {
        let tag = chunk.split('>').next().unwrap_or("");
        if !tag.starts_with(|c: char| c.is_ascii_alphabetic()) {
            continue;
        }
        let names: Vec<&str> = tag
            .split_whitespace()
            .skip(1)
            .filter_map(|t| t.split('=').next())
            .filter(|n| !n.is_empty() && n.chars().all(is_name_char))
            .collect();
        if names.windows(2).any(|w| w[0] > w[1]) {
            return true;
        }
    }
    false
}

/// Characters an XML attribute name can contain. Anything else means the token
/// came from an attribute *value*, not a name.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_' || c == '.'
}
