// ── A SCREEN WEARS A DIFFERENT LOOK FOR EACH KIND OF CONTENT ─────────────────
//
// DECISIONS §97. `output_channels.template_id` gives a screen ONE template for
// ALL kinds; `app_settings['tpl_<kind>']` gives a per-kind default GLOBAL to every
// screen. So an operator had per-kind globally or per-screen uniformly, and never
// "on the main screen scripture looks like this and songs look like that, while
// the lobby TV uses something else for both."
//
// A per-kind look IS NOT AN OVERRIDE. It is the screen's own template, for one
// kind — so it joins one notch above the screen's blanket template, not at the
// override level, and that is what leaves §29 and the transparency law intact.
//
//   npx vitest run src/lib/channellooks.test.js
import { describe, it, expect } from 'vitest';
import { resolveOutputTemplate, lookIdFor, channelLookTemplate } from './layers.js';

/** A template that paints the whole frame — OPAQUE, so the transparency law has
 *  something real to refuse. */
const opaque = (id, name) => ({
  id,
  name,
  layout: {
    layers: [
      { id: 'bg', type: 'background', visible: true, x: 0, y: 0, w: 100, h: 100, fill: '#123456' },
    ],
  },
  style: {},
});
/** A lower third: a band and nothing behind it, so `isKeyedTemplate` says KEYED. */
const keyed = (id, name) => ({
  id,
  name,
  layout: {
    layers: [{ id: 'band', type: 'shape', visible: true, x: 0, y: 70, w: 100, h: 20, fill: '#000' }],
  },
  style: {},
});

const OWN = opaque(7, 'This screen');
const KIND = opaque(9, 'Songs on this screen');
const LOOK = opaque(12, 'The global content look');
const PINNED = opaque(20, 'A cue pinned this');
const DEFAULT = opaque(4, 'House default');

describe('the map a screen reads its own look out of', () => {
  const looks = { 1: { scripture: 9, song: 12 }, 4: { announce: 31 } };

  it('answers for the channel and kind it is asked about, and for nothing else', () => {
    expect(lookIdFor(looks, 1, 'scripture')).toBe(9);
    expect(lookIdFor(looks, 1, 'song')).toBe(12);
    expect(lookIdFor(looks, 4, 'announce')).toBe(31);
    // A kind with no row INHERITS. No row is the only spelling of that, so there
    // is nothing here to distinguish from an explicit null — see the schema.
    expect(lookIdFor(looks, 1, 'media')).toBe(null);
    expect(lookIdFor(looks, 2, 'scripture')).toBe(null);
  });

  it('matches a numeric channel against the string keys JSON gives it', () => {
    // JSON objects have string keys and this page's channel is a number. It is
    // the same comparison `roleOf` documents, and the same way a filter that
    // looks right comes to refuse everything.
    expect(lookIdFor({ '3': { scripture: 9 } }, 3, 'scripture')).toBe(9);
  });

  it('is an absence, never a throw, for every shape a broken read can hand it', () => {
    for (const bad of [null, undefined, {}, [], 'nope', 0]) {
      expect(lookIdFor(bad, 1, 'scripture')).toBe(null);
    }
    expect(lookIdFor(looks, null, 'scripture')).toBe(null);
    expect(lookIdFor(looks, 1, null)).toBe(null);
  });

  it('resolves to the template bytes the surface is already holding', () => {
    expect(channelLookTemplate(looks, 1, 'scripture', [OWN, KIND, LOOK])).toBe(KIND);
    // AN ID WITH NO BYTES IS NOT A LOOK. A screen that holds no template 31 must
    // fall through to its blanket template rather than paint nothing — this is
    // the whole reason the hub sends the bytes before the map.
    expect(channelLookTemplate(looks, 4, 'announce', [OWN, KIND, LOOK])).toBe(null);
  });
});

describe('the resolution chain, rung by rung', () => {
  it('rung 3 — the screen’s look for this kind beats its blanket template', () => {
    expect(resolveOutputTemplate(OWN, null, false, DEFAULT, KIND)).toBe(KIND);
  });

  it('rung 4 — a kind with no look of its own still wears the blanket template', () => {
    expect(resolveOutputTemplate(OWN, null, false, DEFAULT, null)).toBe(OWN);
  });

  it('rung 5 — a screen with neither still follows the global content look', () => {
    expect(resolveOutputTemplate(null, LOOK, false, DEFAULT, null)).toBe(LOOK);
  });

  it('rung 6 — and with no content look either, the configured default', () => {
    expect(resolveOutputTemplate(null, null, false, DEFAULT, null)).toBe(DEFAULT);
  });

  it('rung 2 — a PINNED cue template still outranks a per-kind look', () => {
    // A deliberate choice about THIS ITEM outranks a standing preference about
    // THIS SCREEN. §29, unchanged by any of this.
    expect(resolveOutputTemplate(OWN, PINNED, true, DEFAULT, KIND)).toBe(PINNED);
  });

  it('the global content look never outranks a per-kind look', () => {
    // §29's whole point, one level down: the SCREEN is authoritative, and a
    // per-kind look is the screen speaking.
    expect(resolveOutputTemplate(OWN, LOOK, false, DEFAULT, KIND)).toBe(KIND);
    // …and on a screen with no blanket template at all, which is the case the
    // global look exists for.
    expect(resolveOutputTemplate(null, LOOK, false, DEFAULT, KIND)).toBe(KIND);
  });
});

// ── THE LAW IS EVALUATED ONCE, AGAINST THE PINNED CLAIMANT ONLY ──────────────
//
// This is the case the naive implementation gets wrong, and it gets it wrong
// SILENTLY. Calling the existing resolver twice — screen template as channel,
// per-kind look as override — applies the transparency law BETWEEN rungs 3 and 4,
// so an opaque Announcement look chosen for a lower-third screen is discarded and
// the operator's deliberate choice moves nothing. That is DECISIONS §29's original
// complaint verbatim ("all my outputs have a template set but the output shows
// something else"), reintroduced by the feature meant to give the operator more
// control rather than less.
describe('the transparency law', () => {
  const BAND = keyed(7, 'Lower third');

  it('does not run between the per-kind look and the blanket template', () => {
    expect(
      resolveOutputTemplate(BAND, null, false, DEFAULT, KIND),
      'an opaque per-kind look was discarded on a keyed screen — the operator ' +
        'deliberately chose a full-frame Announcement look for the lower-third ' +
        'channel and got the band, with nothing saying so',
    ).toBe(KIND);
  });

  it('still refuses an opaque PINNED override on a keyed screen', () => {
    // The law itself does not move. A keyed screen is composited over a live
    // camera, and an opaque template blots out the very picture the band exists
    // to caption.
    expect(resolveOutputTemplate(BAND, PINNED, true, DEFAULT, null)).toBe(BAND);
  });

  it('is evaluated against the per-kind look when that is what the screen wears', () => {
    // Rungs 3 and 4 produce "the screen's template" FIRST, and the law is then
    // asked about that one thing. A screen whose per-kind look is itself keyed
    // must still refuse an opaque pinned override, or the camera goes.
    const BANDKIND = keyed(9, 'Keyed songs look');
    expect(resolveOutputTemplate(OWN, PINNED, true, DEFAULT, BANDKIND)).toBe(BANDKIND);
    // …and an OPAQUE per-kind look takes the pinned override as any opaque
    // screen does, because it is the screen's template now.
    expect(resolveOutputTemplate(BAND, PINNED, true, DEFAULT, KIND)).toBe(PINNED);
  });

  it('the null branch comes first, because isKeyedTemplate(null) is true', () => {
    // A template with no background layer is keyed, and an absent template has no
    // layers at all — so a following screen would otherwise "keep its keyed
    // template", which is nothing, and paint an empty frame.
    expect(resolveOutputTemplate(null, PINNED, true, DEFAULT, null)).toBe(PINNED);
  });
});
