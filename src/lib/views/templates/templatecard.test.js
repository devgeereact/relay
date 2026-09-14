// T1 · THE GALLERY CARD'S CAPTION — a layout defect, not a difference of taste.
//
// Rendered at 1600x1000 against the prototype, a card's name came out as `Cla…`,
// `Wo…` and `L.` — three characters of "Classic Serif" — while four things
// competed for the same caption row: the name, a mono kind tag, a second pill
// repeating that kind as a bound content look, and `16:9` printed a second time
// under a badge already saying it on the thumbnail.
//
// The cause is flexbox priority, and it is arithmetic rather than opinion. The
// caption's non-name items declared MORE width than the narrowest column has:
// `.tg-role` claimed up to 42%, `.tg-usedfor` up to 40%, the star and the ⋮ menu
// 45px, three gaps 21px — 225px of claims inside a 194px row. Every one of those
// was `flex:0 0 auto`; the name was the only flexible item, so the name paid the
// whole shortfall and collapsed to an ellipsis.
//
// The prototype's cell is one tidy meta row — name left, kind right — with what
// the template is USED FOR on its own line beneath (`.cell .meta`, `.cell .ttl`,
// `.kd`, `.roletag`). This file holds Relay's version of that as three facts:
//
//   1. the name has a STATED floor, wide enough for every seeded built-in whole;
//   2. the row can honour that floor at the narrowest column the grid admits,
//      priced from the stylesheet's own declarations — and everything sharing
//      the row may ellipse before the name does;
//   3. nothing else is in that row: the used-for line sits beneath it, and the
//      aspect is stated once per card rather than twice.
//
// Every existing suite was blind to this. `qa-inventory` counted the controls and
// found them all present and named; the mount tests read `.tg-name`'s
// textContent, which is the FULL name whatever the box does to it on screen. A
// caption that clips is invisible to a test that reads text.
//
//   npx vitest run src/lib/views/templates/templatecard.test.js
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as svelteRuntime from 'svelte';

// Same self-detecting gate as `surface.test.js` and `screencards.test.js`: with
// `resolve.conditions: ['browser']` tidied out of `vitest.config.js`, `onMount`
// becomes the SSR no-op and every mount below would pass by rendering nothing.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./TemplateGallery.svelte');
const { templates, contentTemplates } = await import('../../stores/capture.js');

const SRC = readFileSync(resolve('src/lib/views/templates/TemplateGallery.svelte'), 'utf8');

// ── the stylesheet, read as declarations ────────────────────────────────────
// The component's own `<style>`, comments stripped. Svelte scopes these at
// compile time; the source spellings are what a reader edits, so they are what
// this file prices.
const CSS = SRC.slice(SRC.lastIndexOf('<style>') + 7, SRC.lastIndexOf('</style>')).replace(
  /\/\*[\s\S]*?\*\//g,
  '',
);

/** Every declaration written against exactly this selector, later winning. */
function declsFor(selector) {
  const out = {};
  const re = /([^{}]+)\{([^{}]*)\}/g;
  let m;
  while ((m = re.exec(CSS))) {
    if (!m[1].split(',').map((s) => s.trim()).includes(selector)) continue;
    for (const d of m[2].split(';')) {
      const i = d.indexOf(':');
      if (i > 0) out[d.slice(0, i).trim()] = d.slice(i + 1).trim();
    }
  }
  return out;
}

const px = (v) => (v && /^-?[\d.]+px$/.test(v.trim()) ? parseFloat(v) : null);

// Design tokens this file prices against, from `src/app.css`. Read, not copied,
// so a retuned type scale reaches the arithmetic instead of silently invalidating
// it — the same reason `check-updater.mjs` reads the endpoint out of the config.
const APPCSS = readFileSync(resolve('src/app.css'), 'utf8');
const token = (name) => {
  const m = new RegExp(`${name}\\s*:\\s*([\\d.]+)px`).exec(APPCSS);
  return m ? parseFloat(m[1]) : null;
};
const FS_B2 = token('--v-fs-b2');

// The per-face advance docs/REBRAND.md §3.4 uses for exactly this kind of
// estimate. Sans is the caption's face.
const SANS_ADVANCE = 0.52;
const textWidth = (s, size) => s.length * size * SANS_ADVANCE;

// The five templates a fresh install ships (`src-tauri/src/db/templates.rs`).
// Three of them are the three the render clipped.
const SEEDED = ['Classic Serif', 'Stage Mono', 'Lower Third', 'Worship Lyrics', 'Lobby Warm'];

// ── the fixtures ────────────────────────────────────────────────────────────
const CLASSIC = {
  id: 1,
  name: 'Classic Serif',
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: { background: '#101018', verseColor: '#f4e4c8', verseSize: '5.5' },
};
const LYRICS = {
  id: 4,
  name: 'Worship Lyrics',
  layout: { regions: ['verse_text'], align: 'center' },
  style: { background: '#0d0d14', verseColor: '#fff', verseSize: '6' },
};
const SUPER = {
  id: 9,
  // The longest role tag in `KIND_META` — SUPERSOURCE — so the row is priced
  // under the worst kind it can be asked to carry, not a convenient one.
  name: 'Camera + Word',
  layout: { layers: [{ id: 'r', type: 'region' }] },
  style: {},
};
const FIXTURE = [CLASSIC, LYRICS, SUPER];

let host;
let cmp;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateGallery({ target: host });
  return cmp;
}
const unmount = () => {
  cmp?.$destroy();
  cmp = null;
  host?.remove();
  host = null;
};
const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => {
  for (let i = 0; i < n; i++) await settle();
};

const cards = () => [...host.querySelectorAll('.tg-card')];
const cardFor = (name) =>
  cards().find((c) => c.querySelector('.tg-name')?.textContent.trim() === name);

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_templates') return structuredClone(FIXTURE);
    if (cmd === 'list_output_channels') return [];
    return null;
  });
  templates.set(structuredClone(FIXTURE));
  // Scripture bound to the scripture template: the state that put a second
  // SCRIPTURE pill in the caption beside the derived role tag.
  contentTemplates.set({ scripture: 1 });
});
afterEach(unmount);

describe('the scanner this file depends on still reads the stylesheet', () => {
  // A scanner that quietly stops matching passes everything. Both of
  // `ipc.test.js`'s scanners were wrong this way while looking exhaustive.
  it('finds the grid, the caption and the card buttons', () => {
    expect(declsFor('.tg-grid')['grid-template-columns']).toMatch(/minmax\(\s*\d+px/);
    expect(declsFor('.tg-meta').padding).toBeTruthy();
    expect(declsFor('.tg-star').width).toMatch(/px$/);
    expect(FS_B2).toBeGreaterThan(0);
  });
});

describe('the card caption gives the template name a stated floor', () => {
  it('declares one, and it fits every seeded built-in whole', () => {
    const floor = px(declsFor('.tg-name')['min-width']);
    expect(
      floor,
      '`.tg-name` declares no min-width, so the name is the only flexible item in ' +
        'its row and pays every shortfall — which is how "Classic Serif" rendered as `Cla…`.',
    ).not.toBeNull();
    const longest = Math.max(...SEEDED.map((n) => textWidth(n, FS_B2)));
    expect(floor).toBeGreaterThanOrEqual(longest);
  });

  it('everything sharing the name row may ellipse before the name does', () => {
    // The kind tag is the prototype's `.kd`. It must be allowed to shrink
    // (`flex-shrink` other than 0, and `min-width:0` so it actually can), or the
    // floor above is a promise the row cannot keep.
    const role = declsFor('.tg-role');
    expect(role.flex, '`.tg-role` must be shrinkable, not `flex:0 0 auto`').toMatch(
      /^0\s+1\s/,
    );
    expect(role['min-width']).toBe('0');
    expect(role['text-overflow']).toBe('ellipsis');
  });
});

describe('the caption row can honour that floor at the narrowest column', () => {
  itMounted('prices every claim on the row against the grid\'s own minimum', async () => {
    mount();
    await drain();
    // EVERY card, not a convenient one. Priced against only the template that
    // happened to have no content look bound, this test passed over the original
    // defect — the extra pill it was written to catch was on a different card.
    expect(cards().length).toBe(FIXTURE.length);
    const worst = cards()
      .map((c) => [c.querySelector('.tg-name').textContent.trim(), priceRow(c)])
      .sort((a, b) => a[1] - b[1])[0];

    const floor = px(declsFor('.tg-name')['min-width']) ?? 0;
    expect(
      worst[1],
      `"${worst[0]}" leaves ${worst[1].toFixed(1)}px for a name whose floor is ${floor}px`,
    ).toBeGreaterThanOrEqual(floor);
  });

  /** What a card's name row leaves the name, in px, at the narrowest column. */
  function priceRow(card) {
    const row = card.querySelector('.tg-name').parentElement;
    const column = parseFloat(
      /minmax\(\s*(\d+(?:\.\d+)?)px/.exec(declsFor('.tg-grid')['grid-template-columns'])[1],
    );
    // `.tg-meta` padding is written as the two-value shorthand.
    const [, side] = declsFor('.tg-meta')
      .padding.trim()
      .split(/\s+/)
      .map((v) => parseFloat(v));
    const content = column - 2 * side;

    const kids = [...row.children];
    const gap = parseFloat(declsFor(`.${row.className.split(' ')[0]}`).gap);
    let claimed = (kids.length - 1) * gap;

    for (const el of kids) {
      if (el.querySelector('.tg-name') || el.classList.contains('tg-name')) continue;
      const btns = [...el.querySelectorAll('button')];
      if (btns.length) {
        // Fixed controls: the star and the ⋮ menu, priced from their own rule.
        const w = px(declsFor('.tg-star').width);
        const g = parseFloat(declsFor('.tg-cardbtns').gap) || 0;
        claimed += btns.length * w + (btns.length - 1) * g;
        continue;
      }
      // Anything else on the row is a text item, and what it costs the name is
      // decided by whether it can YIELD. An item that may shrink to nothing
      // (`flex-shrink` other than 0, `min-width:0`) claims nothing against the
      // floor — flexbox takes the space out of it, which is the whole point of
      // putting the kind tag there rather than beside a rigid pill. An item that
      // cannot shrink claims its declared max-width, or, with none declared,
      // whatever its own longest string is worth.
      const d = declsFor(`.${[...el.classList].filter((c) => c.startsWith('tg-'))[0]}`);
      const yields = !/^0\s+0\b/.test(d.flex ?? '') && d['min-width'] === '0';
      if (yields) continue;
      const mw = d['max-width'];
      claimed += mw?.endsWith('%')
        ? (parseFloat(mw) / 100) * content
        : (px(mw) ?? textWidth(el.textContent.trim(), FS_B2));
    }
    return content - claimed;
  }
});

describe('the caption is one row, and the used-for line sits beneath it', () => {
  itMounted('the name shares its row with the kind and the card buttons only', async () => {
    mount();
    await drain();
    const row = cardFor('Classic Serif').querySelector('.tg-name').parentElement;
    const kinds = [...row.children].map((el) =>
      el.classList.contains('tg-name')
        ? 'name'
        : el.classList.contains('tg-role')
          ? 'kind'
          : el.classList.contains('tg-cardbtns')
            ? 'buttons'
            : `unexpected:${el.className}`,
    );
    expect(kinds).toEqual(['name', 'kind', 'buttons']);
  });

  itMounted('what a template is used for is its own line, and says so in words', async () => {
    mount();
    await drain();
    const card = cardFor('Classic Serif');
    const used = card.querySelector('.tg-roletag');
    expect(used, 'a bound content look has no line of its own').toBeTruthy();
    // Beneath the name's row, not inside it — the prototype's `.roletag`.
    expect(used.parentElement).not.toBe(card.querySelector('.tg-name').parentElement);
    // The derived ROLE and the bound LOOK are two registers (DECISIONS §78).
    // Rendered as two identical uppercase pills they read as one fact printed
    // twice, which is what the 1600x1000 render showed. Words tell them apart;
    // colour cannot, because every colour that carries a promise is spoken for.
    expect(used.textContent.trim().toLowerCase()).toMatch(/^used for /);
    // …and a template bound to nothing does not get an empty line.
    expect(cardFor('Worship Lyrics').querySelector('.tg-roletag')).toBeNull();
  });

  itMounted('a card states its aspect once', async () => {
    mount();
    await drain();
    for (const c of cards()) {
      const n = (c.textContent.match(/16:9/g) ?? []).length;
      expect(n, `"${c.querySelector('.tg-name').textContent.trim()}" states 16:9 ${n} times`).toBeLessThanOrEqual(1);
    }
  });
});
