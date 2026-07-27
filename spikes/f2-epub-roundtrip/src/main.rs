// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! F2 command line.
//!
//! ```text
//! futhark-f2 self-test                  validate the instrument against fixtures
//! futhark-f2 compare <a.epub> <b.epub>  classify one round-trip pair
//! futhark-f2 scan <dir> --tier=b        manifest every .epub under a directory
//! futhark-f2 roundtrip <dir> [--tier=a] [--min-families=N]
//! futhark-f2 characterise <dir> [--tier=a]   F2b: enumerate the population
//! ```
//!
//! `roundtrip` is the spike proper: open each book with rbook, write it back
//! untouched, classify the difference, and judge the result under ADR-F047. The
//! judgement has no plain "pass" — see `verdict.rs`.
//!
//! `scan` is the Tier B entry point. It writes hashes, producer strings, and
//! divergence classes — never content — so its output is publishable (ADR-F038).
//!
//! `characterise` is F2b. It writes nothing back and judges no reader; it
//! enumerates what the corpus contains against a fixed catalogue and emits a
//! floor — see `floor.rs` for why that output is not a requirements list.

#![forbid(unsafe_code)]

use std::path::Path;
use std::process::ExitCode;

use futhark_f2::archive::Archive;
use futhark_f2::characterise::characterise;
use futhark_f2::classify::classify;
use futhark_f2::corpus::find_epubs;
use futhark_f2::error::{F2Error, Result};
use futhark_f2::manifest::{Manifest, Record, Tier};
use futhark_f2::producer::read_package;
use futhark_f2::taxonomy::CLASSIFIER_VERSION;
use futhark_f2::verdict::{self, Verdict};
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
        Some("self-test") => futhark_f2::selftest::self_test(),
        Some("compare") => compare(args.get(1), args.get(2)),
        Some("scan") => scan(&args),
        Some("roundtrip") => roundtrip_corpus(&args),
        Some("characterise") => {
            let dir = args
                .get(1)
                .ok_or_else(|| F2Error::Usage("characterise <dir> [--tier=a|b]".into()))?;
            characterise(Path::new(dir), tier_of(&args))
        }
        _ => Err(F2Error::Usage(
            "self-test | compare <a> <b> | scan <dir> [--tier=a|b] \
             | roundtrip <dir> | characterise <dir>"
                .into(),
        )),
    }
}

/// Tier from the flags, defaulting to B — the developer's own library is the
/// common case and the one whose paths must never reach the manifest.
fn tier_of(args: &[String]) -> Tier {
    if args.iter().any(|a| a == "--tier=a") {
        Tier::A
    } else {
        Tier::B
    }
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
    let tier = tier_of(args);

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
    let tier = tier_of(args);
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

    // ADR-F061: the fixture gaps that were supposed to be contingent. Reported
    // here rather than left for a reader to notice, because a zero that was
    // predicted to become non-zero is the one number nobody re-checks.
    let undissolved = summary.undissolved_contingent_gaps();
    if !undissolved.is_empty() {
        println!(
            "ADR-F061 — {} books, and these classes are still zero: {}.\n\
             The fixture corpus cannot produce them because the synthetic writer\n\
             pins them; a real library has no such discipline. A zero here is a\n\
             finding about the detector, not about the corpus.\n",
            summary.total,
            undissolved.join(", "),
        );
    }
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
