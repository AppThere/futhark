// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Walking a corpus directory.
//!
//! Shared by `scan`, `roundtrip`, and `characterise` so the three commands
//! cannot disagree about which files were in the population — a difference
//! there would show up as a difference in results and be read as a finding.

use std::path::{Path, PathBuf};

use crate::error::{F2Error, Result};

/// Every `.epub` under `dir`, recursively, in sorted order.
///
/// Sorted so two runs over the same directory produce the same manifest order;
/// an unstable order makes two identical runs look like they diverged.
pub fn find_epubs(dir: &Path) -> Result<Vec<PathBuf>> {
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
