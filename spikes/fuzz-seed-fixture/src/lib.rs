// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! A parser with a planted bug, and the seed that finds it (ADR-F060).
//!
//! `check-fuzz-seeds` exists so that "0 crashes in 24 hours" cannot be reported
//! by a target that never reaches its parser — the `manifest-mismatch: 0` bug at
//! a larger scale and a longer timescale. But a check with no inputs reports
//! nothing about anything, and the whole reason to build this one before the
//! first codec crate is that it should be live *now*. So it ships with a
//! subject.
//!
//! This crate is that subject. The parser below panics on a specific input, and
//! `fuzz/corpus/parse_fixture/known-crash` is that input. The check runs the
//! replay and requires the panic. If the harness ever stops reaching the parser,
//! this is what stops crashing.
//!
//! **It is exempt from the no-panics-on-malformed-input rule by construction**,
//! not by oversight: a crash is its entire function. It lives outside the
//! workspace and nothing in `crates/` may depend on it.

#![forbid(unsafe_code)]

/// The planted defect: a length prefix trusted without checking it against the
/// buffer. Exactly the shape a real codec bug takes, which is the point — a
/// seed that triggers something artificial would prove the harness reaches an
/// artificial thing.
pub const PANIC_MESSAGE: &str = "futhark-fuzz-seed-fixture: length prefix exceeds buffer";

/// Parse a trivial length-prefixed record.
///
/// Returns the payload. Panics when the declared length runs past the end of
/// the input, which is what the seeded crash input triggers.
pub fn parse(data: &[u8]) -> &[u8] {
    let Some((&len, rest)) = data.split_first() else {
        return &[];
    };
    let len = usize::from(len);
    if len > rest.len() {
        panic!("{PANIC_MESSAGE}");
    }
    &rest[..len]
}
