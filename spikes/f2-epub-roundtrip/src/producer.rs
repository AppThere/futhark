// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Producer-string extraction, for D12's coverage gate.
//!
//! D12 resolved that Tier B is gated on distinct producers rather than file
//! count: two hundred files from four toolchains is a weaker test than sixty
//! from twenty. That only works if the producer can be named, so this digs it
//! out of wherever the toolchain left it.
//!
//! There is no standard field. `dc:contributor` with `opf:role="bkp"` is the
//! conventional one, `<meta name="generator">` is common, and several tools
//! leave nothing at all — which is itself a producer class worth counting, so
//! an undeclared producer is reported rather than dropped.

use quick_xml::Reader;
use quick_xml::events::Event;

use crate::archive::Archive;

/// What the OPF says about itself.
#[derive(Debug, Clone, Default)]
pub struct PackageInfo {
    /// Producer string, if the package declares one.
    pub producer: Option<String>,
    /// `version` attribute of the `<package>` element.
    pub version: Option<String>,
    /// Manifest hrefs, for the `ManifestMismatch` check.
    pub manifest_hrefs: Vec<String>,
}

/// Find the OPF and read what it declares.
pub fn read_package(archive: &Archive) -> PackageInfo {
    let Some(opf) = find_opf(archive) else {
        return PackageInfo::default();
    };
    parse_opf(&opf.data)
}

/// Locate the package document via `META-INF/container.xml`, falling back to a
/// scan. A malformed container is common enough in the wild that giving up on
/// it would silently under-count producers.
fn find_opf(archive: &Archive) -> Option<&crate::archive::Entry> {
    if let Some(e) = archive
        .get("META-INF/container.xml")
        .and_then(|c| full_path_from_container(&c.data))
        .and_then(|path| archive.get(&path))
    {
        return Some(e);
    }
    archive
        .entries
        .iter()
        .find(|e| e.name.to_ascii_lowercase().ends_with(".opf"))
}

fn full_path_from_container(data: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => return None,
            Ok(Event::Start(e) | Event::Empty(e)) if e.name().as_ref() == b"rootfile" => {
                for a in e.attributes().flatten() {
                    if a.key.as_ref() == b"full-path" {
                        return Some(String::from_utf8_lossy(&a.value).into_owned());
                    }
                }
            }
            Ok(_) => {}
            Err(_) => return None,
        }
        buf.clear();
    }
}

fn parse_opf(data: &[u8]) -> PackageInfo {
    let mut info = PackageInfo::default();
    let mut reader = Reader::from_reader(data);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    // `dc:contributor role="bkp"` names the producer, but the role sits either
    // on the element or in a separate refines meta; the text is only useful once
    // we know the element it belonged to.
    let mut pending_contributor = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e) | Event::Empty(e)) => {
                let name = e.name().as_ref().to_ascii_lowercase();
                let local = local_name(&name);
                match local {
                    b"package" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"version" {
                                info.version = Some(String::from_utf8_lossy(&a.value).into_owned());
                            }
                        }
                    }
                    b"item" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"href" {
                                info.manifest_hrefs
                                    .push(String::from_utf8_lossy(&a.value).into_owned());
                            }
                        }
                    }
                    b"meta" => {
                        let mut is_generator = false;
                        let mut content = None;
                        for a in e.attributes().flatten() {
                            let key = a.key.as_ref().to_ascii_lowercase();
                            let val = String::from_utf8_lossy(&a.value).into_owned();
                            match key.as_slice() {
                                b"name" if val.eq_ignore_ascii_case("generator") => {
                                    is_generator = true;
                                }
                                b"content" => content = Some(val),
                                _ => {}
                            }
                        }
                        if is_generator && info.producer.is_none() {
                            info.producer = content;
                        }
                    }
                    b"contributor" => pending_contributor = true,
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if pending_contributor => {
                if info.producer.is_none() {
                    let text = String::from_utf8_lossy(&e).trim().to_owned();
                    if !text.is_empty() {
                        info.producer = Some(text);
                    }
                }
                pending_contributor = false;
            }
            Ok(Event::End(_)) => pending_contributor = false,
            Ok(_) => {}
            Err(_) => break,
        }
        buf.clear();
    }
    info
}

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == b':') {
        Some(i) => name.get(i + 1..).unwrap_or(name),
        None => name,
    }
}
