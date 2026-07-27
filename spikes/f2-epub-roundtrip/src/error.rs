// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Typed errors. `thiserror`, no `Box<dyn Error>` in a public signature.

use thiserror::Error;

/// Every way F2 can fail.
#[derive(Debug, Error)]
pub enum F2Error {
    /// Filesystem failure, with the path that caused it.
    #[error("io error on {path}: {source}")]
    Io {
        /// Path or archive entry being read or written.
        path: String,
        /// Underlying failure.
        source: std::io::Error,
    },

    /// The archive could not be read as a zip container.
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Serialising or deserialising a manifest.
    #[error("manifest serialisation: {0}")]
    Json(#[from] serde_json::Error),

    /// The container is not a usable OCF package.
    #[error("not a usable OCF container: {0}")]
    NotOcf(String),

    /// A CLI argument did not make sense.
    #[error("usage: {0}")]
    Usage(String),
}

/// Result alias.
pub type Result<T> = std::result::Result<T, F2Error>;
