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
//! This is spike code: outside the Cargo workspace, and nothing in `crates/`
//! may depend on it.

#![forbid(unsafe_code)]

pub mod archive;
pub mod classify;
pub mod error;
pub mod fixtures;
pub mod manifest;
pub mod normalization;
pub mod producer;
pub mod roundtrip;
pub mod taxonomy;
pub mod verdict;
pub mod xmlcmp;
