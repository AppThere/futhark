// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! ADR-F062: a scratch file that cannot be shared, by construction.
//!
//! The predecessor keyed its temp path on the content hash and deleted the file
//! after reading. That is isolation by convention — correct only while no two
//! callers hold the same bytes at the same time. It was latent from the day it
//! was written, invisible at two callers, and surfaced when a third test read
//! the same fixture concurrently: one finished, removed the file, and the other
//! got `InvalidArchive`.
//!
//! **The loud failure was the lucky case.** A truncated-but-plausible read would
//! have classified into a real class and been believed, and nothing in the
//! suite distinguishes those two outcomes. So the fix is not a better key — a
//! better key is still a convention — but a type that owns a private directory
//! and removes it on drop. Two callers cannot collide because they cannot name
//! the same path.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{F2Error, Result};

/// A file in a directory nothing else can name, removed when this is dropped.
#[derive(Debug)]
pub struct Scratch {
    dir: PathBuf,
    file: PathBuf,
}

impl Scratch {
    /// Write `bytes` into a fresh private directory.
    ///
    /// Uniqueness comes from the process id and a monotonic counter, so it holds
    /// across threads within a process and across concurrent test binaries.
    pub fn write(name: &str, bytes: &[u8]) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("futhark-f2-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| F2Error::Io {
            path: dir.display().to_string(),
            source: e,
        })?;
        let file = dir.join(name);
        std::fs::write(&file, bytes).map_err(|e| F2Error::Io {
            path: file.display().to_string(),
            source: e,
        })?;
        Ok(Self { dir, file })
    }

    /// The path to the written file. Borrowed from `self`, so it cannot outlive
    /// the directory that holds it.
    pub fn path(&self) -> &Path {
        &self.file
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: a failure to clean up leaves a temp directory behind,
        // which is a nuisance rather than a wrong answer. Panicking in `drop`
        // would turn a nuisance into a lost test result.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
