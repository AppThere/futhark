// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
/*
 * Host side of the harness. This page stands in for the Tauri app origin. It
 * holds no privileged capability of its own, but in production it is the origin
 * that *does* — which is exactly why the content frame is opaque to it.
 *
 * Note what is missing: there is no path from here into the content document.
 * `iframe.contentDocument` is null because the frame is sandboxed without
 * allow-same-origin. Every measurement crosses postMessage, and the driver
 * cannot cheat that boundary either.
 */
(function () {
  'use strict';

  var frame = document.getElementById('content');
  var statusEl = document.getElementById('status');
  var pending = new Map();
  var nextId = 1;
  var readyResolve = null;
  var CONTENT_ORIGIN = new URLSearchParams(location.search).get('content')
    || 'http://127.0.0.1:7102';

  window.addEventListener('message', function (ev) {
    var msg = ev.data;
    if (!msg || typeof msg !== 'object') return;
    if (msg.f1 === 'ready') {
      statusEl.textContent = 'loaded ' + msg.href;
      if (readyResolve) { var r = readyResolve; readyResolve = null; r(msg); }
      return;
    }
    if (msg.f1 === 'result' && pending.has(msg.id)) {
      var entry = pending.get(msg.id);
      pending.delete(msg.id);
      clearTimeout(entry.timer);
      if (msg.ok) entry.resolve(msg.value);
      else entry.reject(new Error(msg.error || 'content error'));
    }
  });

  function call(method, params, timeoutMs) {
    return new Promise(function (resolve, reject) {
      var id = nextId += 1;
      var timer = setTimeout(function () {
        pending.delete(id);
        reject(new Error('timeout calling ' + method));
      }, timeoutMs || 30000);
      pending.set(id, { resolve: resolve, reject: reject, timer: timer });
      frame.contentWindow.postMessage({ f1: 'call', id: id, method: method, params: params || {} }, '*');
    });
  }

  function setViewport(w, h) {
    frame.style.width = w + 'px';
    frame.style.height = h + 'px';
    return { width: w, height: h };
  }

  function load(href, timeoutMs) {
    statusEl.textContent = 'loading ' + href;
    return new Promise(function (resolve, reject) {
      var timer = setTimeout(function () {
        readyResolve = null;
        reject(new Error('timeout loading ' + href));
      }, timeoutMs || 60000);
      readyResolve = function (msg) { clearTimeout(timer); resolve(msg); };
      frame.src = CONTENT_ORIGIN + '/' + String(href).replace(/^\/+/, '');
    });
  }

  /** Confirms the boundary is real rather than assumed (ADR-F005). */
  function hostProbe() {
    var out = { contentDocumentReachable: null, contentWindowLength: null, error: null };
    try {
      out.contentDocumentReachable = frame.contentDocument !== null
        && frame.contentDocument !== undefined;
    } catch (e) {
      out.contentDocumentReachable = false;
      out.error = e.name;
    }
    try { out.contentWindowLength = frame.contentWindow.length; } catch (e) { /* opaque */ }
    out.sandboxAttr = frame.getAttribute('sandbox');
    out.contentOrigin = CONTENT_ORIGIN;
    return out;
  }

  /** Frame geometry in host viewport coordinates, for real pointer input. */
  function frameBox() {
    var r = frame.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height };
  }

  window.F1 = {
    load: load,
    call: call,
    setViewport: setViewport,
    hostProbe: hostProbe,
    frameBox: frameBox,
  };
  statusEl.textContent = 'ready';
}());
