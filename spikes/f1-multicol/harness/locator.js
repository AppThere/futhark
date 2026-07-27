// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Locators for Spike F1: EPUB CFI paths and quote anchors (ADR-F014, ADR-F015).
//
// F1 asks whether reading position survives a resize or a font-size change. That
// question is only answerable if the locator itself is trustworthy, so every
// sample is anchored three independent ways — DOM id, CFI, and quote match — and
// the harness reports all three. If they disagree, the locator is the suspect and
// the engine is not; if they agree, the number means what it says.
//
// Scope note: this implements the CFI *path* grammar (even element steps, odd
// character-data steps, character offset). Assertions, ranges, and the
// step-indirection `!` are out of scope for the spike — one spine document at a
// time is loaded, so there is nothing to indirect through.

(function (global) {
  'use strict';

  var ELEMENT = 1;
  var TEXT = 3;
  var CDATA = 4;

  function isTextish(n) {
    return n.nodeType === TEXT || n.nodeType === CDATA;
  }

  /** Assign CFI step numbers across a parent's children: elements even, text odd. */
  function stepsOf(parent) {
    var out = [];
    var counter = 0;
    var runOpen = false;
    for (var i = 0; i < parent.childNodes.length; i += 1) {
      var child = parent.childNodes[i];
      if (child.nodeType === ELEMENT) {
        counter = counter % 2 === 0 ? counter + 2 : counter + 1;
        runOpen = false;
        out.push({ node: child, step: counter, kind: 'element' });
      } else if (isTextish(child)) {
        if (!runOpen) {
          counter = counter % 2 === 0 ? counter + 1 : counter;
          runOpen = true;
          out.push({ node: child, step: counter, kind: 'text', run: [child] });
        } else {
          out[out.length - 1].run.push(child);
        }
      }
    }
    return out;
  }

  function stepFor(parent, node) {
    var entries = stepsOf(parent);
    for (var i = 0; i < entries.length; i += 1) {
      var e = entries[i];
      if (e.kind === 'element' && e.node === node) return { step: e.step, before: 0 };
      if (e.kind === 'text') {
        var before = 0;
        for (var j = 0; j < e.run.length; j += 1) {
          if (e.run[j] === node) return { step: e.step, before: before };
          before += e.run[j].data.length;
        }
      }
    }
    return null;
  }

  /** Build a CFI path string for (textNode, offset), rooted at documentElement. */
  function toCfi(node, offset) {
    var steps = [];
    var cur = node;
    var off = offset;
    while (cur && cur !== document.documentElement) {
      var parent = cur.parentNode;
      if (!parent) return null;
      var s = stepFor(parent, cur);
      if (!s) return null;
      if (isTextish(cur)) off += s.before;
      steps.unshift(s.step);
      cur = parent;
    }
    if (!steps.length) return null;
    var tail = isTextish(node) ? ':' + off : '';
    return '/2' + steps.map(function (n) { return '/' + n; }).join('') + tail;
  }

  /** Resolve a CFI path string back to { node, offset }. */
  function fromCfi(cfi) {
    if (typeof cfi !== 'string' || cfi.charAt(0) !== '/') return null;
    var offset = 0;
    var body = cfi;
    var colon = cfi.lastIndexOf(':');
    if (colon > -1) {
      offset = parseInt(cfi.slice(colon + 1), 10);
      body = cfi.slice(0, colon);
      if (!isFinite(offset)) return null;
    }
    var parts = body.split('/').filter(function (p) { return p.length; });
    if (!parts.length || parts[0] !== '2') return null;

    var cur = document.documentElement;
    for (var i = 1; i < parts.length; i += 1) {
      var want = parseInt(parts[i], 10);
      if (!isFinite(want)) return null;
      var entries = stepsOf(cur);
      var hit = null;
      for (var j = 0; j < entries.length; j += 1) {
        if (entries[j].step === want) { hit = entries[j]; break; }
      }
      if (!hit) return null;
      if (hit.kind === 'element') {
        cur = hit.node;
      } else {
        // Character-data step: walk the merged run to place the offset.
        var rest = offset;
        for (var k = 0; k < hit.run.length; k += 1) {
          var len = hit.run[k].data.length;
          if (rest <= len || k === hit.run.length - 1) {
            return { node: hit.run[k], offset: Math.max(0, Math.min(rest, len)) };
          }
          rest -= len;
        }
        return null;
      }
    }
    return { node: cur, offset: 0 };
  }

  var QUOTE_EXACT = 48;
  var QUOTE_CONTEXT = 24;

  function docText() {
    return document.body ? document.body.textContent || '' : '';
  }

  /** Absolute character offset of (node, offset) within body.textContent. */
  function absoluteOffset(node, offset) {
    if (!document.body) return -1;
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var seen = 0;
    var n;
    while ((n = walker.nextNode())) {
      if (n === node) return seen + offset;
      seen += n.data.length;
    }
    return -1;
  }

  /** Inverse of absoluteOffset. */
  function nodeAt(abs) {
    if (!document.body) return null;
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null);
    var seen = 0;
    var n;
    var last = null;
    while ((n = walker.nextNode())) {
      last = n;
      if (abs < seen + n.data.length) return { node: n, offset: abs - seen };
      seen += n.data.length;
    }
    return last ? { node: last, offset: last.data.length } : null;
  }

  /** Quote anchor: prefix / exact / suffix, per ADR-F015 re-anchoring. */
  function toQuote(node, offset) {
    var abs = absoluteOffset(node, offset);
    if (abs < 0) return null;
    var text = docText();
    return {
      prefix: text.slice(Math.max(0, abs - QUOTE_CONTEXT), abs),
      exact: text.slice(abs, abs + QUOTE_EXACT),
      suffix: text.slice(abs + QUOTE_EXACT, abs + QUOTE_EXACT + QUOTE_CONTEXT),
      hint: abs,
    };
  }

  /** Resolve a quote anchor. Exact match first, then nearest occurrence of `exact`. */
  function fromQuote(q) {
    if (!q || typeof q.exact !== 'string' || !q.exact.length) return null;
    var text = docText();
    var full = (q.prefix || '') + q.exact;
    var at = text.indexOf(full);
    if (at > -1) return nodeAt(at + (q.prefix || '').length);

    var best = -1;
    var bestDist = Infinity;
    var from = 0;
    for (;;) {
      var idx = text.indexOf(q.exact, from);
      if (idx < 0) break;
      var d = Math.abs(idx - (q.hint || 0));
      if (d < bestDist) { bestDist = d; best = idx; }
      from = idx + 1;
    }
    return best > -1 ? nodeAt(best) : null;
  }

  global.F1Locator = {
    toCfi: toCfi,
    fromCfi: fromCfi,
    toQuote: toQuote,
    fromQuote: fromQuote,
    absoluteOffset: absoluteOffset,
    nodeAt: nodeAt,
    docText: docText,
  };
}(window));
