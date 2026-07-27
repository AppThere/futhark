// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Loads the CSP probe under one policy. The policy travels in the query so the
// runner can sweep dozens without restarting anything; the server echoes it as
// the document's Content-Security-Policy header verbatim.
//
// The sandboxed iframe is not incidental. The img-src finding only appears in an
// opaque-origin frame, so the R24 sweep has to run under the same conditions or
// it is testing a different thing.

(function () {
  'use strict';
  var frame = document.getElementById('content');
  var out = document.getElementById('out');

  // Results are keyed by the nonce that requested them. Without that, a message
  // from a previous policy can satisfy the wait for the next one — which is
  // exactly the race that made the first sweep report a WebKit fail-open that
  // was not there. A stale result is worse than no result.
  window.__F1C_CSP_RESULT = null;
  window.addEventListener('message', function (ev) {
    if (!ev.data || ev.data.f1c !== 'csp-matrix-result') return;
    window.__F1C_CSP_RESULT = { nonce: ev.data.nonce, results: ev.data.results };
    out.textContent = JSON.stringify(window.__F1C_CSP_RESULT, null, 2);
  });

  // Deliberately no initial load: the frame stays blank until the runner asks
  // for a specific policy.
}());
