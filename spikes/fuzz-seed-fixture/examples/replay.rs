// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Feed one seed to the parser, outside libFuzzer.
//!
//! `cargo-fuzz` needs a nightly toolchain and a long run; `check-fuzz-seeds`
//! needs to know *today* that the target reaches the parser. This runs the same
//! function the fuzz target calls, on the same seed, in a normal build — so the
//! check has something it can actually execute rather than a directory listing
//! it can only describe.

fn main() {
    let path = std::env::args().nth(1).expect("usage: replay <seed>");
    let data = std::fs::read(&path).expect("seed readable");
    let out = futhark_fuzz_seed_fixture::parse(&data);
    // Reached only when the seed failed to crash the parser, which is itself
    // the finding this check exists to surface.
    println!("no crash: parsed {} bytes", out.len());
}
