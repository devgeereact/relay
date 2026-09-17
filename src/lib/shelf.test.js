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

/** The role each shelf entry DERIVES, named one by one. A table rather than a
 *  loop over `templateKind` comparing it to itself: the point is that the
 *  derivation and the intent agree, which a self-comparison cannot show.
 *
 *  THE PREFIX IS FOR FINDING, NOT FOR DECIDING. DECISIONS §78 keeps a template's
 *  role derived from what it renders, and three of the eight prefixes therefore
 *  land somewhere else on purpose:
 *
 *   · **Announce · ***  derives `scripture` (a large line and a small one) or
 *     `song` (one line alone). A full-screen notice and a full-screen verse are
 *     the same SHAPE, which is exactly why `templateKind` refuses to guess a
 *     pre-service role. The scrolling ticker is the shape that derives as an
 *     announcement, and that is the Scroll role below. A church points these at
 *     notices through the content-look register (`tpl_announce`), which is where
 *     a role is DECLARED in Relay. The same answer was already recorded here for
 *     `Notice Board` before wave 5, and it is honest rather than a miss.
 *   · **Scroll · ***  derives `lower-third`: a `band` layer is a lower third
 *     whatever it holds (§4), and the band rule is more specific than the scroll
 *     rule. These are lower thirds that crawl, which is what the role is called.
 *   · **Source · ***  derives `supersource`, which is the one prefix that agrees.
 */
const ROLE_OF = {
  'Scripture · Dayspring': 'scripture',
  'Scripture · Meridian': 'scripture',
  'Scripture · Vellum': 'scripture',
  'Scripture · Indigo': 'scripture',
  'Scripture · Column': 'scripture',
  'Song · Anthem': 'song',
  'Song · Chorus': 'song',
  'Song · Open': 'song',
  'Song · Plain': 'song',
  'Song · Ember': 'song',
  'Media · Full': 'media',
  'Media · Caption': 'media',
  'Media · Frame': 'media',
  'Media · Split': 'media',
  'Media · Wash': 'media',
  'Announce · Board': 'scripture',
  'Announce · Card': 'scripture',
  'Announce · Bold': 'song',
  'Announce · List': 'scripture',
  'Announce · Contrast': 'scripture',
  'Timer · Monolith': 'timer',
  'Timer · Rail': 'timer',
  'Timer · Titled': 'timer',
  'Timer · Panel': 'timer',
  'Timer · Contrast': 'timer',
  'Scroll · Banner': 'lower-third',
  'Scroll · Ribbon': 'lower-third',
  'Scroll · Glass': 'lower-third',
  'Scroll · Bold': 'lower-third',
  'Scroll · Clear': 'lower-third',
  'Source · Word right': 'supersource',
  'Source · Word left': 'supersource',
  'Source · Tall': 'supersource',
  'Source · Inset': 'supersource',
  'Source · Even': 'supersource',
  'Stage · Reading': 'stage',
  'Stage · Next': 'stage',
  'Stage · Message': 'stage',
  'Stage · Rail': 'stage',
  'Stage · Plain': 'stage',
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
  it('is the forty looks a church finds on a fresh install', () => {
    // Name for name against the table above rather than a count, so a look going
    // missing cannot pass by being replaced with another.
    expect(SHELF.map((t) => t.name)).toEqual(Object.keys(ROLE_OF));
    expect(SHELF).toHaveLength(40);
  });

  it('is eight roles of five', () => {
    const byRole = {};
    for (const t of SHELF) {
      const role = t.name.split(' · ')[0];
      byRole[role] = (byRole[role] ?? 0) + 1;
    }
    expect(byRole).toEqual({
      Scripture: 5, Song: 5, Media: 5, Announce: 5,
      Timer: 5, Scroll: 5, Source: 5, Stage: 5,
    });
  });

  it('is entirely layer-model, because that is where bands and regions live', () => {
    // AND BECAUSE THE REGION MODEL IS THE FACT UNDERNEATH RG-140 AND RG-141. A
    // region-model countdown paints 30.3px digits against 192px designed; a
    // region-model announcement paints a footer ticker before the Templates
    // workspace is opened and a mid-screen crawl after. Neither is closed by
    // repairing the region fitter; both are closed by seeding nothing that
    // reaches it, and this is the assertion that keeps it closed.
    for (const t of SHELF) {
      expect(isLayered(t), t.name).toBe(true);
      expect(t.layout.regions, `${t.name} carries region keys`).toBeUndefined();
    }
  });

  it('declares layout.shows explicitly, on every one of them', () => {
    // `templateShows` returns true for EVERY kind when `layout.shows` is absent,
    // so a silent template claims all five implicitly and the editor's filter
    // register has to materialise a list on the operator's first click — which is
    // what made that click look as though it had wiped four. And it is all five
    // every time: `layout.shows` is a per-SCREEN filter read at `Output.svelte`
    // BEFORE `resolveOutputTemplate`, so a narrow list on a screen's own template
    // makes that screen silently drop every other kind. What a template is FOR is
    // the content-look register, a different register entirely.
    for (const t of SHELF) {
      expect(t.layout.shows, `${t.name} declares nothing`).toEqual([
        'scripture', 'song', 'media', 'announce', 'countdown',
      ]);
    }
  });

  it('gives every look an identity the legacy conversion cannot destroy', () => {
    // RG-142: a byte-matching retirement removed 0 of 21 against a copy of a real
    // database, because `upgradeLegacyToLayers` had rewritten every row on mount.
    const keys = SHELF.map((t) => t.seed_key);
    for (const k of keys) expect(typeof k, 'a look with no seed_key').toBe('string');
    expect(new Set(keys).size, 'two looks share a seed_key').toBe(40);
    for (const k of keys) expect(k.startsWith('legacy.'), k).toBe(false);
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

  it('so the gallery rail shows every role Relay can render', () => {
    // The rail is one row per role that OCCURS (`kindsPresent`), in display order.
    // Seven of the eight, and the missing one is `announcement` — see the table
    // above: only a SCROLLING text layer derives it, and the Scroll five are
    // bands, which is the more specific rule. A row for a role nothing renders
    // would be a control that saves a setting nothing reads (DECISIONS §78).
    const rows = kindsPresent(SHELF).map((k) => k.key);
    expect(rows).toEqual(
      ['scripture', 'song', 'lower-third', 'supersource', 'stage', 'media', 'timer'].sort(
        (a, b) => KIND_ORDER.indexOf(a) - KIND_ORDER.indexOf(b),
      ),
    );
    expect(rows).not.toContain('custom');
  });
});

describe('every shelf look renders through the one renderer', () => {
  it('paints the verse and its reference on a full-screen scripture look', () => {
    const el = mount(byName('Scripture · Dayspring'));
    expect(el.textContent).toContain('For God so loved');
    expect(el.textContent).toContain('John 3:16');
  });

  it('paints a notice over its ground', () => {
    const el = mount(byName('Announce · Board'));
    expect(el.querySelector('.lbg').getAttribute('style')).toContain('#0a0e14');
    expect(el.textContent).toContain('For God so loved');
  });

  it('paints the picture inside the frame, with only its caption over it', () => {
    const el = mount(byName('Media · Frame'), MEDIA_CONTENT);
    const img = el.querySelector('.lmediabox img');
    expect(img).toBeTruthy();
    expect(img.getAttribute('src')).toBe(MEDIA_CONTENT.media_url);
    // The caption binds `reference`, never `verse`. A media fire carries
    // `media_url` and a reference; a verse layer here would be blank at every
    // fire — and it would make `templateKind` read the look as scripture.
    const binds = byName('Media · Frame').layout.layers
      .filter((L) => L.type === 'text')
      .map((L) => L.bind);
    expect(binds).toEqual(['reference']);
  });

  it('the picture alone has nothing over it at all', () => {
    const el = mount(byName('Media · Full'), MEDIA_CONTENT);
    expect(el.querySelector('.lmediabox img')).toBeTruthy();
    expect(el.querySelectorAll('.ltext').length).toBe(0);
  });

  it('lays a band out and gives its words boxes the band computed', () => {
    const tpl = byName('Scroll · Banner');
    expect(tpl, 'the shelf has no band left to measure').toBeTruthy();
    const el = mount(tpl);
    const band = el.querySelector('.lband');
    expect(band).toBeTruthy();
    // §75: the band runs from its own `top` to the BOTTOM edge.
    const b = boxOf(band);
    expect(b.y + b.h).toBeCloseTo(100, 4);
    // AND THE BAND COMPUTED ITS MEMBER'S BOX, which is the half of §75 that the
    // band's own geometry does not show. A member is NOT a DOM descendant of the
    // band — both are absolutely-positioned siblings emitted by the same `{#each}`
    // — so no selector can express containment, and one that looks as though it
    // does (`.lband ~ .ltext, .ltext`) reduces to "every `.ltext` on the page".
    // What IS answerable is that the rendered box is the band's arithmetic rather
    // than the layer's own authoring: `drawBoxes` overrides a member's box.
    const words = el.querySelectorAll('.ltext');
    expect(words.length, 'the band laid out none of its members').toBe(1);
    const m = boxOf(words[0]);
    const authored = tpl.layout.layers.find((L) => L.id === 'slb-crawl');
    expect(m.y, 'the member kept its authored top, so the band computed nothing')
      .not.toBeCloseTo(authored.y, 4);
    expect(m.y).toBeGreaterThanOrEqual(b.y);
    expect(m.y + m.h).toBeLessThanOrEqual(b.y + b.h);
  });

  it('a scrolling band carries the notice and NO reference at all', () => {
    const el = mount(byName('Scroll · Banner'));
    expect(el.textContent).toContain('For God so loved');
    expect(el.textContent).not.toContain('John 3:16');
  });

  it('a ribbon keeps its fixed label over the left end of the crawl track', () => {
    // TWO THINGS `bandLayout` DOES THAT A SECOND MEMBER CANNOT SURVIVE, both found
    // by building this look: it STACKS members vertically, so a chip declared as a
    // second member is a second LINE (which hung 2% off the bottom of a band this
    // thin); and it overrides a member's x to the band's own inset, so a member can
    // never sit beside another whatever it was authored at. The chip is a sibling
    // painted AFTER the crawl instead, which is how a ticker does this.
    const tpl = byName('Scroll · Ribbon');
    expect(tpl.layout.layers.find((L) => L.type === 'band').members).toEqual(['slr-crawl']);
    const order = tpl.layout.layers.map((L) => L.id);
    expect(order.indexOf('slr-chip'), 'the chip paints under the crawl it must mask')
      .toBeGreaterThan(order.indexOf('slr-crawl'));
    const el = mount(tpl);
    const chip = boxOf(el.querySelector('.lshape'));
    const crawl = boxOf(el.querySelectorAll('.ltext')[0]);
    expect(chip.x).toBeLessThanOrEqual(crawl.x + 0.0001);
    expect(chip.x + chip.w, 'the chip does not reach the crawl it masks')
      .toBeGreaterThan(crawl.x);
  });

  it('every scrolling lower third is KEYED — no ground, so the camera survives', () => {
    // `isKeyedTemplate(undefined)` is TRUE (nothing paints the frame, because
    // there is no frame), so a missing subject would pass this silently. Each
    // lookup is asserted first for exactly that reason.
    for (const name of ['Scroll · Banner', 'Scroll · Ribbon', 'Scroll · Glass',
                        'Scroll · Bold', 'Scroll · Clear']) {
      const tpl = byName(name);
      expect(tpl, `${name} is not on the shelf`).toBeTruthy();
      expect(isKeyedTemplate(tpl), name).toBe(true);
    }
    // AND A FULL-SCREEN MEDIA SLIDE IS NOT. The transparency law exempts a keyed
    // template from an opaque override and from a blackout, which is right for a
    // caption bar over a camera and wrong for a picture filling the wall.
    for (const name of ['Media · Full', 'Media · Caption', 'Media · Wash']) {
      expect(isKeyedTemplate(byName(name)), name).toBe(false);
    }
  });

  it('a composite renders a REAL inner template, not a picture of one', () => {
    // DECISIONS §74. `templateRef` names a BUILT-IN, because a kiosk or OBS page
    // has no database and resolves the id against the bundled list — so the words
    // in the composite are the same renderer's output at the region's width.
    const el = mount(byName('Source · Word right'));
    const region = el.querySelector('.lregion');
    expect(region).toBeTruthy();
    expect(region.textContent).toContain('For God so loved');
    // The camera half is what is NOT painted: the region starts at 50%.
    expect(boxOf(region).x).toBeCloseTo(50, 4);
  });

  it('the other arrangement is the same region with its x moved', () => {
    const el = mount(byName('Source · Word left'));
    const region = el.querySelector('.lregion');
    expect(boxOf(region).x).toBeCloseTo(2, 4);
    expect(region.textContent).toContain('For God so loved');
  });

  it('a stage look shows what is coming, which no congregation screen does', () => {
    const el = mount(byName('Stage · Next'), { ...CONTENT, next_reference: 'Psalm 23:1' });
    expect(el.textContent).toContain('Psalm 23:1');
    expect(el.textContent).toContain('Up Next');
  });

  it('a timer look places its digits rather than falling into the default block', () => {
    // RG-141's other half. `.cd-default` is the fallback a template with no
    // countdown-bound layer takes; every Timer look carries a real one, so the
    // digits are laid out by the layer stack at the size the design asked for.
    for (const name of ['Timer · Monolith', 'Timer · Rail', 'Timer · Titled',
                        'Timer · Panel', 'Timer · Contrast']) {
      const tpl = byName(name);
      const bound = tpl.layout.layers.filter((L) => L.bind === 'countdown');
      expect(bound.length, `${name} has no countdown layer`).toBe(1);
      const el = mount(tpl, { ...CONTENT, countdown_to: Date.now() + 60_000 });
      expect(el.querySelector('.cd-default'), `${name} took the fallback block`).toBeNull();
      app.$destroy();
      host.remove();
      app = null;
      host = null;
    }
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
