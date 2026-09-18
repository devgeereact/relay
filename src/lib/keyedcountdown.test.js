// RG-166 — A COUNTDOWN PAINTED OVER THE LIVE CAMERA, ON TEN OF THE FORTY SHELF LOOKS.
//
// `TemplateRender` has two render paths. The REGION path has refused a countdown
// on a keyed screen since the band was built — `{#if countdownTo &&
// !countdownAllowed}` renders deliberately nothing. The LAYER path never asked:
// `showDefaultCountdown` was `layered && countdown_to != null && !hasTimerLayer`,
// and `.cd-default` is `position:absolute; inset:0`, so a fired countdown painted
// a full-frame clock over the shot of the preacher on every layer-model lower
// third in the shelf.
//
// ── WHY NO EXISTING TEST SAW IT ──────────────────────────────────────────────
//
// `lowerthird.test.js` has held *"a countdown NEVER goes out on a band"* for as
// long as the rule has existed, and it is a correct test. It mounts
// `layout: { lowerThird: true }` — a REGION-model band — so it checked the one
// door that was never broken. Both models were meant to be covered; one was. That
// is CLAUDE.md's *"a guarantee is only kept on the doors you checked"*, on the
// surface where the failure is visible to the broadcast and to nobody in the room.
//
// And the predicate is the one this repository had already corrected once.
// `layers.js::isKeyedTemplate` carries the reason in its own doc comment: *"The
// old check was `layout.lowerThird`, which is FALSE for a layer-model lower third
// (its band is a shape layer, not that flag) — so blackout blacked out the camera
// and clear left the band sitting on it."* Blackout and the transparency law were
// migrated when that was found. The countdown was not.
//
// ── WHAT THIS FILE ASSERTS, AND WHAT IT DELIBERATELY DOES NOT ────────────────
//
// It mounts EVERY keyed look the shelf ships, against the real renderer, with a
// real countdown on it. Enumerating the shipped file rather than a fixture is the
// point: a test built from three hand-written templates would have passed on the
// night all ten were broken, and the shelf is what a church actually finds.
//
// It does NOT claim that nothing at all of a countdown can reach a keyed screen.
// Five of the ten (`Source · …`) are a single `region` layer — a REAL rendered
// slide inside a box, drawn from an opaque built-in template, which is the whole
// point of that family — and that inner render is not keyed and still shows the
// clock inside its own box. That is the template doing what its design says, at
// the size and position its designer chose, and it is a different thing from an
// `inset:0` overlay nobody asked for. The measured residue is asserted below by
// name so it is a recorded fact rather than an oversight.
//
//   npx vitest run src/lib/keyedcountdown.test.js

import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { isKeyedTemplate, isLayered, templateShows } from './layers.js';

const SHELF = JSON.parse(
  readFileSync(resolve(process.cwd(), 'src-tauri/data/shelf_templates.json'), 'utf8'),
).templates;

/** The looks a keyed channel composites over a camera. */
const KEYED = SHELF.filter((t) => isKeyedTemplate(t));
/** Everything else — a screen that paints its own whole frame. */
const OPAQUE = SHELF.filter((t) => !isKeyedTemplate(t));

/** A live countdown, shaped exactly as `pipeline::Fire` broadcasts one. */
const COUNTING = () => ({
  kind: 'countdown',
  reference: 'Service begins in',
  text: '',
  countdown_to: Date.now() + 283_000,
});

let host;
let app;
function mount(template, content) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

// ── THE SCANNER'S OWN GUARDS ────────────────────────────────────────────────
//
// "No keyed look paints a full-frame countdown" is satisfied by a shelf with no
// keyed looks in it, by a shelf that failed to parse, and by a `shows` list that
// had quietly stopped admitting countdowns before the render was ever reached.
// Every one of those is a green suite over an unguarded wall, which is this
// repository's most-repeated finding about its own instruments.
describe('the shelf this reads is the shelf a church gets', () => {
  it('parses, and ships both kinds of look', () => {
    expect(SHELF.length, 'the shelf file parsed to nothing').toBeGreaterThan(10);
    expect(KEYED.length, 'no keyed look in the shelf — this file would pass by emptiness').toBeGreaterThanOrEqual(1);
    expect(OPAQUE.length, 'no opaque look in the shelf — the control group is gone').toBeGreaterThanOrEqual(1);
  });

  it('the keyed looks are the ten this finding was measured against', () => {
    // Named, not counted. A count alone cannot tell "one was renamed" from "one
    // stopped being keyed", and the second is the direction that loses cover.
    expect(KEYED.map((t) => t.name).sort()).toEqual(
      [
        'Scroll · Banner',
        'Scroll · Bold',
        'Scroll · Clear',
        'Scroll · Glass',
        'Scroll · Ribbon',
        'Source · Even',
        'Source · Inset',
        'Source · Tall',
        'Source · Word left',
        'Source · Word right',
      ].sort(),
    );
  });

  it('every keyed look would be SENT a countdown, so the refusal is the renderer’s to make', () => {
    // If `shows` already dropped the kind, the fire never reaches this component
    // and the assertions below would be about nothing. They are not: all ten
    // admit `countdown`, none carries `lowerThird`, and none carries a timer
    // layer — which is precisely the combination that reached `.cd-default`.
    for (const t of KEYED) {
      expect(templateShows(t, 'countdown'), `${t.name} does not show countdowns`).toBe(true);
      expect(isLayered(t), `${t.name} is not layer-model`).toBe(true);
      expect(!!t.layout?.lowerThird, `${t.name} carries lowerThird — the old flag would have caught it`).toBe(false);
      const timerLayer = (t.layout?.layers ?? []).some(
        (L) => L.visible !== false && (L.type === 'timer' || L.bind === 'countdown'),
      );
      expect(timerLayer, `${t.name} has a timer layer, so the default never applied`).toBe(false);
    }
  });
});

describe('RG-166 · a countdown never paints full-frame over a keyed screen', () => {
  for (const t of KEYED) {
    it(`${t.name} draws no full-frame clock`, () => {
      const el = mount(t, COUNTING());
      expect(
        el.querySelector('.cd-default'),
        `${t.name} painted the default countdown overlay — inset:0, over the camera`,
      ).toBeNull();
    });
  }

  it('and the five that still show a clock show it INSIDE their own region box', () => {
    // The recorded residue, by name. A `region` layer renders a real slide from
    // an opaque built-in inside the box its designer placed, and that inner
    // render is not keyed. Asserting it here means the next reader finds a
    // measured fact instead of re-deriving it from a blank screen.
    const withClock = KEYED.filter((t) => {
      const el = mount(t, COUNTING());
      const shown = /\d+:\d\d/.test(el.textContent);
      app.$destroy();
      host.remove();
      app = host = null;
      return shown;
    }).map((t) => t.name);
    expect(withClock.sort()).toEqual(
      ['Source · Even', 'Source · Inset', 'Source · Tall', 'Source · Word left', 'Source · Word right'].sort(),
    );
    // And each of those is a single region layer — not a stray text layer that
    // has started binding the countdown on its own.
    for (const name of withClock) {
      const t = KEYED.find((x) => x.name === name);
      expect(t.layout.layers.map((L) => L.type)).toEqual(['region']);
    }
  });
});

describe('the refusal is about being KEYED, not about being empty', () => {
  it('an opaque shelf look still paints its countdown', () => {
    // The half that makes the assertions above mean something. A renderer that
    // had simply stopped drawing countdowns would satisfy every `toBeNull()` in
    // this file, and a congregation would watch a blank pre-service screen.
    const opaqueLayered = OPAQUE.filter(
      (t) =>
        isLayered(t) &&
        templateShows(t, 'countdown') &&
        !(t.layout.layers ?? []).some((L) => L.visible !== false && (L.type === 'timer' || L.bind === 'countdown')),
    );
    expect(opaqueLayered.length, 'no opaque layer-model look left to check with').toBeGreaterThanOrEqual(1);
    const el = mount(opaqueLayered[0], COUNTING());
    expect(el.querySelector('.cd-default'), `${opaqueLayered[0].name} lost its countdown`).not.toBeNull();
    expect(el.textContent).toMatch(/\d+:\d\d/);
  });

  it('a template that has not been resolved yet is not treated as keyed', () => {
    // `isKeyedTemplate(null)` and `isKeyedTemplate({})` both answer TRUE, which
    // is the right answer for blackout — nothing paints a whole frame when there
    // is no template — and the wrong one here. `Live.svelte`'s slide cells and
    // the Planner inspector both render `?? {}`, so answering the countdown
    // question from that would blank the clock on a preview surface.
    for (const empty of [{}, null, undefined]) {
      const el = mount(empty, COUNTING());
      expect(el.textContent, `an unresolved template (${JSON.stringify(empty)}) refused the countdown`).toMatch(
        /\d+:\d\d/,
      );
      app.$destroy();
      host.remove();
      app = host = null;
    }
  });
});
