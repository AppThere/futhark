// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! F2 command line.
//!
//! ```text
//! futhark-f2 self-test                  validate the instrument against fixtures
//! futhark-f2 compare <a.epub> <b.epub>  classify one round-trip pair
//! futhark-f2 scan <dir> --tier=b        manifest every .epub under a directory
//! futhark-f2 roundtrip <dir> [--tier=a] [--min-families=N]
//! ```
//!
//! `roundtrip` is the spike proper: open each book with rbook, write it back
//! untouched, classify the difference, and judge the result under ADR-F047. The
//! judgement has no plain "pass" — see `verdict.rs`.
//!
//! `scan` is the Tier B entry point. It writes hashes, producer strings, and
//! divergence classes — never content — so its output is publishable (ADR-F038).

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use futhark_f2::archive::Archive;
use futhark_f2::classify::classify;
use futhark_f2::error::{F2Error, Result};
use futhark_f2::manifest::{Manifest, Record, Tier};
use futhark_f2::producer::read_package;
use futhark_f2::taxonomy::CLASSIFIER_VERSION;
use futhark_f2::verdict::{self, Verdict};
use futhark_f2::{fixtures, taxonomy::Class};
use futhark_f2::{normalization, roundtrip};

fn main() -> ExitCode {
    match run() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("f2: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<bool> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("self-test") => self_test(),
        Some("compare") => compare(args.get(1), args.get(2)),
        Some("scan") => scan(&args),
        Some("roundtrip") => roundtrip_corpus(&args),
        _ => Err(F2Error::Usage(
            "self-test | compare <a> <b> | scan <dir> [--tier=a|b]".into(),
        )),
    }
}

/// The instrument validating itself against inputs whose class is known by
/// construction. A classifier that has never been shown a known answer is not
/// an instrument, it is an opinion.
fn self_test() -> Result<bool> {
    let mut failures = 0;
    for f in fixtures::all()? {
        let a = read_bytes(&f.source)?;
        let b = read_bytes(&f.roundtrip)?;
        let found = classify(&a, &b);
        let classes: Vec<Class> = {
            let mut v: Vec<Class> = found.iter().map(|d| d.class).collect();
            v.sort_unstable();
            v.dedup();
            v
        };

        let mut want = f.expect.to_vec();
        want.sort_unstable();
        let ok = classes == want;

        if ok {
            println!("  ok    {:<22} {}", f.id, describe(&classes));
        } else {
            failures += 1;
            println!(
                "  FAIL  {:<22} expected [{}], got [{}]",
                f.id,
                describe(&want),
                describe(&classes)
            );
            for d in &found {
                println!("           {} {:?} {}", d.class.slug(), d.entry, d.detail);
            }
        }
    }
    println!(
        "\nclassifier {CLASSIFIER_VERSION}: {} failing fixture(s)",
        failures
    );
    Ok(failures == 0)
}

fn describe(classes: &[Class]) -> String {
    if classes.is_empty() {
        "no divergence".to_owned()
    } else {
        classes
            .iter()
            .map(|c| c.slug())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Read an in-memory container by way of a temp file, since the zip reader
/// wants a seekable source and the fixtures live in memory.
fn read_bytes(bytes: &[u8]) -> Result<Archive> {
    let mut path = std::env::temp_dir();
    path.push(format!("f2-{:x}.zip", fnv(bytes)));
    std::fs::write(&path, bytes).map_err(|e| F2Error::Io {
        path: path.display().to_string(),
        source: e,
    })?;
    let a = Archive::read(&path);
    let _ = std::fs::remove_file(&path);
    a
}

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

fn compare(a: Option<&String>, b: Option<&String>) -> Result<bool> {
    let (Some(a), Some(b)) = (a, b) else {
        return Err(F2Error::Usage(
            "compare <source.epub> <roundtrip.epub>".into(),
        ));
    };
    let source = Archive::read(Path::new(a))?;
    let roundtrip = Archive::read(Path::new(b))?;
    let divergences = classify(&source, &roundtrip);

    if divergences.is_empty() {
        println!("identical: every entry byte-for-byte, same order, same storage");
        return Ok(true);
    }
    let mut lossless = true;
    for d in &divergences {
        if d.class.bears_on_losslessness() {
            lossless = false;
        }
        println!(
            "{:<24} {:<40} {}",
            d.class.slug(),
            d.entry.clone().unwrap_or_else(|| "(archive)".into()),
            d.detail
        );
    }
    println!("\nADR-F012 lossless: {lossless}");
    Ok(lossless)
}

fn scan(args: &[String]) -> Result<bool> {
    let dir = args
        .get(1)
        .ok_or_else(|| F2Error::Usage("scan <dir> [--tier=a|b]".into()))?;
    let tier = if args.iter().any(|a| a == "--tier=a") {
        Tier::A
    } else {
        Tier::B
    };

    let mut manifest = Manifest::new();
    for path in find_epubs(Path::new(dir))? {
        let source = Archive::read(&path)?;
        let pkg = read_package(&source);
        let bytes = std::fs::metadata(&path)
            .map_err(|e| F2Error::Io {
                path: path.display().to_string(),
                source: e,
            })?
            .len();
        manifest.records.push(Record {
            source_hash: Archive::file_hash(&path)?,
            source_bytes: bytes,
            tier,
            // Tier B paths are the developer's filesystem and are not published.
            path: matches!(tier, Tier::A).then(|| path.display().to_string()),
            producer: pkg.producer,
            outcome: None,
            normalized_by: normalization::detect(&source, Some(&path)).labels(),
            epub_version: pkg.version,
            entry_count: source.entries.len(),
            // No round-trip yet: the reader under test is ADR-F011's open
            // question. Scanning first gives the producer distribution D12
            // gates on, before a single byte is written back.
            divergences: Vec::new(),
            classifier_version: CLASSIFIER_VERSION.to_owned(),
        });
    }

    let summary = manifest.summary();
    print!("{}", summary.render());
    if summary.total > 0 && summary.distinct_unnormalized_producers < 2 {
        println!(
            "\nR27: this corpus has {} distinct producer(s) among un-normalized\n\
             files. Whatever the overall producer count says, it is measuring one\n\
             writer. Prefer files that have not been through a library manager.",
            summary.distinct_unnormalized_producers,
        );
    }
    std::fs::write("f2-manifest.json", serde_json::to_string_pretty(&manifest)?).map_err(|e| {
        F2Error::Io {
            path: "f2-manifest.json".into(),
            source: e,
        }
    })?;
    println!(
        "\nwrote f2-manifest.json ({} records)",
        manifest.records.len()
    );
    Ok(true)
}

/// The spike proper. Round-trips every book and judges the result under
/// ADR-F047's asymmetry, which is why the exit status follows
/// `resolves_adr_f011` rather than "did anything fail": an inconclusive run is
/// not a success, and a disqualifying one is a successful *measurement*.
fn roundtrip_corpus(args: &[String]) -> Result<bool> {
    let dir = args
        .get(1)
        .ok_or_else(|| F2Error::Usage("roundtrip <dir> [--tier=a] [--min-families=N]".into()))?;
    let tier = if args.iter().any(|a| a == "--tier=a") {
        Tier::A
    } else {
        Tier::B
    };
    let floor = args
        .iter()
        .find_map(|a| a.strip_prefix("--min-families="))
        .and_then(|v| v.parse().ok())
        .unwrap_or(verdict::FAMILY_FLOOR);

    let work = std::env::temp_dir().join("futhark-f2-roundtrip");
    std::fs::create_dir_all(&work).map_err(|e| F2Error::Io {
        path: work.display().to_string(),
        source: e,
    })?;

    let mut manifest = Manifest::new();
    for (i, path) in find_epubs(Path::new(dir))?.into_iter().enumerate() {
        let source = Archive::read(&path)?;
        let pkg = read_package(&source);
        let bytes = std::fs::metadata(&path)
            .map_err(|e| F2Error::Io {
                path: path.display().to_string(),
                source: e,
            })?
            .len();

        let dest = work.join(format!("rt-{i}.epub"));
        let outcome = roundtrip::roundtrip(&path, &dest);
        // Only a produced file can be compared. An absent output is recorded as
        // the outcome it is, never as an empty divergence list.
        let divergences = if outcome.produced_output() {
            match Archive::read(&dest) {
                Ok(rt) => classify(&source, &rt),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let _ = std::fs::remove_file(&dest);

        if !matches!(outcome, roundtrip::Outcome::RoundTripped) {
            println!("  {:?}  {}", outcome, path.display());
        }

        manifest.records.push(Record {
            source_hash: Archive::file_hash(&path)?,
            source_bytes: bytes,
            tier,
            path: matches!(tier, Tier::A).then(|| path.display().to_string()),
            producer: pkg.producer,
            normalized_by: normalization::detect(&source, Some(&path)).labels(),
            epub_version: pkg.version,
            entry_count: source.entries.len(),
            divergences,
            outcome: Some(outcome),
            classifier_version: CLASSIFIER_VERSION.to_owned(),
        });
    }

    let summary = manifest.summary();
    print!("\n{}", summary.render());

    let verdict = Verdict::judge(&summary, floor);
    println!("\n{}\n", verdict.render());
    std::fs::write("f2-manifest.json", serde_json::to_string_pretty(&manifest)?).map_err(|e| {
        F2Error::Io {
            path: "f2-manifest.json".into(),
            source: e,
        }
    })?;
    println!(
        "wrote f2-manifest.json ({} records)",
        manifest.records.len()
    );

    Ok(verdict.resolves_adr_f011())
}

fn find_epubs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| F2Error::Io {
        path: dir.display().to_string(),
        source: e,
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(find_epubs(&path)?);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("epub"))
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
