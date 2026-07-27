// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// R24: does any source ordering make a *restrictive* directive fail open?
//
// The img-src finding tested the permissive direction — a source that should
// have allowed and did not. That fails closed: missing images, bad but safe.
// This probe tests the other direction, which is the one `default-src 'none'`
// depends on as the backstop for the whole content sandbox (ADR-F005).
//
// It reports what loaded. The runner owns the expectations, because a probe
// that knows what it is supposed to find is a probe that can agree with itself.

(function () {
  'use strict';

  var out = {};

  var NONCE = new URLSearchParams(location.search).get('n') || '';

  function done() {
    var payload = { f1c: 'csp-matrix-result', nonce: NONCE, results: out };
    try {
      if (window.parent && window.parent !== window) window.parent.postMessage(payload, '*');
    } catch (err) { /* rendered below regardless */ }
    var pre = document.getElementById('out');
    if (pre) pre.textContent = JSON.stringify(payload, null, 2);
  }

  // --- Script ---------------------------------------------------------------
  out.externalScript = window.__f1cExternalScript === true ? 'allowed' : 'blocked';

  out.inlineScript = (function () {
    window.__f1cInline = false;
    try {
      var s = document.createElement('script');
      s.textContent = 'window.__f1cInline = true;';
      document.head.appendChild(s);
    } catch (err) {
      return 'threw:' + err.name;
    }
    return window.__f1cInline ? 'allowed' : 'blocked';
  }());

  // eval() is governed by script-src too, and is the classic fail-open check.
  out.evalAllowed = (function () {
    try {
      // eslint-disable-next-line no-eval
      return window.eval('1+1') === 2 ? 'allowed' : 'blocked';
    } catch (err) {
      return 'blocked';
    }
  }());

  // --- Style ----------------------------------------------------------------
  out.externalStyle = (function () {
    var el = document.getElementById('css-probe');
    if (!el) return 'no-probe-element';
    return window.getComputedStyle(el).color === 'rgb(0, 128, 0)' ? 'allowed' : 'blocked';
  }());

  out.inlineStyle = (function () {
    var d = document.createElement('div');
    d.setAttribute('style', 'color: rgb(1, 2, 3)');
    document.body.appendChild(d);
    return window.getComputedStyle(d).color === 'rgb(1, 2, 3)' ? 'allowed' : 'blocked';
  }());

  // --- Image ----------------------------------------------------------------
  out.image = (function () {
    var img = document.getElementById('img-probe');
    if (!img) return 'no-probe-element';
    return img.complete && img.naturalWidth > 0 ? 'allowed' : 'blocked';
  }());

  // --- Connect --------------------------------------------------------------
  var pending = 2;
  function settle() { pending -= 1; if (pending <= 0) done(); }

  out.fetch = 'pending';
  try {
    window.fetch('probe.png', { cache: 'no-store' }).then(
      function () { out.fetch = 'allowed'; settle(); },
      function () { out.fetch = 'blocked'; settle(); },
    );
  } catch (err) {
    out.fetch = 'blocked';
    settle();
  }

  out.xhr = (function () {
    try {
      var x = new XMLHttpRequest();
      x.open('GET', 'probe.png', false);
      x.send();
      return x.status === 200 ? 'allowed' : 'blocked';
    } catch (err) {
      return 'blocked';
    }
  }());

  // A late-loading image would otherwise be judged before it settled.
  var img = document.getElementById('img-probe');
  if (img && !img.complete) {
    img.addEventListener('load', function () { out.image = 'allowed'; settle(); });
    img.addEventListener('error', function () { out.image = 'blocked'; settle(); });
  } else {
    settle();
  }

  // Backstop: report even if a listener never fires.
  setTimeout(function () { if (pending > 0) { pending = 0; done(); } }, 2500);
}());
