// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0

//! Spike F1c — custom-scheme origin semantics.
//!
//! The smallest Tauri app that can answer R23: does an iframe served through a
//! registered URI scheme behave like the opaque, CSP-constrained, sandboxed
//! origin ADR-F005 and ADR-F007 assume?
//!
//! On macOS the registered scheme is backed by `WKURLSchemeHandler`, which is
//! the whole point. Spike F1b serves the same probe over `http` through
//! safaridriver; if the two disagree, the difference is the scheme, because
//! nothing else about the probe changes.
//!
//! This is spike code. It is outside the Cargo workspace, it is not the
//! presentation shell, and no `futhark-*` crate may depend on it (ADR-F002).

#![forbid(unsafe_code)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::borrow::Cow;
use std::path::PathBuf;

use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, UriSchemeContext};

/// Assets are embedded rather than read from disk so the scheme handler has no
/// filesystem dependency — one less difference between this and the `http`
/// control, and one less thing to explain if the results diverge.
const CONTENT_XHTML: &[u8] = include_bytes!("../../probe/content.xhtml");
const PROBE_JS: &[u8] = include_bytes!("../../probe/probe.js");
const BOOK_CSS: &[u8] = include_bytes!("../../probe/book.css");
const PROBE_PNG: &[u8] = include_bytes!("../../probe/probe.png");

/// The Content-Security-Policy served with book resources.
///
/// `default-src 'none'` forces every resource class to be named. Whether the
/// engine applies this header at all to a custom-scheme response is one of the
/// things F1c is measuring, so it is served exactly as the real handler would
/// serve it — no relaxation to make the probe convenient.
const CSP: &str = "default-src 'none'; \
     script-src 'self' futhark-content://localhost; \
     style-src 'self' futhark-content://localhost; \
     img-src 'self' data: futhark-content://localhost; \
     font-src 'self' futhark-content://localhost; \
     connect-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'";

fn asset_for(path: &str) -> Option<(&'static [u8], &'static str)> {
    match path.trim_start_matches('/') {
        "" | "content.xhtml" => Some((CONTENT_XHTML, "application/xhtml+xml")),
        "probe.js" => Some((PROBE_JS, "application/javascript")),
        "book.css" => Some((BOOK_CSS, "text/css")),
        "probe.png" => Some((PROBE_PNG, "image/png")),
        _ => None,
    }
}

/// Stamp the transport onto the served document so the probe reports which
/// experiment produced it without the probe itself having to know.
fn stamp_transport(body: &'static [u8]) -> Vec<u8> {
    String::from_utf8_lossy(body)
        .replace("data-transport=\"unknown\"", "data-transport=\"custom-scheme\"")
        .into_bytes()
}

fn serve(request: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = request.uri().path().to_owned();
    match asset_for(&path) {
        Some((body, mime)) => {
            let payload: Cow<'static, [u8]> = if mime == "application/xhtml+xml" {
                Cow::Owned(stamp_transport(body))
            } else {
                Cow::Borrowed(body)
            };
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", mime)
                .header("content-security-policy", CSP)
                .header("x-content-type-options", "nosniff")
                .header("cache-control", "no-store")
                .body(payload)
                .unwrap_or_else(|_| Response::new(Cow::Borrowed(b"builder error".as_slice())))
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("content-type", "text/plain")
            .body(Cow::Borrowed(b"not found".as_slice()))
            .unwrap_or_else(|_| Response::new(Cow::Borrowed(b"not found".as_slice()))),
    }
}

/// Receives the probe result from the app origin and writes it next to the
/// binary, so the run leaves an artifact rather than a screenshot.
#[tauri::command]
fn report(app: tauri::AppHandle, payload: serde_json::Value) -> Result<String, String> {
    let dir: PathBuf = app
        .path()
        .resolve("", tauri::path::BaseDirectory::Temp)
        .map_err(|e| e.to_string())?;
    let path = dir.join("f1c-result.json");
    let text = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    std::fs::write(&path, text.as_bytes()).map_err(|e| e.to_string())?;
    println!("\n=== F1c probe result ===\n{text}\n=== written to {} ===", path.display());
    Ok(path.display().to_string())
}

fn main() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("futhark-content", |_ctx: UriSchemeContext<'_, _>, request| {
            serve(&request)
        })
        .invoke_handler(tauri::generate_handler![report])
        .run(tauri::generate_context!())
        .expect("failed to run the F1c probe shell");
}
