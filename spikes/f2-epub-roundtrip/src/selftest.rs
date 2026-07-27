// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The instrument validating itself.
//!
//! Split out of the command line because it is the part of F2 that has to be
//! reviewable on its own: every other command reports what it found, and this
//! one reports whether the thing doing the finding works. A classifier that has
//! never been shown an input whose class is known by construction is not an
//! instrument, it is an opinion.

use crate::archive::Archive;
use crate::classify::classify;
use crate::error::Result;
use crate::fixtures;
use crate::scratch::Scratch;
use crate::taxonomy::{CLASSIFIER_VERSION, Class};

/// The instrument validating itself against inputs whose class is known by
/// construction. A classifier that has never been shown a known answer is not
/// an instrument, it is an opinion.
pub fn self_test() -> Result<bool> {
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
///
/// Public so `tests/reachability.rs` can drive the fixtures through the same
/// path the self-test uses. ADR-F053 wants witnesses reached through the real
/// code path; a test that built its own reader would be witnessing itself.
///
/// The scratch file owns a private directory (ADR-F062). Concurrent callers
/// with identical bytes used to share one path and delete it from under each
/// other.
pub fn read_bytes(bytes: &[u8]) -> Result<Archive> {
    let scratch = Scratch::write("container.zip", bytes)?;
    Archive::read(scratch.path())
}
