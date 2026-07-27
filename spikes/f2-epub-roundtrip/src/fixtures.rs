// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Fixtures that encode one divergence class each, by construction.
//!
//! This is how the instrument is validated (ADR-F041, D13). Each fixture is a
//! pair — a base container and a variant that differs in exactly one known way —
//! so the expected class is known *a priori* rather than inferred from the data.
//! Validating against Tier A instead would encode one toolchain's habits as the
//! definition of normal; validating against Tier B would fit the instrument to
//! the corpus it is supposed to be measuring.
//!
//! Every variant holds constant everything it can. Some perturbations are
//! irreducibly multi-class — `mimetype` cannot stop being the first entry
//! without the entry order changing, and cannot be compressed without the
//! compression method changing — so a fixture declares the *complete* expected
//! set and the test asserts set equality. That still catches both failure
//! directions: a missing class is under-classification, an extra one is
//! over-classification.

use std::io::{Cursor, Write};

use zip::write::{FileOptions, SimpleFileOptions};
use zip::{CompressionMethod, ZipWriter};

use crate::error::Result;
use crate::taxonomy::Class;

/// A fixture: two containers and the single class that should separate them.
pub struct Fixture {
    /// Stable identifier, used in test output.
    pub id: &'static str,
    /// The source container.
    pub source: Vec<u8>,
    /// The round-tripped container.
    pub roundtrip: Vec<u8>,
    /// The classes the classifier must report — exactly these, no more and no
    /// fewer. Usually one; more where the perturbation cannot be isolated.
    pub expect: &'static [Class],
}

const OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="id">urn:uuid:f2-fixture</dc:identifier>
    <dc:title>Fixture</dc:title>
    <dc:language>en</dc:language>
    <meta name="generator" content="futhark-f2-fixtures"/>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx"><itemref idref="c1"/></spine>
</package>
"#;

// A nav document and an NCX are not optional decoration. Without them the
// container is not a valid EPUB 3, and a reader that *repairs* invalid input by
// synthesising the missing pieces looks identical to one that corrupts valid
// input — both show up as `entry-added`. The first round-trip run against rbook
// produced exactly that confound, which is why they are here.
const NAV: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><title>Contents</title></head>
<body><nav epub:type="toc" id="toc"><ol><li><a href="ch1.xhtml">One</a></li></ol></nav></body>
</html>
"#;

const NCX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
<head><meta name="dtb:uid" content="urn:uuid:f2-fixture"/></head>
<docTitle><text>Fixture</text></docTitle>
<navMap><navPoint id="np1" playOrder="1">
  <navLabel><text>One</text></navLabel><content src="ch1.xhtml"/>
</navPoint></navMap>
</ncx>
"#;

const CHAPTER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>One</title></head>\n\
<body><p id=\"a\" class=\"x\">Hello.</p></body></html>\n";

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/package.opf"
    media-type="application/oebps-package+xml"/></rootfiles>
</container>
"#;

/// One entry as it will be written.
struct Plan {
    name: &'static str,
    body: Vec<u8>,
    stored: bool,
}

fn base_plan() -> Vec<Plan> {
    vec![
        Plan {
            name: "mimetype",
            body: b"application/epub+zip".to_vec(),
            stored: true,
        },
        Plan {
            name: "META-INF/container.xml",
            body: CONTAINER.as_bytes().to_vec(),
            stored: false,
        },
        Plan {
            name: "OEBPS/package.opf",
            body: OPF.as_bytes().to_vec(),
            stored: false,
        },
        Plan {
            name: "OEBPS/nav.xhtml",
            body: NAV.as_bytes().to_vec(),
            stored: false,
        },
        Plan {
            name: "OEBPS/toc.ncx",
            body: NCX.as_bytes().to_vec(),
            stored: false,
        },
        Plan {
            name: "OEBPS/ch1.xhtml",
            body: CHAPTER.as_bytes().to_vec(),
            stored: false,
        },
    ]
}

/// Fixed timestamp everywhere, so `Timestamp` only fires when a fixture asks it to.
fn opts(stored: bool) -> SimpleFileOptions {
    let method = if stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    FileOptions::default()
        .compression_method(method)
        .last_modified_time(zip::DateTime::default())
}

fn build(plans: &[Plan], comment: Option<&str>) -> Result<Vec<u8>> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    if let Some(c) = comment {
        zw.set_comment(c)?;
    }
    for p in plans {
        zw.start_file(p.name, opts(p.stored))?;
        zw.write_all(&p.body)
            .map_err(|e| crate::error::F2Error::Io {
                path: p.name.to_owned(),
                source: e,
            })?;
    }
    Ok(zw.finish()?.into_inner())
}

fn mutate(mut plans: Vec<Plan>, name: &str, f: impl Fn(&mut Plan)) -> Vec<Plan> {
    if let Some(p) = plans.iter_mut().find(|p| p.name == name) {
        f(p);
    }
    plans
}

/// Build a container whose OPF is the given text, for exercising detectors that
/// read the package document (normalization, producer extraction).
pub fn container_with_opf(opf: &str) -> Result<Vec<u8>> {
    let plans = mutate(base_plan(), "OEBPS/package.opf", |p| {
        p.body = opf.as_bytes().to_vec();
    });
    build(&plans, None)
}

/// The base container, unmodified: a file no manager has touched.
pub fn clean_container() -> Result<Vec<u8>> {
    build(&base_plan(), None)
}

/// Every fixture. Each asserts exactly one class.
pub fn all() -> Result<Vec<Fixture>> {
    let base = build(&base_plan(), None)?;
    let mut out = Vec::new();

    // --- A: entry content -------------------------------------------------
    out.push(Fixture {
        id: "content-byte-diff",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER.replace("Hello.", "Hello!").into_bytes();
            }),
            None,
        )?,
        expect: &[Class::ContentByteDiff],
    });

    out.push(Fixture {
        id: "entry-missing",
        source: base.clone(),
        roundtrip: build(
            &base_plan()
                .into_iter()
                .filter(|p| p.name != "OEBPS/ch1.xhtml")
                .collect::<Vec<_>>(),
            None,
        )?,
        expect: &[Class::EntryMissing],
    });

    let mut added = base_plan();
    added.push(Plan {
        name: "OEBPS/extra.txt",
        body: b"surplus".to_vec(),
        stored: false,
    });
    out.push(Fixture {
        id: "entry-added",
        source: base.clone(),
        roundtrip: build(&added, None)?,
        expect: &[Class::EntryAdded],
    });

    // Attribute order reversed and the empty element expanded: same infoset,
    // different bytes.
    out.push(Fixture {
        id: "xml-canonicalization",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER
                    .replace("id=\"a\" class=\"x\"", "class=\"x\" id=\"a\"")
                    .into_bytes();
            }),
            None,
        )?,
        expect: &[Class::XmlCanonicalization],
    });

    out.push(Fixture {
        id: "text-encoding",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                let mut b = vec![0xEF, 0xBB, 0xBF];
                b.extend_from_slice(CHAPTER.as_bytes());
                p.body = b;
            }),
            None,
        )?,
        expect: &[Class::TextEncoding],
    });

    out.push(Fixture {
        id: "line-endings",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| {
                p.body = CHAPTER.replace('\n', "\r\n").into_bytes();
            }),
            None,
        )?,
        expect: &[Class::LineEndings],
    });

    // --- B: container metadata --------------------------------------------
    let mut reordered = base_plan();
    reordered.swap(3, 4);
    out.push(Fixture {
        id: "entry-order",
        source: base.clone(),
        roundtrip: build(&reordered, None)?,
        expect: &[Class::EntryOrder],
    });

    out.push(Fixture {
        id: "compression-method",
        source: base.clone(),
        roundtrip: build(
            &mutate(base_plan(), "OEBPS/ch1.xhtml", |p| p.stored = true),
            None,
        )?,
        expect: &[Class::CompressionMethod],
    });

    out.push(Fixture {
        id: "comment",
        source: base.clone(),
        roundtrip: build(&base_plan(), Some("written by something else"))?,
        expect: &[Class::Comment],
    });

    // --- C: declared versus actual ----------------------------------------
    let mut not_first = base_plan();
    not_first.swap(0, 1);
    out.push(Fixture {
        id: "mimetype-not-first",
        source: base.clone(),
        roundtrip: build(&not_first, None)?,
        // Moving mimetype out of first position *is* a reordering. Both are true.
        expect: &[Class::EntryOrder, Class::MimetypeNotFirst],
    });

    out.push(Fixture {
        id: "mimetype-compressed",
        source: base.clone(),
        roundtrip: build(&mutate(base_plan(), "mimetype", |p| p.stored = false), None)?,
        // Compressing mimetype *is* a compression-method change. Both are true.
        expect: &[Class::CompressionMethod, Class::MimetypeCompressed],
    });

    // --- The identity case, which is the one that catches a broken instrument.
    out.push(Fixture {
        id: "identical",
        source: base.clone(),
        roundtrip: base,
        expect: &[], // the case that catches an instrument crying wolf
    });

    Ok(out)
}
