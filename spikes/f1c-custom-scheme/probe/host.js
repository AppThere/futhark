// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// App-origin half of the F1c probe. Loads the content frame, collects what the
// frame reports, and adds the checks only the app side can make.
//
// This page runs under both transports without modification. Under Tauri it
// hands the result to Rust over IPC — which doubles as a demonstration of the
// asymmetry ADR-F005 requires: the app origin *has* the bridge, and the content
// origin must not.

(function () {
  'use strict';

  var params = new URLSearchParams(location.search);
  // Under Tauri the content origin is the custom scheme; over http it is the
  // control server. Nothing else about this page changes.
  var CONTENT = params.get('content')
    || (location.protocol === 'http:' ? 'http://127.0.0.1:7202' : 'futhark-content://localhost');
  var TRANSPORT = params.get('transport')
    || (location.protocol === 'http:' ? 'http' : 'custom-scheme');

  var frame = document.getElementById('content');
  var out = document.getElementById('out');
  document.getElementById('transport').textContent = TRANSPORT;
  document.getElementById('content-url').textContent = CONTENT;

  var report = {
    transport: TRANSPORT,
    contentOrigin: CONTENT,
    appOrigin: String(location.origin),
    appHref: String(location.href),
    userAgent: navigator.userAgent,
    host: {},
    content: null,
    messageOrigin: null,
  };

  /** Checks only the embedder can make. */
  function hostChecks() {
    var h = {};
    try {
      h.contentDocumentReachable = frame.contentDocument !== null
        && frame.contentDocument !== undefined;
    } catch (err) {
      h.contentDocumentReachable = false;
      h.contentDocumentThrew = err.name;
    }
    try {
      h.contentWindowLocation = String(frame.contentWindow.location.href);
    } catch (err) {
      h.contentWindowLocation = 'threw: ' + err.name;
    }
    h.sandboxAttr = frame.getAttribute('sandbox');
    // The app origin is where the bridge belongs. Its absence here would mean
    // the probe is not testing the configuration Futhark ships.
    h.appHasTauriBridge = !!(window.__TAURI__ || window.__TAURI_INTERNALS__);
    h.appIsSecureContext = window.isSecureContext === true;
    return h;
  }

  function finish() {
    report.host = hostChecks();
    out.textContent = JSON.stringify(report, null, 2);
    window.__F1C_RESULT = report;

    // Hand off to Rust when there is a Rust to hand off to.
    var tauri = window.__TAURI__;
    if (tauri && tauri.core && typeof tauri.core.invoke === 'function') {
      tauri.core.invoke('report', { payload: report })
        .then(function (path) { out.textContent += '\n\nwritten to ' + path; },
          function (e) { out.textContent += '\n\nIPC report failed: ' + e; });
    }
  }

  window.addEventListener('message', function (ev) {
    if (!ev.data || ev.data.f1c !== 'probe-result') return;
    // Worth recording rather than asserting: an opaque origin serialises to
    // "null" here, which means the app cannot use event.origin to tell the
    // content frame apart from any other opaque frame. That is a design input
    // for how the real IPC boundary authenticates messages.
    report.messageOrigin = String(ev.origin);
    report.content = ev.data.results;
    finish();
  });

  frame.src = CONTENT.replace(/\/+$/, '') + '/content.xhtml';

  // The content frame may be unable to postMessage out at all. Report what we
  // have rather than hanging; the frame renders its own copy on screen.
  setTimeout(function () {
    if (!report.content) {
      report.content = 'NO MESSAGE RECEIVED — read the JSON rendered inside the frame';
      finish();
    }
  }, 6000);
}());
