// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Producer *families* (ADR-F050).
//!
//! D12's coverage gate counts families, not strings. "InDesign 17" and
//! "InDesign 19" are one toolchain wearing two labels, so a string count
//! overstates diversity — the R27 shape one level up: a healthy-looking number
//! produced by the wrong mechanism.
//!
//! Classification is by known-toolchain substring first, then by a version-
//! stripped fallback for anything unrecognised. The fallback deliberately does
//! not try to be clever: an unrecognised producer becomes its own family under
//! a cleaned-up name, which over-counts diversity slightly rather than merging
//! two genuinely different toolchains into one. Over-counting is visible in the
//! named list; silent merging is not.

use std::collections::BTreeMap;

/// Toolchains recognised by substring, longest and most specific first.
/// The left side is matched case-insensitively against the producer string.
const KNOWN: [(&str, &str); 16] = [
    ("adobe indesign", "InDesign"),
    ("indesign", "InDesign"),
    ("sigil", "Sigil"),
    ("calibre", "calibre"),
    ("ebookmaker", "ebookmaker (Gutenberg)"),
    ("project gutenberg", "ebookmaker (Gutenberg)"),
    ("standard ebooks", "Standard Ebooks"),
    ("vellum", "Vellum"),
    ("pressbooks", "Pressbooks"),
    ("pages", "Apple Pages"),
    ("ibooks author", "iBooks Author"),
    ("kindlegen", "KindleGen"),
    ("jutoh", "Jutoh"),
    ("pubcoder", "PubCoder"),
    ("atlantis", "Atlantis"),
    ("pandoc", "Pandoc"),
];

/// The label used when a package declares no producer at all. Its own family:
/// "declares nothing" is a real and common toolchain behaviour, and folding it
/// into the others would hide it.
pub const UNDECLARED: &str = "(undeclared)";

/// Map a producer string to its family.
pub fn classify(producer: Option<&str>) -> String {
    let Some(raw) = producer else {
        return UNDECLARED.to_owned();
    };
    let lower = raw.to_ascii_lowercase();
    if lower.trim().is_empty() {
        return UNDECLARED.to_owned();
    }
    for (needle, family) in KNOWN {
        if lower.contains(needle) {
            return (*family).to_owned();
        }
    }
    strip_version(raw)
}

/// Fallback for unrecognised producers: drop version-looking tokens so the same
/// tool at two releases lands in one family, and keep the rest verbatim.
fn strip_version(raw: &str) -> String {
    let kept: Vec<&str> = raw
        .split_whitespace()
        .take_while(|t| !looks_like_version(t))
        .collect();
    let name = if kept.is_empty() {
        raw.trim()
    } else {
        &kept.join(" ")
    };
    let trimmed = name.trim_matches(|c: char| !c.is_alphanumeric()).trim();
    if trimmed.is_empty() {
        raw.trim().to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn looks_like_version(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
    !t.is_empty() && t.starts_with(|c: char| c.is_ascii_digit())
        || t.starts_with('v')
            && t.get(1..)
                .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()))
}

/// Share of the largest family, as a percentage of the population.
///
/// A count alone cannot tell twenty families where one holds 95% from six held
/// evenly, and the first is the worse corpus. This is the second half of the
/// gate for that reason.
pub fn largest_share_pct(families: &BTreeMap<String, usize>) -> f64 {
    let total: usize = families.values().sum();
    if total == 0 {
        return 0.0;
    }
    let largest = families.values().copied().max().unwrap_or(0);
    (largest as f64 / total as f64) * 100.0
}
