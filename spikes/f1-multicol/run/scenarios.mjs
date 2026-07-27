// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// The measurements. Each scenario maps onto a clause of the F1 pass criteria in
// spec §10, plus R10 (selection across a paginated iframe) which the spec folds
// into F1 explicitly.
//
// Loops live inside the browser rather than in the driver: one round trip per
// document instead of one per property read. That matters for WebDriver, where
// every call is an HTTP request, and it keeps the two engines on the same code.

/** Settings tuples. A "settings tuple" in the pass criteria is one of these. */
export const TUPLES = [
  { id: 'T1-baseline', pageWidth: 800, pageHeight: 1000, marginH: 40, marginV: 40, fontSize: 16, lineHeight: 1.6, fontFamily: 'Georgia, "Times New Roman", serif' },
  { id: 'T2-large-font', pageWidth: 800, pageHeight: 1000, marginH: 40, marginV: 40, fontSize: 24, lineHeight: 1.6, fontFamily: 'Georgia, "Times New Roman", serif' },
  { id: 'T3-small-font', pageWidth: 800, pageHeight: 1000, marginH: 40, marginV: 40, fontSize: 14, lineHeight: 1.4, fontFamily: 'Georgia, "Times New Roman", serif' },
  { id: 'T4-phone', pageWidth: 414, pageHeight: 896, marginH: 20, marginV: 28, fontSize: 16, lineHeight: 1.6, fontFamily: 'Georgia, "Times New Roman", serif' },
  { id: 'T5-wide-sans', pageWidth: 1280, pageHeight: 800, marginH: 64, marginV: 40, fontSize: 16, lineHeight: 1.6, fontFamily: 'Helvetica, Arial, sans-serif' },
  { id: 'T6-sans', pageWidth: 800, pageHeight: 1000, marginH: 40, marginV: 40, fontSize: 16, lineHeight: 1.6, fontFamily: 'Helvetica, Arial, sans-serif' },
];

// ---------------------------------------------------------------------------
// Host-page function sources. These strings are evaluated in the host document,
// which is the only place with a handle on the content frame.
// ---------------------------------------------------------------------------

const LAYOUT_DOC = `async (arg) => {
  const F1 = window.F1;
  F1.setViewport(arg.settings.pageWidth, arg.settings.pageHeight);
  await F1.load(arg.href);
  const metrics = await F1.call('apply', { settings: arg.settings });
  const out = { href: arg.href, metrics };
  if (arg.withCoverage) out.coverage = await F1.call('coverage', {});
  if (arg.withStats) out.stats = await F1.call('docStats', {});
  return out;
}`;

const PROBE = `async (arg) => {
  const F1 = window.F1;
  F1.setViewport(800, 1000);
  await F1.load(arg.href);
  const sandbox = await F1.call('sandboxProbe', {});
  return { host: F1.hostProbe(), content: sandbox, frame: F1.frameBox() };
}`;

const POSITION = `async (arg) => {
  const F1 = window.F1;
  const norm = (s) => String(s || '').replace(/\\s+/g, ' ').trim();

  F1.setViewport(arg.from.pageWidth, arg.from.pageHeight);
  await F1.load(arg.href);
  const before = await F1.call('apply', { settings: arg.from });

  const samples = [];
  const step = Math.max(1, Math.floor(before.pageCount / (arg.sampleCount + 1)));
  for (let i = 1; i <= arg.sampleCount; i += 1) {
    const page = Math.min(before.pageCount - 1, i * step);
    await F1.call('goto', { page });
    const loc = await F1.call('locatorAt', { page });
    if (loc) samples.push({ page, loc });
  }

  // Same document, same DOM, new settings. This is exactly what a font-size
  // change or a window resize does in the real reader.
  const after = await F1.call('apply', { settings: arg.to });
  const results = [];
  for (const s of samples) {
    const resolved = await F1.call('pageForLocator', { locator: s.loc });
    const target = resolved.byCfi >= 0 ? resolved.byCfi
      : (resolved.byId >= 0 ? resolved.byId : resolved.byQuote);
    let visible = null;
    if (target >= 0) {
      await F1.call('goto', { page: target });
      const t = await F1.call('textOnPage', { page: target });
      visible = norm(t.text).includes(norm(s.loc.text).slice(0, 24));
    }
    results.push({
      originalPage: s.page,
      anchorText: s.loc.text,
      resolved,
      restoredPage: target,
      anchorVisibleAfterRestore: visible,
      // CFI and quote anchor the same character, so they must agree exactly.
      cfiQuoteAgree: resolved.byCfi === resolved.byQuote,
      // The DOM id anchors the *containing element*, which can start on an
      // earlier page than the character does. Earlier is fine; later is a bug.
      idConsistent: resolved.byId < 0 || resolved.byId <= resolved.byCfi,
    });
  }

  // Round trip: go back to the original settings and see whether the same
  // locator lands on the page it started on.
  await F1.call('apply', { settings: arg.from });
  const roundTrip = [];
  for (const s of samples) {
    const r = await F1.call('pageForLocator', { locator: s.loc });
    const target = r.byCfi >= 0 ? r.byCfi : r.byId;
    roundTrip.push({ originalPage: s.page, returnedPage: target, drift: target - s.page });
  }

  return { href: arg.href, before, after, results, roundTrip };
}`;

const SELECTION = `async (arg) => {
  const F1 = window.F1;
  F1.setViewport(arg.settings.pageWidth, arg.settings.pageHeight);
  await F1.load(arg.href);
  const metrics = await F1.call('apply', { settings: arg.settings });

  const page = Math.min(arg.page, metrics.pageCount - 1);
  await F1.call('goto', { page });

  const a = await F1.call('locatorAt', { page });
  const b = await F1.call('locatorAt', { page: Math.min(page + 1, metrics.pageCount - 1) });
  const out = { href: arg.href, page, pageCount: metrics.pageCount, cases: {} };

  // (a) within one column
  if (a && a.cfi) {
    const endCfi = a.cfi.replace(/:(\\d+)$/, (m, n) => ':' + (Number(n) + 60));
    await F1.call('clearSelection', {});
    out.cases.withinColumn = await F1.call('selectBetween', { from: a.cfi, to: endCfi });
  }

  // (b) across a column boundary — the R10 case. The end has to be *inside* the
  // next column, not at its first character, or the range never crosses.
  if (a && b && a.cfi && b.cfi && a.cfi !== b.cfi) {
    const deepCfi = b.cfi.replace(/:(\\d+)$/, (m, n) => ':' + (Number(n) + 40));
    await F1.call('clearSelection', {});
    out.cases.acrossColumn = await F1.call('selectBetween', { from: a.cfi, to: deepCfi });
  }

  return out;
}`;

const DRAG_PREPARE = `async (arg) => {
  const F1 = window.F1;
  F1.setViewport(arg.settings.pageWidth, arg.settings.pageHeight);
  await F1.load(arg.href);
  const metrics = await F1.call('apply', { settings: arg.settings });
  const page = Math.min(arg.page, metrics.pageCount - 1);
  await F1.call('goto', { page });
  await F1.call('clearSelection', {});
  return { metrics, page, box: F1.frameBox() };
}`;

const DRAG_READ = `async () => window.F1.call('selectionInfo', {})`;

const MONOTONIC = `async (arg) => {
  const F1 = window.F1;
  const out = [];
  for (const fontSize of arg.sizes) {
    const settings = Object.assign({}, arg.base, { fontSize });
    F1.setViewport(settings.pageWidth, settings.pageHeight);
    await F1.load(arg.href);
    const m = await F1.call('apply', { settings });
    out.push({ fontSize, pageCount: m.pageCount, scrollWidth: m.scrollWidth, remainder: m.remainder });
  }
  return out;
}`;

// ---------------------------------------------------------------------------

export async function probeEnvironment(driver, hostUrl, href) {
  await driver.goto(hostUrl);
  return driver.evalAsync(PROBE, { href });
}

export async function layoutDoc(driver, arg) {
  return driver.evalAsync(LAYOUT_DOC, arg);
}

export async function positionRun(driver, arg) {
  return driver.evalAsync(POSITION, arg);
}

export async function selectionRun(driver, arg) {
  return driver.evalAsync(SELECTION, arg);
}

export async function monotonicRun(driver, arg) {
  return driver.evalAsync(MONOTONIC, arg);
}

/**
 * Real pointer selection. The drag is issued by the driver against the host
 * page, so it crosses the sandbox boundary the same way a user's finger does.
 */
export async function dragSelectionRun(driver, arg) {
  const prep = await driver.evalAsync(DRAG_PREPARE, arg);
  const { box } = prep;
  const inset = arg.settings.marginH + 8;
  const cases = {};

  await driver.drag(
    { x: box.x + inset, y: box.y + arg.settings.marginV + 40 },
    { x: box.x + box.width - inset, y: box.y + box.height - arg.settings.marginV - 40 },
  );
  cases.dragWithinPage = await driver.evalAsync(DRAG_READ, null);

  await driver.evalAsync(`async () => window.F1.call('clearSelection', {})`, null);
  await driver.drag(
    { x: box.x + box.width / 2, y: box.y + box.height / 2 },
    { x: box.x + box.width + 200, y: box.y + box.height / 2 },
  );
  cases.dragPastRightEdge = await driver.evalAsync(DRAG_READ, null);

  return { href: arg.href, page: prep.page, pageCount: prep.metrics.pageCount, cases };
}
