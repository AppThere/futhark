// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Writes the synthetic containers to a directory so the round-trip stage can
//! be exercised end to end without a real corpus.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .ok_or("usage: emit_fixtures <dir>")?;
    std::fs::create_dir_all(&dir)?;
    for f in futhark_f2::fixtures::all()? {
        std::fs::write(format!("{dir}/{}.epub", f.id), &f.source)?;
    }
    println!("wrote fixtures to {dir}");
    Ok(())
}
