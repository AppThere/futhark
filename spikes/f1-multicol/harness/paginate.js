// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// The Spike F1 paginator. Runs *inside* the content document, on the content
// origin, in a sandboxed iframe with no `allow-same-origin` — so the host page
// cannot reach in and this script cannot reach out. Everything the harness knows
// about the layout arrives over postMessage.
//
// That is not an artefact of the test rig. It is ADR-F005 as executable code: if
// the reader's own pagination cannot be driven across an opaque-origin boundary,
// the security model and the pagination model are in conflict, and F1 needs to
// say so rather than quietly grant same-origin to make the measurement easy.

(function () {
  'use strict';

  var EPS = 1; // sub-pixel slack when binning a rect into a column

  var state = {
    stride: 0,
    pageCount: 1,
    scrollerOk: null,
    settings: null,
  };

  function root() {
    return document.documentElement;
  }

  function applySettings(s) {
    var st = root().style;
    st.setProperty('--f1-page-w', s.pageWidth + 'px');
    st.setProperty('--f1-page-h', s.pageHeight + 'px');
    st.setProperty('--f1-margin-h', s.marginH + 'px');
    st.setProperty('--f1-margin-v', s.marginV + 'px');
    st.setProperty('--f1-font-size', s.fontSize + 'px');
    st.setProperty('--f1-line-height', String(s.lineHeight));
    st.setProperty('--f1-font-family', s.fontFamily);
    state.settings = s;
  }

  /** Force layout, then read the geometry the pager depends on. */
  function measure() {
    var r = root();
    var t0 = (window.performance && performance.now) ? performance.now() : 0;
    // Reading offsetHeight flushes pending layout in every engine that matters.
    void r.offsetHeight;
    void r.scrollWidth;
    var layoutMs = ((window.performance && performance.now) ? performance.now() : 0) - t0;
    var viewportW = r.clientWidth;
    var stride = viewportW; // by construction: column-width + column-gap == viewport
    var scrollWidth = r.scrollWidth;
    var derived = stride > 0 ? Math.round(scrollWidth / stride) : 1;
    state.stride = stride;
    state.pageCount = Math.max(1, derived);
    return {
      pageCount: state.pageCount,
      stride: stride,
      scrollWidth: scrollWidth,
      bodyScrollWidth: document.body ? document.body.scrollWidth : 0,
      viewportWidth: viewportW,
      viewportHeight: r.clientHeight,
      remainder: stride > 0 ? scrollWidth % stride : 0,
      layoutMs: Math.round(layoutMs * 100) / 100,
      devicePixelRatio: window.devicePixelRatio || 1,
    };
  }

  function scrollLeft() {
    return root().scrollLeft || document.body.scrollLeft || 0;
  }

  function gotoPage(n) {
    var target = Math.max(0, Math.min(state.pageCount - 1, n | 0)) * state.stride;
    root().scrollLeft = target;
    if (Math.abs(scrollLeft() - target) > 1) document.body.scrollLeft = target;
    return { requested: n, scrollLeft: scrollLeft(), landedOn: currentPage() };
  }

  function currentPage() {
    return state.stride > 0 ? Math.round(scrollLeft() / state.stride) : 0;
  }

  /** Programmatic scrolling has to work even though overflow is hidden. */
  function probeScroller() {
    var before = scrollLeft();
    root().scrollLeft = state.stride;
    var moved = Math.abs(scrollLeft() - state.stride) <= 1;
    root().scrollLeft = before;
    state.scrollerOk = moved;
    return moved;
  }

  function pageOfRect(rect) {
    if (!rect || (rect.width === 0 && rect.height === 0)) return -1;
    if (state.stride <= 0) return -1;
    return Math.floor((rect.left + scrollLeft() + EPS) / state.stride);
  }

  function rangeFor(node, offset, len) {
    var r = document.createRange();
    var max = node.data ? node.data.length : 0;
    var start = Math.max(0, Math.min(offset, max));
    var end = Math.max(start, Math.min(start + (len || 1), max));
    r.setStart(node, start);
    r.setEnd(node, end);
    return r;
  }

  function pageOfPosition(node, offset) {
    if (!node) return -1;
    var r = rangeFor(node, offset, 1);
    var rects = r.getClientRects();
    var rect = rects.length ? rects[0] : r.getBoundingClientRect();
    return pageOfRect(rect);
  }

  /** First text position whose first rect lands on `page`. */
  function locatorAt(page) {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var n;
    while ((n = walker.nextNode())) {
      if (!/\S/.test(n.data)) continue;
      var lo = 0;
      var hi = n.data.length - 1;
      if (pageOfPosition(n, hi) < page) continue;
      // Binary search the first offset in this node that reaches `page`.
      while (lo < hi) {
        var mid = (lo + hi) >> 1;
        if (pageOfPosition(n, mid) >= page) hi = mid; else lo = mid + 1;
      }
      if (pageOfPosition(n, lo) !== page) continue;
      var el = n.parentElement;
      while (el && !el.id) el = el.parentElement;
      return {
        page: page,
        domId: el ? el.id : null,
        cfi: window.F1Locator.toCfi(n, lo),
        quote: window.F1Locator.toQuote(n, lo),
        text: n.data.slice(lo, lo + 40),
      };
    }
    return null;
  }

  /** Resolve a locator through all three channels and report each separately. */
  function pageForLocator(loc) {
    var out = { byId: -1, byCfi: -1, byQuote: -1, visibleText: null };

    if (loc.domId) {
      var el = document.getElementById(loc.domId);
      if (el) out.byId = pageOfRect(el.getBoundingClientRect());
    }
    if (loc.cfi) {
      var c = window.F1Locator.fromCfi(loc.cfi);
      if (c && c.node) {
        out.byCfi = pageOfPosition(c.node, c.offset);
        out.visibleText = (c.node.data || '').slice(c.offset, c.offset + 40);
      }
    }
    if (loc.quote) {
      var q = window.F1Locator.fromQuote(loc.quote);
      if (q && q.node) out.byQuote = pageOfPosition(q.node, q.offset);
    }
    return out;
  }

  /**
   * Content-loss check. Multicol's characteristic failure is not a wrong page
   * count, it is text that lands past the last column and is never reachable.
   */
  function coverage() {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var n;
    var total = 0;
    var unrendered = 0;
    var unrenderedTags = {};
    var beyond = 0;
    var maxPage = -1;
    var overflowing = 0;
    var sl = scrollLeft();
    while ((n = walker.nextNode())) {
      if (!/\S/.test(n.data)) continue;
      total += 1;
      var rects = document.createRange();
      rects.selectNodeContents(n);
      var list = rects.getClientRects();
      if (!list.length) {
        // Not necessarily a defect: <rp> fallbacks are display:none in any
        // engine that actually implements ruby. The tag breakdown is what tells
        // a lost paragraph apart from a correctly hidden one.
        unrendered += 1;
        var tag = n.parentElement ? n.parentElement.tagName.toLowerCase() : '#none';
        unrenderedTags[tag] = (unrenderedTags[tag] || 0) + 1;
        continue;
      }
      for (var i = 0; i < list.length; i += 1) {
        var p = Math.floor((list[i].left + sl + EPS) / state.stride);
        if (p > maxPage) maxPage = p;
        if (p >= state.pageCount) beyond += 1;
        // Text wider than its column box has run off the page edge.
        var colLeft = p * state.stride - sl;
        if (list[i].right > colLeft + state.stride + EPS) overflowing += 1;
      }
    }
    return {
      textNodes: total,
      unrendered: unrendered,
      unrenderedTags: unrenderedTags,
      rectsBeyondLastPage: beyond,
      rectsOverflowingColumn: overflowing,
      maxPageSeen: maxPage,
      pageCount: state.pageCount,
      // The invariant that matters: every rendered rect is reachable by paging.
      allTextReachable: beyond === 0 && maxPage === state.pageCount - 1,
    };
  }

  function visibleTextOnPage(page) {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var n;
    var acc = [];
    var sl = scrollLeft();
    while ((n = walker.nextNode())) {
      if (!/\S/.test(n.data)) continue;
      var r = document.createRange();
      r.selectNodeContents(n);
      var list = r.getClientRects();
      for (var i = 0; i < list.length; i += 1) {
        if (Math.floor((list[i].left + sl + EPS) / state.stride) === page) {
          acc.push(n.data);
          break;
        }
      }
    }
    return acc.join(' ').replace(/\s+/g, ' ').trim();
  }

  function describeSelection(expected) {
    var sel = window.getSelection();
    if (!sel || sel.rangeCount === 0) {
      return { ok: false, reason: 'no-range', rangeCount: sel ? sel.rangeCount : -1 };
    }
    var r = sel.getRangeAt(0);
    var rects = r.getClientRects();
    var pages = {};
    for (var i = 0; i < rects.length; i += 1) {
      pages[Math.floor((rects[i].left + scrollLeft() + EPS) / state.stride)] = true;
    }
    var text = sel.toString();
    var loc = window.F1Locator.toCfi(r.startContainer, r.startOffset);
    var back = loc ? window.F1Locator.fromCfi(loc) : null;
    return {
      ok: text.length > 0 && rects.length > 0,
      text: text.slice(0, 120),
      length: text.length,
      rangeCount: sel.rangeCount,
      rectCount: rects.length,
      pagesSpanned: Object.keys(pages).map(Number).sort(function (a, b) { return a - b; }),
      collapsed: r.collapsed,
      roundTrip: !!(back && back.node === r.startContainer && back.offset === r.startOffset),
      matchesExpected: expected == null ? null : text.replace(/\s+/g, ' ').trim()
        === String(expected).replace(/\s+/g, ' ').trim(),
    };
  }

  /** Programmatic selection between two locators, both resolved by CFI. */
  function selectBetween(a, b) {
    var s = window.F1Locator.fromCfi(a);
    var e = window.F1Locator.fromCfi(b);
    if (!s || !e) return { ok: false, reason: 'unresolvable-locator' };
    var r = document.createRange();
    try {
      r.setStart(s.node, s.offset);
      r.setEnd(e.node, e.offset);
    } catch (err) {
      return { ok: false, reason: 'range-error: ' + err.message };
    }
    var expected = r.toString();
    var sel = window.getSelection();
    sel.removeAllRanges();
    sel.addRange(r);
    return describeSelection(expected);
  }

  function clearSelection() {
    var sel = window.getSelection();
    if (sel) sel.removeAllRanges();
    return true;
  }

  /** What the sandbox actually denies us, measured rather than assumed. */
  function sandboxProbe() {
    var out = { origin: null, sameOriginWithParent: null, storage: null, hasTauriBridge: null };
    try { out.origin = String(window.location.origin); } catch (e) { out.origin = 'throws: ' + e.name; }
    try {
      void window.parent.location.href;
      out.sameOriginWithParent = true;
    } catch (e) {
      out.sameOriginWithParent = false;
    }
    try {
      window.localStorage.setItem('f1', '1');
      out.storage = 'allowed';
    } catch (e) {
      out.storage = 'denied: ' + e.name;
    }
    out.hasTauriBridge = !!(window.__TAURI__ || window.__TAURI_INTERNALS__ || window.ipc);
    out.userAgent = navigator.userAgent;
    out.supportsColumnWidth = window.CSS && CSS.supports('column-width', '400px');
    out.supportsBreakInside = window.CSS && CSS.supports('break-inside', 'avoid-column');
    return out;
  }

  var METHODS = {
    apply: function (p) {
      applySettings(p.settings);
      var m = measure();
      m.scrollerOk = probeScroller();
      gotoPage(0);
      return m;
    },
    measure: function () { return measure(); },
    goto: function (p) { return gotoPage(p.page); },
    currentPage: function () { return { page: currentPage(), scrollLeft: scrollLeft() }; },
    locatorAt: function (p) { return locatorAt(p.page); },
    pageForLocator: function (p) { return pageForLocator(p.locator); },
    coverage: function () { return coverage(); },
    textOnPage: function (p) { return { page: p.page, text: visibleTextOnPage(p.page) }; },
    selectBetween: function (p) { return selectBetween(p.from, p.to); },
    selectionInfo: function () { return describeSelection(null); },
    clearSelection: function () { return clearSelection(); },
    sandboxProbe: function () { return sandboxProbe(); },
    docStats: function () {
      var t = window.F1Locator.docText();
      return { chars: t.length, words: t.split(/\s+/).filter(Boolean).length };
    },
  };

  window.addEventListener('message', function (ev) {
    var msg = ev.data;
    if (!msg || msg.f1 !== 'call') return;
    var reply = { f1: 'result', id: msg.id };
    try {
      var fn = METHODS[msg.method];
      if (!fn) throw new Error('unknown method ' + msg.method);
      reply.ok = true;
      reply.value = fn(msg.params || {});
    } catch (err) {
      reply.ok = false;
      reply.error = err && err.message ? err.message : String(err);
    }
    // targetOrigin '*' because this document has an opaque origin and cannot
    // name its parent; the host validates the message shape instead.
    ev.source.postMessage(reply, '*');
  });

  function announce() {
    if (window.parent && window.parent !== window) {
      window.parent.postMessage({ f1: 'ready', href: String(location.pathname) }, '*');
    }
  }

  if (document.readyState === 'complete') announce();
  else window.addEventListener('load', announce);
}());
