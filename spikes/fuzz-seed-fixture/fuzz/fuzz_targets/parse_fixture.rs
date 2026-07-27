// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The libFuzzer target. Shape only — CI runs `examples/replay.rs` instead,
//! because `cargo-fuzz` wants nightly and the seed check has to be cheap enough
//! to run on every push.

#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let _ = futhark_fuzz_seed_fixture::parse(data);
});
