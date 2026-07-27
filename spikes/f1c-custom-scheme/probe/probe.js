// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// The origin probe. Runs inside the content frame and answers one question:
// what does this engine actually permit, given how this document was delivered?
//
// It is deliberately transport-agnostic. The same file runs unmodified over
// `http` (the control) and over `futhark-content://` (the real configuration),
// because R23 is precisely the claim that those two are not the same experiment.
// If the probe differed between them, it could not be evidence about the
// difference.
//
// Every check reports what happened rather than asserting. A probe that throws
// on the first surprise tells you less than one that finishes and shows you the
// shape of the surprise.

(function () {
  'use strict';

  var results = {};

  /** Run a check, recording the outcome rather than propagating a throw. */
  function check(name, fn) {
    try {
      results[name] = { ok: true, value: fn() };
    } catch (err) {
      results[name] = { ok: false, threw: (err && err.name) || 'Error', message: String((err && err.message) || err).slice(0, 200) };
    }
    return results[name];
  }

  // --- Origin identity -----------------------------------------------------
  //
  // `window.origin` is the document's origin and serialises to "null" when it
  // is opaque. `location.origin` is NOT the same thing — it is computed from
  // the URL and reports a real origin even inside an opaque-origin frame. The
  // http control run confirmed that divergence, so both are recorded and only
  // `windowOrigin` bears on whether the frame is opaque.
  check('windowOrigin', function () { return String(window.origin); });
  check('documentOrigin', function () {
    return typeof document.origin === 'undefined' ? '(undefined)' : String(document.origin);
  });
  check('locationOrigin', function () { return String(window.location.origin); });
  check('href', function () { return String(window.location.href); });
  check('protocol', function () { return String(window.location.protocol); });

  // Custom schemes are frequently not secure contexts. Not a security failure
  // in itself, but it gates APIs and is worth knowing before Phase 1 designs
  // around one.
  check('isSecureContext', function () { return window.isSecureContext === true; });

  // --- Isolation from the app origin ---------------------------------------
  // These must all fail. A success is the finding.
  check('parentLocationHref', function () { return String(window.parent.location.href); });
  check('parentDocument', function () { return window.parent.document ? 'reachable' : 'null'; });
  check('topLocationHref', function () { return String(window.top.location.href); });
  check('localStorage', function () { window.localStorage.setItem('f1c', '1'); return 'allowed'; });
  check('sessionStorage', function () { window.sessionStorage.setItem('f1c', '1'); return 'allowed'; });
  // Presence of the object proves nothing; opening it is what an opaque origin
  // refuses.
  check('indexedDB', function () {
    if (!window.indexedDB) return 'absent';
    window.indexedDB.open('f1c');
    return 'open() did not throw';
  });
  check('cookie', function () { document.cookie = 'f1c=1'; return document.cookie || '(empty)'; });

  // --- The IPC bridge must not be here (ADR-F005) --------------------------
  check('tauriBridge', function () {
    var names = ['__TAURI__', '__TAURI_INTERNALS__', '__TAURI_INVOKE__', 'ipc', '__TAURI_IPC__'];
    var found = names.filter(function (n) { return typeof window[n] !== 'undefined'; });
    return found.length ? found : 'none';
  });
  check('webkitMessageHandlers', function () {
    return (window.webkit && window.webkit.messageHandlers)
      ? Object.keys(window.webkit.messageHandlers) : 'absent';
  });

  // --- CSP enforcement ------------------------------------------------------
  // connect-src 'none' should stop all three of these. Under a custom scheme
  // the CSP arrives as a response header from the scheme handler, and whether
  // the engine applies it the same way is exactly what F1c is asking.
  check('fetchAppOrigin', function () {
    // Synchronous answer is impossible; record that it was attempted and let
    // the async result land in `asyncResults` below.
    window.fetch('/__f1c/should-be-blocked').then(function (r) {
      results.fetchAppOrigin = { ok: true, value: 'ALLOWED status ' + r.status };
    }, function (e) {
      results.fetchAppOrigin = { ok: false, threw: e.name, message: 'blocked: ' + e.message };
    });
    return 'pending';
  });
  check('xhr', function () {
    var x = new XMLHttpRequest();
    x.open('GET', '/__f1c/should-be-blocked', false);
    x.send();
    return 'ALLOWED status ' + x.status;
  });
  // The constructor not throwing is not the same as the connection being
  // permitted — Chromium allows construction and fails later, WebKit throws at
  // once. Resolve it on the actual outcome instead of on the constructor.
  check('webSocket', function () {
    var ws = new WebSocket('ws://127.0.0.1:7201/');
    results.webSocket = { ok: true, value: 'constructed, awaiting outcome' };
    ws.onerror = function () {
      results.webSocket = { ok: false, threw: 'error', message: 'connection refused or blocked' };
    };
    ws.onopen = function () {
      results.webSocket = { ok: true, value: 'CONNECTION OPENED' };
      ws.close();
    };
    return 'constructed, awaiting outcome';
  });

  // Inline script must not run: script-src has no 'unsafe-inline'. If the
  // sentinel flips, CSP is not being applied to this document.
  check('inlineScriptBlocked', function () {
    window.__f1cInline = false;
    var s = document.createElement('script');
    s.textContent = 'window.__f1cInline = true;';
    document.head.appendChild(s);
    return window.__f1cInline ? 'INLINE SCRIPT RAN' : 'blocked';
  });

  // --- Sandbox attribute enforcement ---------------------------------------
  check('topNavigation', function () {
    // Without allow-top-navigation this must throw or be ignored.
    var before = String(window.top.location.href);
    window.top.location.href = 'about:blank#f1c';
    return 'attempted from ' + before;
  });
  check('windowOpen', function () {
    var w = window.open('about:blank', '_blank');
    if (w) { w.close(); return 'ALLOWED'; }
    return 'blocked (null)';
  });
  // submit() does not throw when the sandbox blocks it; the observable is
  // whether the frame navigated away. If it did, this probe never reports at
  // all — so "still here" is the pass and the value records only that much.
  check('formSubmitNavigated', function () {
    var before = String(location.href);
    var f = document.createElement('form');
    f.method = 'GET';
    f.action = 'about:blank';
    document.body.appendChild(f);
    try { f.submit(); } catch (e) { return 'threw: ' + e.name; }
    return String(location.href) === before ? 'no navigation' : 'NAVIGATED';
  });

  // --- Resource resolution (ADR-F007's fidelity claim) ---------------------
  // Relative hrefs, @font-face, and stylesheet imports have to resolve
  // naturally under the scheme. This is the *reason* for the scheme, not a
  // side effect, so a failure here is an ADR-F007 problem rather than a
  // security one.
  check('relativeStylesheetApplied', function () {
    var el = document.getElementById('css-probe');
    if (!el) return 'probe element missing';
    var colour = window.getComputedStyle(el).color;
    // The stylesheet sets this to rgb(0, 128, 0); anything else means the
    // relative href did not resolve.
    return colour;
  });
  // Read on load/error, never synchronously. `complete` is false for an image
  // that is merely still in flight, so a synchronous check reports "not loaded"
  // for a perfectly permitted resource — which is exactly the artifact that
  // produced the retracted ordering finding. Timing is not policy.
  check('relativeImageLoaded', function () {
    var img = document.getElementById('img-probe');
    if (!img) return 'probe element missing';
    if (img.complete) {
      return img.naturalWidth > 0 ? 'loaded ' + img.naturalWidth + 'px' : 'NOT loaded';
    }
    results.relativeImageLoaded = { ok: true, value: 'pending' };
    img.addEventListener('load', function () {
      results.relativeImageLoaded = { ok: true, value: 'loaded ' + img.naturalWidth + 'px' };
    });
    img.addEventListener('error', function () {
      results.relativeImageLoaded = { ok: false, value: 'NOT loaded' };
    });
    return 'pending';
  });

  check('userAgent', function () { return navigator.userAgent; });

  // --- Report ---------------------------------------------------------------
  function payload() {
    return {
      f1c: 'probe-result',
      transport: (document.documentElement.getAttribute('data-transport') || 'unknown'),
      results: results,
    };
  }

  function report() {
    // Primary channel. If the frame cannot postMessage out, the host learns
    // nothing — so the fallback below renders the same JSON on screen.
    try {
      if (window.parent && window.parent !== window) {
        window.parent.postMessage(payload(), '*');
      }
    } catch (err) {
      results.postMessageOut = { ok: false, threw: err.name, message: err.message };
    }
    var pre = document.getElementById('out');
    if (pre) pre.textContent = JSON.stringify(payload(), null, 2);
  }

  // The async CSP checks need a beat to settle before the report goes out.
  window.addEventListener('load', function () { setTimeout(report, 600); });
}());
