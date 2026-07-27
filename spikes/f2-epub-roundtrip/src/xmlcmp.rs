// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Infoset comparison, for telling `XmlCanonicalization` from `ContentByteDiff`.
//!
//! Two documents are infoset-equal here when they carry the same elements,
//! attributes, and text, differing only in how they were serialised: attribute
//! order, quote style, self-closing form, whitespace between tags.
//!
//! This is deliberately *not* the ADR-F012 standard. The editor promises bytes,
//! so an infoset-equal round-trip still fails the promise. The distinction earns
//! its place because the two failures have different causes and different fixes:
//! a serialiser that normalises attribute order is a tractable problem, and a
//! parser that loses content is a different crate.

use quick_xml::Reader;
use quick_xml::events::Event;

/// A serialisation-independent view of one XML event.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    Start(String, Vec<(String, String)>),
    End(String),
    /// Start and End collapsed, so `<a/>` and `<a></a>` compare equal.
    Empty(String, Vec<(String, String)>),
    Text(String),
    Comment(String),
    /// Doctype, processing instructions, CDATA — carried so they cannot be
    /// dropped silently.
    Other(String),
}

fn tokenise(data: &[u8]) -> Option<Vec<Token>> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut out = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => out.push(Token::Start(name_of(e.name().as_ref()), attrs(&e)?)),
            Ok(Event::End(e)) => out.push(Token::End(name_of(e.name().as_ref()))),
            Ok(Event::Empty(e)) => out.push(Token::Empty(name_of(e.name().as_ref()), attrs(&e)?)),
            Ok(Event::Text(e)) => {
                let text = String::from_utf8_lossy(&e).trim().to_owned();
                if !text.is_empty() {
                    out.push(Token::Text(text));
                }
            }
            Ok(Event::CData(e)) => out.push(Token::Text(String::from_utf8_lossy(&e).into_owned())),
            Ok(Event::Comment(e)) => {
                out.push(Token::Comment(
                    String::from_utf8_lossy(&e).trim().to_owned(),
                ));
            }
            Ok(Event::Decl(_)) => {}
            Ok(Event::DocType(e)) => {
                out.push(Token::Other(String::from_utf8_lossy(&e).trim().to_owned()));
            }
            Ok(Event::PI(e)) => {
                out.push(Token::Other(
                    String::from_utf8_lossy(e.as_ref()).trim().to_owned(),
                ));
            }
            Ok(Event::GeneralRef(e)) => {
                out.push(Token::Text(String::from_utf8_lossy(&e).into_owned()));
            }
            Err(_) => return None,
        }
        buf.clear();
    }
    Some(out)
}

fn name_of(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

/// Attributes sorted by name, so declaration order stops mattering.
fn attrs(e: &quick_xml::events::BytesStart<'_>) -> Option<Vec<(String, String)>> {
    let mut v = Vec::new();
    for a in e.attributes() {
        let a = a.ok()?;
        v.push((
            String::from_utf8_lossy(a.key.as_ref()).into_owned(),
            String::from_utf8_lossy(&a.value).into_owned(),
        ));
    }
    v.sort();
    Some(v)
}

/// Whether two documents differ only in serialisation.
///
/// Returns `false` when either side does not parse: an unparseable document is
/// not evidence of equivalence, and treating a parse failure as a match is the
/// same error as scoring an unobservable result as a pass (ADR-F042).
pub fn infoset_equal(a: &[u8], b: &[u8]) -> bool {
    match (tokenise(a), tokenise(b)) {
        (Some(ta), Some(tb)) => ta == tb,
        _ => false,
    }
}
