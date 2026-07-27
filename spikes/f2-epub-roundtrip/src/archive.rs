// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Reading an OCF archive into a comparable model.
//!
//! The classifier needs more than the bytes: it needs the container facts the
//! bytes are wrapped in, because ADR-F036's whole point is telling those two
//! apart. So this reads entries *and* their storage metadata, in central
//! directory order, and keeps the order.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{F2Error, Result};

/// One archive entry, with the container metadata the classifier compares.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Path within the container, as written.
    pub name: String,
    /// Uncompressed bytes.
    pub data: Vec<u8>,
    /// Compression method, as reported by the zip reader.
    pub method: String,
    /// Compressed size on disk. Differs with compression level for identical
    /// input, which is exactly the `CompressionLevel` class.
    pub compressed_size: u64,
    /// Last-modified timestamp, if the entry carries one.
    pub modified: Option<String>,
    /// CRC-32 recorded in the archive, not recomputed.
    pub declared_crc: u32,
    /// Uncompressed size recorded in the archive, not measured.
    pub declared_size: u64,
    /// Per-entry comment.
    pub comment: String,
    /// Raw extra field bytes.
    pub extra: Vec<u8>,
}

impl Entry {
    /// SHA-256 of the uncompressed bytes. Used in the manifest so a file can be
    /// identified without being redistributed (ADR-F038).
    pub fn content_hash(&self) -> String {
        let mut h = Sha256::new();
        h.update(&self.data);
        format!("{:x}", h.finalize())
    }
}

/// A whole OCF container, in the order the archive stores it.
#[derive(Debug, Clone)]
pub struct Archive {
    /// Entries in central-directory order. Order is data, not incidental —
    /// `EntryOrder` is a class.
    pub entries: Vec<Entry>,
    /// Archive-level comment.
    pub comment: String,
}

impl Archive {
    /// Read an archive from disk.
    pub fn read(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path).map_err(|e| F2Error::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut zip = zip::ZipArchive::new(file)?;
        let comment = String::from_utf8_lossy(zip.comment()).into_owned();

        let mut entries = Vec::with_capacity(zip.len());
        for i in 0..zip.len() {
            let mut f = zip.by_index(i)?;
            if f.is_dir() {
                continue;
            }
            let mut data = Vec::new();
            f.read_to_end(&mut data).map_err(|e| F2Error::Io {
                path: f.name().to_owned(),
                source: e,
            })?;
            entries.push(Entry {
                name: f.name().to_owned(),
                method: format!("{:?}", f.compression()),
                compressed_size: f.compressed_size(),
                modified: f.last_modified().map(|t| format!("{t:?}")),
                declared_crc: f.crc32(),
                declared_size: f.size(),
                comment: f.comment().to_owned(),
                extra: f.extra_data().map(<[u8]>::to_vec).unwrap_or_default(),
                data,
            });
        }
        Ok(Self { entries, comment })
    }

    /// Look an entry up by name.
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Entry names in stored order.
    pub fn order(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// SHA-256 over the whole file as it sits on disk. This is the manifest's
    /// identity for a Tier B book: enough to prove which file produced a
    /// result, carrying none of its content.
    pub fn file_hash(path: &Path) -> Result<String> {
        let bytes = std::fs::read(path).map_err(|e| F2Error::Io {
            path: path.display().to_string(),
            source: e,
        })?;
        let mut h = Sha256::new();
        h.update(&bytes);
        Ok(format!("{:x}", h.finalize()))
    }
}
