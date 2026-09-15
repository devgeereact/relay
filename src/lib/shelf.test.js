// THE SHELF — the prototype's own looks, seeded and rendered (REBRAND wave 4, T2).
//
// `src-tauri/data/shelf_templates.json` is read by TWO things: `db/templates.rs`
// seeds it, and this file renders it. That is the whole reason it is a data file
// rather than a Rust string literal beside the other presets — the last time a
// shipped design lived in two places (`BUILTINS` here, `builtin_templates()`
// there) they drifted by one row and a kiosk resolving `template_id=4` rendered a
// song through a scripture look.
//
// What this file claims, and it is deliberately narrow:
//
//   1. every entry DERIVES a role, and the role is the one it is for — the
//      failure phase 4 closed was `templateKind` reading every layer template as
//      `custom`, so a role could have a renderer, have a row, and never appear on
//      one;
//   2. every entry RENDERS through the real `TemplateRender` — the band lays its
//      words out, the composite renders a real inner template, the media frame
//      paints a picture;
//   3. nothing hangs off the frame. jsdom does no layout, so this is arithmetic
//      over the boxes the renderer actually emitted — which, for a band, are the
//      boxes `bandLayout` DERIVED, not the ones stored in the file. The stored
//      ones are checked on the Rust side; these are the drawn ones, and they are
//      different numbers.
//
// What it does NOT claim: that the type is the right SIZE. That needs measurement
// and jsdom has none — `templatefit.test.js` owns the fit, and the real evidence
// is a rendered browser pass by the lead.
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { templateKind, kindsPresent, KIND_ORDER } from './templateKind.js';
import { isLayered, isKeyedTemplate } from './layers.js';

const SHELF_PATH = resolve(__dirname, '../../src-tauri/data/shelf_templates.json');
const SHELF = JSON.parse(readFileSync(SHELF_PATH, 'utf8')).templates;

/** The role each shelf entry is FOR. A table rather than a loop over
 *  `templateKind` comparing it to itself: the point is that the derivation and
 *  the intent agree, which a self-comparison cannot show. */
const ROLE_OF = {
  'High Visibility': 'scripture',
  // NOT `announcement`, and that is the honest answer rather than a miss. A
  // full-screen notice and a full-screen verse are the same shape — a large line
  // and a small one — which is exactly why `templateKind` refuses to guess
  // `preservice` (DECISIONS §78). A church points this at announcements through
  // the content-look register (`tpl_announcement`), which is where a role is
  // DECLARED in Relay. The scrolling ticker is the shape that derives as one.
  'Notice Board': 'scripture',
  'Media Frame': 'media',
  'Lower Third · Lyric': 'lower-third',
  'Lower Third · Scripture': 'lower-third',
  'SuperSource · Word right': 'supersource',
  'SuperSource · Word left': 'supersource',
  'Stage · Large type': 'stage',
};

const CONTENT = {
  reference: 'John 3:16',
  text: 'For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.',
};
const MEDIA_CONTENT = {
  ...CONTENT,
  media_url: 'http://127.0.0.1:8032/media/7',
  media_kind: 'image',
};

let host;
let app;
function mount(template, content = CONTENT) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

const byName = (n) => SHELF.find((t) => t.name === n);

/** Pull `left/top/width/height` percentages back out of an emitted inline style. */
function boxOf(el) {
  const s = el.getAttribute('style') || '';
  const num = (k) => {
    const m = s.match(new RegExp(`${k}\\s*:\\s*(-?[\\d.]+)%`));
    return m ? Number(m[1]) : null;
  };
  return { x: num('left'), y: num('top'), w: num('width'), h: num('height') };
}

describe('the shelf file', () => {
  it('is the eight looks the prototype ships that Relay did not have', () => {
    expect(SHELF.map((t) => t.name)).toEqual(Object.keys(ROLE_OF));
  });

  it('is entirely layer-model, because that is where bands and regions live', () => {
    for (const t of SHELF) expect(isLayered(t), t.name).toBe(true);
  });
});

describe('every shelf look derives the role it is for', () => {
  // THE TRIPWIRE this file exists for. `templateKind` read `layout.regions` only
  // until phase 4, so every layer template — the lower thirds and the composite
  // included — came back `custom`, and the gallery rail could offer no row but
  // Custom for any of them.
  it('and not one of them is Custom', () => {
    const got = Object.fromEntries(SHELF.map((t) => [t.name, templateKind(t)]));
    expect(got).toEqual(ROLE_OF);
    expect(Object.values(got)).not.toContain('custom');
  });

  it('so the gallery rail gains four rows a fresh install could not show before', () => {
    // The rail is one row per role that OCCURS (`kindsPresent`), in display order.
    const rows = kindsPresent(SHELF).map((k) => k.key);
    expect(rows).toEqual(['scripture', 'lower-third', 'supersource', 'stage', 'media']
      .sort((a, b) => KIND_ORDER.indexOf(a) - KIND_ORDER.indexOf(b)));
    for (const want of ['supersource', 'stage', 'media']) {
      expect(rows, `${want} is why this shelf exists`).toContain(want);
    }
  });
});

describe('every shelf look renders through the one renderer', () => {
  it('paints the verse and its reference on a full-screen scripture look', () => {
    const el = mount(byName('High Visibility'));
    expect(el.textContent).toContain('For God so loved');
    expect(el.textContent).toContain('John 3:16');
  });

  it('paints a notice over its gradient', () => {
    const el = mount(byName('Notice Board'));
    expect(el.querySelector('.lbg').getAttribute('style')).toContain('linear-gradient');
    expect(el.textContent).toContain('For God so loved');
  });

  it('paints the picture inside the frame, and nothing over it', () => {
    const el = mount(byName('Media Frame'), MEDIA_CONTENT);
    const img = el.querySelector('.lmediabox img');
    expect(img).toBeTruthy();
    expect(img.getAttribute('src')).toBe(MEDIA_CONTENT.media_url);
    // A caption line would be blank at every fire — Relay's media fire carries
    // media_url and nothing else. The frame deliberately has no text layer.
    expect(el.querySelectorAll('.ltext').length).toBe(0);
  });

  it('lays a band out and gives its words boxes the band computed', () => {
    const el = mount(byName('Lower Third · Scripture'));
    const band = el.querySelector('.lband');
    expect(band).toBeTruthy();
    // §75: the band runs from its own `top` to the BOTTOM edge.
    const b = boxOf(band);
    expect(b.y + b.h).toBeCloseTo(100, 4);
    expect(el.textContent).toContain('John 3:16');
  });

  it('a lyric band carries the words and NO reference at all', () => {
    const el = mount(byName('Lower Third · Lyric'));
    expect(el.textContent).toContain('For God so loved');
    expect(el.textContent).not.toContain('John 3:16');
  });

  it('both lower thirds are KEYED — no background, so the camera survives', () => {
    for (const n of ['Lower Third · Lyric', 'Lower Third · Scripture']) {
      expect(isKeyedTemplate(byName(n)), n).toBe(true);
    }
  });

  it('a composite renders a REAL inner template, not a picture of one', () => {
    // DECISIONS §74. `wtpl:1` is Classic Serif at region width, so the words in
    // the composite are the same renderer's output — change the built-in and the
    // composite changes with it.
    const el = mount(byName('SuperSource · Word right'));
    const region = el.querySelector('.lregion');
    expect(region).toBeTruthy();
    expect(region.textContent).toContain('For God so loved');
    // The camera half is what is NOT painted: the region starts at 30%, and the
    // plates only cover the bars above and below.
    expect(boxOf(region).x).toBeCloseTo(30, 4);
  });

  it('the other arrangement is the same region with its x moved', () => {
    const el = mount(byName('SuperSource · Word left'));
    const region = el.querySelector('.lregion');
    expect(boxOf(region).x).toBeCloseTo(0, 4);
    expect(region.textContent).toContain('For God so loved');
  });

  it('a stage look shows what is coming, which no congregation screen does', () => {
    const el = mount(byName('Stage · Large type'), { ...CONTENT, next_reference: 'Psalm 23:1' });
    expect(el.textContent).toContain('Psalm 23:1');
    expect(el.textContent).toContain('NEXT');
  });
});

describe('nothing a shelf look draws hangs off the frame', () => {
  // The DRAWN boxes, not the stored ones. A band's words are positioned by
  // `bandLayout` at render time from the band's own geometry, so the numbers
  // asserted here were computed rather than typed — which is the half the Rust
  // test cannot reach.
  it.each(SHELF.map((t) => t.name))('%s', (name) => {
    const el = mount(byName(name), MEDIA_CONTENT);
    const drawn = el.querySelectorAll('.ltext, .lband, .lshape, .lregion, .lmediabox');
    expect(drawn.length, `${name} drew nothing`).toBeGreaterThan(0);
    for (const node of drawn) {
      const { x, y, w, h } = boxOf(node);
      if (x == null || y == null || w == null || h == null) continue;
      expect(x, `${name}: negative x`).toBeGreaterThanOrEqual(-0.0001);
      expect(y, `${name}: negative y`).toBeGreaterThanOrEqual(-0.0001);
      expect(x + w, `${name}: runs off the right edge`).toBeLessThanOrEqual(100.0001);
      expect(y + h, `${name}: runs off the bottom edge`).toBeLessThanOrEqual(100.0001);
    }
  });
});
