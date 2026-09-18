// RG-140 — ONE TEMPLATE RENDERS AN ANNOUNCEMENT TWO WAYS, AND A CRAWL THAT
// CANNOT CRAWL SWALLOWS THE NOTICE.
//
// Two separate claims live in RG-140's row and this file holds both, because
// until now NOTHING IN EITHER SUITE distinguished `.ticker` from `.lrun` — the
// row says so in as many words — so a green suite had no opinion at all about
// which of the two designs a church was looking at.
//
// ── ONE: THE SPLIT, CHARACTERISED RATHER THAN FIXED ────────────────────────
//
// A region-model row with `"scroll": true` paints a FOOTER TICKER: a band pinned
// to the bottom edge, a fixed label on the left under a 45% cap, the body
// crawling through what is left. `TemplateGallery.upgradeLegacyToLayers` converts
// that row on mount and SAVES, and `regionsToLayers` gives a non-band template a
// full-frame verse layer at y20 h54 carrying `scroll: true` — so the same row
// then paints the notice crawling across the MIDDLE of the wall. Same template,
// same fire, two designs, and which one a church sees depends on whether anyone
// has opened the Templates workspace.
//
// THE DECISION IS ALREADY TAKEN and it is the layer model: wave 5 Track A stopped
// seeding anything region-model, so a fresh install has exactly one rendering for
// a notice. What that decision did not do is make the CONVERSION preserve the
// design. A `scroll: true` region row is a BAND crawl by construction — that is
// what the shipped `Scroll · …` family reproduces — and converting it into a
// full-frame block is the conversion being wrong, not the region rendering being
// wrong. A notice belongs along the bottom of a wall.
//
// THE REPAIR WAS THEREFORE IN `regionsToLayers`, IN `src/lib/layers.js`, AND WAS
// DELIBERATELY NOT MADE HERE: that file was being edited by another worktree in
// the same session, and two agents rewriting one converter is how this repository
// gets two of something in the first place. What this file did instead was make
// the split VISIBLE, and say that the day the conversion was made design-
// preserving, the assertion below would turn red and send its reader to this
// comment.
//
// THAT IS WHAT HAPPENED, on the same day, in the same session (RG-174). The
// converter now gives a `scroll: true` row a band on the bottom edge, the
// characterisation assertion went red on the rebase, and it has been REPLACED
// with one that asserts the band — not deleted, and not relaxed. The case below
// carries both halves of that history at its own call site. The split itself is
// unchanged and still characterised here: a region row paints `.ticker`, a
// converted row paints `.lrun`, and which one a church sees still depends on
// whether anyone has opened the Templates workspace. What RG-174 fixed is that
// the two now agree about WHERE a notice goes.
//
// This is what a characterisation test is for, and it is strictly better than the
// silence the row was complaining about: the fix could not land quietly.
//
// ── TWO: THE HALF THAT IS RENDERER WORK, AND IS FIXED HERE ─────────────────
//
// RG-140's own cell assigns this one: *"`.lscroll .lrun` still sets `animation:
// none` under `prefers-reduced-motion`, inside a `white-space: nowrap; overflow:
// hidden` box, so a notice longer than its band is silently CUT OFF … that is
// renderer work in `TemplateRender`."*
//
// Measured at 1920×1080 on the shipped `Scroll · Banner` with a 185-character
// notice and reduced motion on: `clientWidth 1651` against `scrollWidth 4299`, so
// **2648px — 62% of the notice — was never painted at all**, while `onFit`
// reported `{ scale: 1, legible: true, clipped: false }`. The region ticker was
// worse: `1617` against `5080`, 68% gone, same report. A fit that reports success
// over a notice with two thirds missing is rule 37 and rule 35 in one place.
//
// DECISIONS §88 already had to distort the shelf around this — `High Visibility ·
// Announcement` deliberately does not scroll, *"because a crawl that stops under
// `prefers-reduced-motion` silently truncates a notice, and a family built for a
// low-vision or vestibular reader cannot ship the one member most likely to cut
// text off screen"* — which is the right instinct and should not have needed a
// carve-out.
//
// AFTER, same instrument. Layer mode: `.lfit` loses `lscroll`, wraps inside
// `.ltext` (a real percentage box, its designer's own), 1651/1651 wide and
// 181/180 tall — nothing clipped — at 46.76px, reported `{ scale: 0.812 }`.
// Region mode: the track measures 1617/1617, the run sits at 20.28px, reported
// `{ scale: 0.440, legible: false }`. The band keeps its geometry: y972 h108
// against y965 h115 before.
//
// WRAPPING THE REGION BAND WAS TRIED FIRST AND MEASURED AND REJECTED: it grew
// that band from 115px to 236px and lifted its top edge from y965 to y844, a
// legacy footer silently becoming a fifth of the wall over a camera because
// somebody's operating system prefers less motion. Layer mode has no such
// conflict, which is the same lesson `.content.cdbox` records under RG-141: a
// text box with a real rectangle can absorb a change that a shrink-to-fit one
// turns into a redesign.
//
// jsdom computes no layout, so nothing here claims a pixel; the figures above are
// the browser's. What is asserted is which elements the renderer emits, which is
// what decides all of it.
//
//   npx vitest run src/lib/noticecrawl.test.js

import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { regionsToLayers, isLayered } from './layers.js';

const SHELF = JSON.parse(
  readFileSync(resolve(process.cwd(), 'src-tauri/data/shelf_templates.json'), 'utf8'),
).templates;

/** A region-model announcement row, shaped exactly as the pre-wave-5 seed shipped
 *  one: full frame, `scroll: true`. This is what an existing install still holds
 *  wherever a channel, a cue, a content look or the default names it. */
const REGION_ANNOUNCEMENT = {
  name: 'Classic · Announcement',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
  style: { verseColor: '#ffffff', accent: '#4fa8c9', verseSize: '2.4', refSize: '2.4', scroll: true },
};

/** The shipped crawls — the surface the reduced-motion half applies to. */
const CRAWLS = SHELF.filter((t) =>
  (t.layout?.layers || []).some((L) => L.scroll && L.visible !== false),
);

const NOTICE = {
  kind: 'announcement',
  content_kind: 'announcement',
  reference: 'Notice',
  text: 'The church council meets in the hall straight after this morning service, all welcome; the youth residential deposit is due by Friday the twelfth and forms are on the table by the door.',
};

let host;
let app;
function mount(template, content = NOTICE) {
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

/** `reduceMotion` is read once, at component init, from `window.matchMedia`. jsdom
 *  answers `false` for everything, so the preference has to be installed BEFORE a
 *  mount and taken away after — otherwise every other test in the run inherits it. */
let realMatchMedia;
function prefersReducedMotion(on) {
  window.matchMedia = (q) => ({
    matches: on && /prefers-reduced-motion/.test(q) && !/no-preference/.test(q),
    media: q,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    onchange: null,
    dispatchEvent: () => false,
  });
}
beforeEach(() => {
  realMatchMedia = window.matchMedia;
});
afterEach(() => {
  window.matchMedia = realMatchMedia;
});

describe('the two renderings of one announcement, told apart at last', () => {
  it('a region-model row paints the footer ticker', () => {
    expect(isLayered(REGION_ANNOUNCEMENT), 'the fixture is not region-model').toBe(false);
    const el = mount(REGION_ANNOUNCEMENT);
    expect(el.querySelector('.ticker'), 'no footer band').toBeTruthy();
    expect(el.querySelector('.ticker-run'), 'no crawling body').toBeTruthy();
    expect(el.querySelector('.ticker-label'), 'no fixed label').toBeTruthy();
    expect(el.querySelector('.lrun'), 'a region row painted a layer crawl').toBeNull();
  });

  it('and the SAME row, once converted, still paints a band — not a full-frame crawl', () => {
    // THIS WAS THE DEFECT, AND IT IS NOW THE FIX, SWAPPED RATHER THAN DELETED.
    // The characterisation this case carried — a converted crawl ending at 74,
    // a block across the MIDDLE of the wall — went red the moment RG-174 taught
    // `regionsToLayers` to give a `scroll: true` row a band, which is exactly
    // what the header said should happen. The assertion below is its replacement
    // and asserts the design rather than the bug: the crawl is a strip on the
    // bottom edge, as the region path always painted it.
    //
    // The DESIGN SPLIT this file exists for is unchanged and still characterised
    // above: a region row paints `.ticker`, a converted row paints `.lrun`. What
    // RG-174 fixed is the GEOMETRY of the converted rendering, not the fact that
    // there are two of them. `noticecrawlconvert.test.js` holds the converter's
    // own contract in full (the label's band, its side, the no-reference case);
    // this case is the one that had to fail here for that work to be visible.
    const converted = { ...REGION_ANNOUNCEMENT, layout: regionsToLayers(REGION_ANNOUNCEMENT) };
    expect(isLayered(converted), 'the conversion produced no layers').toBe(true);
    const el = mount(converted);
    expect(el.querySelector('.lrun'), 'no layer crawl after conversion').toBeTruthy();
    expect(el.querySelector('.ticker'), 'the footer band survived the conversion').toBeNull();

    // A notice belongs along the bottom of a wall, on BOTH paths. Before RG-174
    // this box was `y20 h54` and ended at 74 — most of the frame, nothing near an
    // edge. It is a band now, so it ends ON the bottom edge and is short.
    const crawl = converted.layout.layers.find((L) => L.scroll);
    expect(crawl, 'the conversion dropped the crawl').toBeTruthy();
    expect(
      crawl.y + crawl.h,
      'the converted crawl does not reach the bottom edge, so it is not a band',
    ).toBe(100);
    expect(
      crawl.h,
      'the converted crawl is too tall to be a band — this is the y20 h54 ' +
        'full-frame block RG-174 removed, back again',
    ).toBeLessThanOrEqual(20);
  });
});

describe('a crawl that cannot crawl does not swallow the notice', () => {
  it('the shelf ships crawls for this to matter to', () => {
    expect(CRAWLS.length, 'no shipped look scrolls a text layer').toBeGreaterThan(0);
  });

  it.each(CRAWLS.map((t) => [t.name, t]))('%s crawls when motion is welcome', (name, template) => {
    prefersReducedMotion(false);
    const el = mount(template);
    expect(el.querySelector('.lrun'), `${name}: the crawl stopped crawling`).toBeTruthy();
    expect(el.querySelector('.lfit.lscroll'), `${name}: the crawl lost its clip`).toBeTruthy();
  });

  it.each(CRAWLS.map((t) => [t.name, t]))(
    '%s becomes an ordinary fitted layer when it is not',
    (name, template) => {
      prefersReducedMotion(true);
      const el = mount(template);
      expect(el.querySelector('.lrun'), `${name}: a still crawl is still a crawl`).toBeNull();
      // THE CLASS IS THE HALF THAT MATTERS TO THE FITTER. `fitLayers` skips any
      // `.lscroll` element outright — "it scrolls, so its length is time, not
      // overflow" — so leaving the class on a stopped crawl would keep the notice
      // unmeasured and unreported, which is the defect with a different painter.
      expect(el.querySelector('.lfit.lscroll'), `${name}: the fitter still thinks it moves`).toBeNull();
      expect(el.querySelector('.lfit'), `${name}: no text box at all`).toBeTruthy();
    },
  );

  it('the region ticker says so too, and its fitter reads the same fact', () => {
    prefersReducedMotion(true);
    const el = mount(REGION_ANNOUNCEMENT);
    const band = el.querySelector('.ticker');
    expect(band, 'no footer band').toBeTruthy();
    expect(band.classList.contains('still'), 'a stopped ticker did not say it had stopped').toBe(true);
  });

  it('and one fact decides all three — the paint, the clip and the measurement', () => {
    // A stylesheet media query and a JS `matchMedia` read can disagree in
    // principle; what may not happen is the RENDERER and the FITTER disagreeing,
    // because that is how a notice ends up painted still and measured as moving.
    // Both follow `crawls()` / `reduceMotion`, and this asserts it rather than
    // trusting that the two reads were written by the same person on the same day.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect(src).toMatch(/const crawls = \(L\) => !!L\?\.scroll && !reduceMotion;/);
    expect(src).toMatch(/class:lscroll=\{crawls\(L\)\}/);
    expect(src).toMatch(/\{#if crawls\(L\)\}/);
    expect(src).toMatch(/class:still=\{reduceMotion\}/);
    // And the region body is measured against the TRACK, never against the run:
    // an `inline-block` under `nowrap` sizes its own box, so `scrollWidth` on it
    // can never exceed `clientWidth` and the loop would measure a box that cannot
    // report — the same trap `.ticker-label` and `.content` both fell into.
    const fn = src.slice(src.indexOf('function fitStillBody('));
    expect(fn.slice(0, 900)).toMatch(/track\.scrollWidth > track\.clientWidth/);
  });
});
