// SPDX-FileCopyrightText: 2026 Kevin Carlson
// SPDX-License-Identifier: Apache-2.0
//
// Generates the Spike F1 corpus: a structurally real EPUB 3 OCF tree.
//
// The book is synthetic on purpose. D7 (corpus sourcing and licensing) is open,
// so nothing redistributable-but-unlicensed goes in the repo. What F1 measures is
// webview multicol behaviour, and the webview only ever sees XHTML + CSS — so
// synthetic prose with a realistic word-length distribution exercises the same
// layout paths a real novel does. The stress chapters are where the real risk
// lives (floats, tables, vertical-rl, ruby, MathML, unbreakable runs), and those
// are hand-written, not generated.
//
// Output is the unpacked OCF tree, not a .epub zip. That is deliberate: ADR-F007
// serves book resources through a URI scheme handler backed by the unpacked
// container anyway, so the tree is the more faithful input. Zip container
// handling belongs to Spike F2.

import { mkdir, writeFile, rm } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const OUT = join(HERE, 'book');

/** Deterministic xorshift32. Same corpus on every machine, every run. */
function rng(seed) {
  let s = seed >>> 0 || 0x9e3779b9;
  return () => {
    s ^= s << 13; s >>>= 0;
    s ^= s >>> 17;
    s ^= s << 5; s >>>= 0;
    return s / 0x100000000;
  };
}

// Word-length distribution matters more than the words themselves: line-breaking
// and hyphenation behaviour is what differs between engines.
const WORDS = `the of and to in a is that it for on with as was at by an be this from or have not are but had his they which one you were her all she there would their we him been has when who will more no if out so said what up its about into than them can only other new some could time these two may then do first any my now such like our over man me even most made after also did many before must through back years where much your way well down should because each just those people mr how too little state good very make world still own see men work long get here between both life being under never day same another know while last might us great old year off come since against go came right used take three form
lantern harbour mercy cipher orchard ledger vellum tremor cathedral quarrel salt marsh threshold lantern glass iron ember archive tide willow granite fathom compass murmur bramble hollow ash reckoning`
  .split(/\s+/).filter(Boolean);

const PUNCT = ['.', '.', '.', '.', ',', ',', ';', '?', '!', '—'];

function sentence(rand) {
  const n = 6 + Math.floor(rand() * 18);
  const w = [];
  for (let i = 0; i < n; i += 1) w.push(WORDS[Math.floor(rand() * WORDS.length)]);
  w[0] = w[0][0].toUpperCase() + w[0].slice(1);
  const end = PUNCT[Math.floor(rand() * PUNCT.length)];
  return w.join(' ') + (end === '—' ? '.' : end);
}

function paragraph(rand) {
  const n = 3 + Math.floor(rand() * 7);
  const s = [];
  for (let i = 0; i < n; i += 1) s.push(sentence(rand));
  return s.join(' ');
}

const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

function xhtml(title, body) {
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head>
<meta charset="UTF-8"/>
<title>${esc(title)}</title>
<link rel="stylesheet" type="text/css" href="../css/book.css"/>
</head>
<body>
${body}
</body>
</html>
`;
}

/** A cover-ish SVG. Inline drawing, no binary assets, no licensing question. */
function figure(rand, n) {
  const hue = Math.floor(rand() * 360);
  return `<figure class="plate">
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 260" width="400" height="260" role="img" aria-label="Plate ${n}">
  <rect width="400" height="260" fill="hsl(${hue} 30% 88%)"/>
  <circle cx="${120 + Math.floor(rand() * 160)}" cy="130" r="${40 + Math.floor(rand() * 50)}" fill="hsl(${(hue + 40) % 360} 45% 55%)"/>
  <path d="M0 220 L120 150 L240 205 L400 120 L400 260 L0 260 Z" fill="hsl(${(hue + 200) % 360} 35% 45%)"/>
</svg>
<figcaption>Plate ${n}. ${esc(sentence(rand))}</figcaption>
</figure>`;
}

function proseChapter(index, wordTarget, rand) {
  const parts = [`<section epub:type="chapter" id="ch${index}">`,
    `<h1 id="ch${index}-title">Chapter ${index}</h1>`];
  let words = 0;
  let para = 0;
  while (words < wordTarget) {
    const roll = rand();
    if (roll < 0.035 && para > 2) {
      parts.push(`<h2 id="ch${index}-s${para}">${esc(sentence(rand).replace(/[.!?]$/, ''))}</h2>`);
    } else if (roll < 0.06) {
      parts.push(`<blockquote><p>${esc(paragraph(rand))}</p></blockquote>`);
    } else if (roll < 0.08) {
      parts.push(figure(rand, `${index}.${para}`));
    } else if (roll < 0.10) {
      const items = Array.from({ length: 3 + Math.floor(rand() * 4) },
        () => `<li>${esc(sentence(rand))}</li>`).join('\n');
      parts.push(`<ul>\n${items}\n</ul>`);
    } else {
      const text = paragraph(rand);
      words += text.split(' ').length;
      const marked = rand() < 0.12
        ? `${esc(text)} <a epub:type="noteref" href="#fn${index}-${para}" id="ref${index}-${para}">${para}</a>`
        : esc(text);
      parts.push(`<p id="p${index}-${para}">${marked}</p>`);
    }
    para += 1;
  }
  parts.push(`<aside epub:type="footnote" id="fn${index}-0"><p>${esc(sentence(rand))}</p></aside>`);
  parts.push('</section>');
  return xhtml(`Chapter ${index}`, parts.join('\n'));
}

// --- Stress chapters. These are the ones that break multicol, not the prose. ---

function floatsChapter(rand) {
  const blocks = [];
  for (let i = 0; i < 40; i += 1) {
    const side = i % 2 ? 'right' : 'left';
    blocks.push(`<div class="float-${side}">${figure(rand, `F.${i}`)}</div>`);
    blocks.push(`<p>${esc(paragraph(rand))}</p>`);
    blocks.push(`<p>${esc(paragraph(rand))}</p>`);
  }
  return xhtml('Floats', `<section epub:type="chapter" id="floats"><h1>Floats</h1>
<p>Floated figures interleaved with body text. Floats fragment badly in some
multicol implementations; a float that escapes its column is the failure mode.</p>
${blocks.join('\n')}</section>`);
}

function tablesChapter(rand) {
  const tables = [];
  for (let t = 0; t < 12; t += 1) {
    const rows = Array.from({ length: 6 + Math.floor(rand() * 25) }, (_, r) =>
      `<tr><td>${r + 1}</td><td>${esc(WORDS[Math.floor(rand() * WORDS.length)])}</td>` +
      `<td>${esc(sentence(rand))}</td><td>${Math.floor(rand() * 10000)}</td></tr>`).join('\n');
    tables.push(`<table><caption>Table ${t + 1}</caption>
<thead><tr><th>#</th><th>Key</th><th>Note</th><th>Value</th></tr></thead>
<tbody>\n${rows}\n</tbody></table>
<p>${esc(paragraph(rand))}</p>`);
  }
  return xhtml('Tables', `<section epub:type="chapter" id="tables"><h1>Tables</h1>
${tables.join('\n')}</section>`);
}

// Two CJK chapters, identical content, differing only in writing-mode. Without
// the horizontal control there is no way to tell a CJK problem from a
// vertical-writing-mode problem, and those have very different consequences:
// most Chinese and modern Japanese ebooks are horizontal.
function cjkChapter(rand, vertical) {
  const HANZI = '春夜洛城聞笛誰家玉笛暗飛聲散入東風滿洛城此夜曲中聞折柳何人不起故園情';
  const line = (n) => Array.from({ length: n }, () => HANZI[Math.floor(rand() * HANZI.length)]).join('');
  const paras = Array.from({ length: 60 }, () =>
    `<p>${line(60 + Math.floor(rand() * 120))}</p>`).join('\n');
  const ruby = Array.from({ length: 20 }, () =>
    `<p><ruby>${line(2)}<rp>(</rp><rt>ふりがな</rt><rp>)</rp></ruby>${line(40)}</p>`).join('\n');
  const cls = vertical ? ' class="vertical"' : '';
  const heading = vertical ? '縦書きとルビ' : '横書きとルビ';
  return xhtml(vertical ? 'CJK vertical' : 'CJK horizontal',
    `<section epub:type="chapter" id="cjk" lang="ja"${cls}>
<h1>${heading}</h1>
<p>R13: ${vertical ? 'vertical' : 'horizontal'} writing mode with ruby annotations.
The two chapters are the same text; any difference between them is the writing
mode and nothing else.</p>
${ruby}
${paras}</section>`);
}

function mathChapter(rand) {
  const eqs = Array.from({ length: 40 }, (_, i) => `<p>${esc(paragraph(rand))}</p>
<math xmlns="http://www.w3.org/1998/Math/MathML" display="block">
  <mrow><msub><mi>x</mi><mn>${i}</mn></msub><mo>=</mo>
  <mfrac><mrow><mo>-</mo><mi>b</mi><mo>&#xB1;</mo>
  <msqrt><mrow><msup><mi>b</mi><mn>2</mn></msup><mo>-</mo>
  <mn>4</mn><mi>a</mi><mi>c</mi></mrow></msqrt></mrow>
  <mrow><mn>2</mn><mi>a</mi></mrow></mfrac></mrow>
</math>`).join('\n');
  return xhtml('Mathematics', `<section epub:type="chapter" id="math"><h1>Mathematics</h1>
${eqs}</section>`);
}

function unbreakableChapter(rand) {
  const runs = Array.from({ length: 30 }, (_, i) => {
    const long = 'supercalifragilistic'.repeat(4 + (i % 6));
    return `<p>${esc(paragraph(rand))}</p>
<p class="run">${long}</p>
<pre><code>fn record_${i}(bytes: &amp;[u8]) -&gt; Result&lt;Record, MobiError&gt; {
    let header = bytes.get(..16).ok_or(MobiError::Truncated)?;
    Ok(Record::parse(header)?)
}</code></pre>`;
  }).join('\n');
  return xhtml('Unbreakable runs', `<section epub:type="chapter" id="runs"><h1>Unbreakable runs</h1>
<p>Overlong words and preformatted blocks wider than the column box. An engine
that lets these overflow the column silently loses text off the page edge.</p>
${runs}</section>`);
}

const CSS = `/* SPDX-FileCopyrightText: 2026 Kevin Carlson */
/* SPDX-License-Identifier: Apache-2.0 */
/* Author stylesheet. Deliberately opinionated: the reader stylesheet has to win
   the pagination properties without stomping the author's typography. */
h1 { font-size: 1.8em; margin: 1.2em 0 0.6em; break-after: avoid; }
h2 { font-size: 1.25em; margin: 1em 0 0.4em; break-after: avoid; }
p { margin: 0 0 0.75em; text-align: justify; hyphens: auto; }
blockquote { margin: 1em 2em; font-style: italic; }
figure { margin: 1em 0; }
figure svg { max-width: 100%; height: auto; }
figcaption { font-size: 0.85em; font-style: italic; }
.float-left figure { float: left; margin: 0 1em 0.5em 0; width: 45%; }
.float-right figure { float: right; margin: 0 0 0.5em 1em; width: 45%; }
table { border-collapse: collapse; width: 100%; margin: 1em 0; font-size: 0.85em; }
th, td { border: 1px solid #999; padding: 0.25em 0.4em; text-align: left; }
pre { background: #f2f2f2; padding: 0.5em; font-size: 0.85em; }
.run { font-family: monospace; }
.vertical { writing-mode: vertical-rl; }
ruby rt { font-size: 0.5em; }
`;

function opf(spine, title) {
  const items = spine.map((s, i) =>
    `    <item id="c${i}" href="${s.href}" media-type="application/xhtml+xml"/>`).join('\n');
  const refs = spine.map((_, i) => `    <itemref idref="c${i}"/>`).join('\n');
  return `<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id"
         xml:lang="en" prefix="cc: http://creativecommons.org/ns#">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="pub-id">urn:uuid:f1-multicol-spike-0000-0000-000000000001</dc:identifier>
    <dc:title>${esc(title)}</dc:title>
    <dc:language>en</dc:language>
    <dc:creator>Futhark Spike F1 corpus generator</dc:creator>
    <dc:rights>Apache-2.0. Synthetic text, generated deterministically.</dc:rights>
    <meta property="dcterms:modified">2026-07-27T00:00:00Z</meta>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="css" href="css/book.css" media-type="text/css"/>
${items}
  </manifest>
  <spine toc="ncx">
${refs}
  </spine>
</package>
`;
}

function nav(spine) {
  const li = spine.map((s) => `      <li><a href="${s.href}">${esc(s.title)}</a></li>`).join('\n');
  return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" lang="en">
<head><meta charset="UTF-8"/><title>Contents</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>Contents</h1>
    <ol>
${li}
    </ol>
  </nav>
</body>
</html>
`;
}

function ncx(spine) {
  const points = spine.map((s, i) => `    <navPoint id="np${i}" playOrder="${i + 1}">
      <navLabel><text>${esc(s.title)}</text></navLabel>
      <content src="${s.href}"/>
    </navPoint>`).join('\n');
  return `<?xml version="1.0" encoding="UTF-8"?>
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head><meta name="dtb:uid" content="urn:uuid:f1-multicol-spike-0000-0000-000000000001"/></head>
  <docTitle><text>The Reckoning of Salt Marsh</text></docTitle>
  <navMap>
${points}
  </navMap>
</ncx>
`;
}

async function main() {
  // Tuned so the T1 baseline tuple lands at ~900 pages, which is the size the
  // spike question names. Every other tuple falls where it falls.
  const wordsPerChapter = Number(process.env.F1_WORDS_PER_CHAPTER ?? 16600);
  const proseChapters = Number(process.env.F1_PROSE_CHAPTERS ?? 26);
  const rand = rng(0x5a17_ce25);

  await rm(OUT, { recursive: true, force: true });
  await mkdir(join(OUT, 'OEBPS', 'text'), { recursive: true });
  await mkdir(join(OUT, 'OEBPS', 'css'), { recursive: true });
  await mkdir(join(OUT, 'META-INF'), { recursive: true });

  const spine = [];
  for (let i = 1; i <= proseChapters; i += 1) {
    const href = `text/ch${String(i).padStart(2, '0')}.xhtml`;
    await writeFile(join(OUT, 'OEBPS', href), proseChapter(i, wordsPerChapter, rand));
    spine.push({ href, title: `Chapter ${i}`, kind: 'prose' });
  }

  const stress = [
    ['text/s-floats.xhtml', 'Floats', floatsChapter(rand)],
    ['text/s-tables.xhtml', 'Tables', tablesChapter(rand)],
    ['text/s-cjk-h.xhtml', 'CJK horizontal and ruby', cjkChapter(rand, false)],
    ['text/s-cjk.xhtml', 'CJK vertical and ruby', cjkChapter(rand, true)],
    ['text/s-math.xhtml', 'Mathematics', mathChapter(rand)],
    ['text/s-runs.xhtml', 'Unbreakable runs', unbreakableChapter(rand)],
  ];
  for (const [href, title, body] of stress) {
    await writeFile(join(OUT, 'OEBPS', href), body);
    spine.push({ href, title, kind: 'stress' });
  }

  await writeFile(join(OUT, 'mimetype'), 'application/epub+zip');
  await writeFile(join(OUT, 'META-INF', 'container.xml'),
    `<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/package.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
`);
  await writeFile(join(OUT, 'OEBPS', 'css', 'book.css'), CSS);
  await writeFile(join(OUT, 'OEBPS', 'package.opf'), opf(spine, 'The Reckoning of Salt Marsh'));
  await writeFile(join(OUT, 'OEBPS', 'nav.xhtml'), nav(spine));
  await writeFile(join(OUT, 'OEBPS', 'toc.ncx'), ncx(spine));
  await writeFile(join(OUT, 'spine.json'), JSON.stringify(spine, null, 2));

  process.stdout.write(`corpus: ${spine.length} spine documents in ${OUT}\n`);
}

await main();
