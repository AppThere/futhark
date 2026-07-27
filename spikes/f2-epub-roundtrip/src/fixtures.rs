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

use zip::write::{ExtendedFileOptions, FileOptions};
use zip::{CompressionMethod, ZipWriter};

use crate::error::Result;

/// The cases live in `fixture_cases`; re-exported so `fixtures::all()` stays
/// the one entry point callers know.
pub use crate::fixture_cases::all;
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

pub(crate) const OPF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
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

pub(crate) const CHAPTER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\"><head><title>One</title></head>\n\
<body><p id=\"a\" class=\"x\">Hello.</p></body></html>\n";

const CONTAINER: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles><rootfile full-path="OEBPS/package.opf"
    media-type="application/oebps-package+xml"/></rootfiles>
</container>
"#;

/// One entry as it will be written.
pub struct Plan {
    pub(crate) name: &'static str,
    pub(crate) body: Vec<u8>,
    pub(crate) stored: bool,
    /// A zip extra field, as `(header id, payload)`. Unix permissions, UT
    /// timestamps, and Zip64 headers all ride here in the wild.
    pub(crate) extra: Option<(u16, Vec<u8>)>,
}

pub(crate) fn base_plan() -> Vec<Plan> {
    vec![
        Plan {
            name: "mimetype",
            body: b"application/epub+zip".to_vec(),
            stored: true,
            extra: None,
        },
        Plan {
            name: "META-INF/container.xml",
            body: CONTAINER.as_bytes().to_vec(),
            stored: false,
            extra: None,
        },
        Plan {
            name: "OEBPS/package.opf",
            body: OPF.as_bytes().to_vec(),
            stored: false,
            extra: None,
        },
        Plan {
            name: "OEBPS/nav.xhtml",
            body: NAV.as_bytes().to_vec(),
            stored: false,
            extra: None,
        },
        Plan {
            name: "OEBPS/toc.ncx",
            body: NCX.as_bytes().to_vec(),
            stored: false,
            extra: None,
        },
        Plan {
            name: "OEBPS/ch1.xhtml",
            body: CHAPTER.as_bytes().to_vec(),
            stored: false,
            extra: None,
        },
    ]
}

/// Fixed timestamp everywhere, so `Timestamp` only fires when a fixture asks it to.
fn opts(p: &Plan) -> Result<FileOptions<'static, ExtendedFileOptions>> {
    let method = if p.stored {
        CompressionMethod::Stored
    } else {
        CompressionMethod::Deflated
    };
    let mut o = FileOptions::<ExtendedFileOptions>::default()
        .compression_method(method)
        .last_modified_time(zip::DateTime::default());
    if let Some((id, data)) = &p.extra {
        o.add_extra_data(*id, data, false)?;
    }
    Ok(o)
}

pub(crate) fn build(plans: &[Plan], comment: Option<&str>) -> Result<Vec<u8>> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    if let Some(c) = comment {
        zw.set_comment(c)?;
    }
    for p in plans {
        zw.start_file(p.name, opts(p)?)?;
        zw.write_all(&p.body)
            .map_err(|e| crate::error::F2Error::Io {
                path: p.name.to_owned(),
                source: e,
            })?;
    }
    Ok(zw.finish()?.into_inner())
}

/// Rewrite the uncompressed-size field in an entry's central directory record,
/// leaving the bytes it describes untouched.
///
/// No zip writer will emit this, which is the point: `declared-size-mismatch`
/// is a malformed-input class, and a container that lies about itself is the
/// only witness for it. The CRC is left correct so the archive still opens —
/// the header contradicts the entry, rather than the entry being corrupt.
pub(crate) fn lie_about_size(zip: &[u8], entry: &str, claim: u32) -> Vec<u8> {
    const CENTRAL_HEADER: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    let mut out = zip.to_vec();
    let mut i = 0;
    while i + 46 <= out.len() {
        if out[i..i + 4] != CENTRAL_HEADER {
            i += 1;
            continue;
        }
        let name_len = u16::from_le_bytes([out[i + 28], out[i + 29]]) as usize;
        let name = String::from_utf8_lossy(&out[i + 46..i + 46 + name_len]).into_owned();
        if name == entry {
            out[i + 24..i + 28].copy_from_slice(&claim.to_le_bytes());
            break;
        }
        i += 46 + name_len;
    }
    out
}

pub(crate) fn mutate(mut plans: Vec<Plan>, name: &str, f: impl Fn(&mut Plan)) -> Vec<Plan> {
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

/// One extra entry, for fixtures that add a file.
pub(crate) fn plan(name: &'static str, body: Vec<u8>) -> Plan {
    Plan {
        name,
        body,
        stored: false,
        extra: None,
    }
}

/// The unmodified container every fixture is measured against.
pub(crate) fn base() -> Result<Vec<u8>> {
    build(&base_plan(), None)
}
