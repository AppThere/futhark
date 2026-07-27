// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! The round-trip stage: open a container with `rbook`, write it back untouched,
//! and hand both to the classifier.
//!
//! Nothing is edited between read and write. That is the whole experiment —
//! ADR-F012 promises that saving a file the user did not touch produces
//! byte-identical output, so the no-op save is the promise at its weakest and
//! most testable point.
//!
//! A reader that *panics* on a real book is a decisive F2 result, not a reason
//! for the run to die: the outcome is recorded and the sweep continues. Futhark's
//! own no-panic rule is about Futhark's code, and a dependency that violates it
//! is exactly what this spike exists to discover.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What happened when the reader was pointed at one book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "outcome")]
pub enum Outcome {
    /// Read and written; the pair is ready to classify.
    RoundTripped,
    /// The reader refused the file. Reported, not swallowed — a book Futhark
    /// cannot open is a product failure even when nothing was corrupted.
    ReadFailed {
        /// The reader's own error text.
        error: String,
    },
    /// The reader parsed the file but could not write it back.
    WriteFailed {
        /// The reader's own error text.
        error: String,
    },
    /// The reader panicked. Decisive against adoption on its own: it violates
    /// the no-panic-on-malformed-input rule the codecs are held to, and no
    /// wrapper can fix a panic in a dependency's parser.
    Panicked {
        /// Panic payload, where it was a string.
        message: String,
    },
}

impl Outcome {
    /// Whether a comparable output file was produced.
    pub fn produced_output(&self) -> bool {
        matches!(self, Self::RoundTripped)
    }
}

/// Read `source` with `rbook` and write it back to `dest`, unmodified.
///
/// Never returns `Err`: every failure mode is a *finding* about the reader and
/// belongs in the manifest rather than in the caller's error path.
pub fn roundtrip(source: &Path, dest: &Path) -> Outcome {
    let source = source.to_path_buf();
    let dest = dest.to_path_buf();

    let result = catch_unwind(AssertUnwindSafe(move || {
        let epub = match rbook::Epub::open(&source) {
            Ok(e) => e,
            Err(e) => {
                return Outcome::ReadFailed {
                    error: e.to_string(),
                };
            }
        };
        match epub.write().save(&dest) {
            Ok(()) => Outcome::RoundTripped,
            Err(e) => Outcome::WriteFailed {
                error: e.to_string(),
            },
        }
    }));

    result.unwrap_or_else(|payload| Outcome::Panicked {
        message: panic_message(&payload),
    })
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "(non-string panic payload)".to_owned()
    }
}
