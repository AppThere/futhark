// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Spike F2 — EPUB round-trip divergence classifier and result manifest.
//!
//! F2 asks whether a reader can round-trip real EPUBs without changing them.
//! The answer is only meaningful if divergence is classified before it is
//! counted (ADR-F036): a bare byte-difference percentage over a zip archive
//! measures the writer's zip library, not its fidelity to the book.
//!
//! The deliverable is the manifest (ADR-F038) — what diverged, in what class,
//! for which file — not a single pass/fail. That is what tells you whether
//! `rbook` is fixable or replaceable, which is the decision ADR-F011 is waiting
//! on.
//!
//! F2b reuses the same reading machinery for a different question — what real
//! EPUBs *contain* that `futhark-epub` must preserve. Its deliverable has no
//! verdict to carry a caveat, so the caveat is two types: a three-state feature
//! catalogue (ADR-F052) and a floor that cannot be read as a specification
//! without an explicit widening (ADR-F051).
//!
//! This is spike code: outside the Cargo workspace, and nothing in `crates/`
//! may depend on it.

#![forbid(unsafe_code)]

pub mod archive;
pub mod catalogue;
pub mod characterise;
pub mod classify;
pub mod corpus;
pub mod detect;
pub mod error;
pub mod family;
pub mod fixture_cases;
pub mod fixtures;
pub mod floor;
pub mod manifest;
pub mod normalization;
pub mod producer;
pub mod provenance;
pub mod roundtrip;
pub mod selftest;
pub mod taxonomy;
pub mod verdict;
pub mod xmlcmp;
