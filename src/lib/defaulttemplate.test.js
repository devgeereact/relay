import { describe, it, expect } from 'vitest';
import { resolveOutputTemplate } from './layers.js';

// THE CONFIGURED DEFAULT IS THE LAST LINK, NEVER A NEW WINNER.
//
// DECISIONS §29 and §70 rank the authorities: the transparency law first, then
// a pinned cue template, then the screen's own template, then the content look.
// Before this test the chain simply ended, and every caller finished the
// sentence itself with `|| DEFAULT_TEMPLATE` — the bundled Classic Serif — so
// the operator's default reached a screen exactly once, when the channel was
// created. Adding it at the END is the whole change; adding it anywhere else
// would let a house look overrule a screen the operator styled by hand.

const keyed = { id: 1, name: 'Band', layout: { lowerThird: true } };
const opaque = { id: 2, name: 'Full', layout: { lowerThird: false } };
const look = { id: 3, name: 'Look', layout: { lowerThird: false } };
const fallback = { id: 9, name: 'House', layout: { lowerThird: false } };

describe('resolveOutputTemplate — the configured default', () => {
  it('answers when the screen has no template and no look applies', () => {
    expect(resolveOutputTemplate(null, null, false, fallback)).toBe(fallback);
  });

  it('never beats the screen own template', () => {
    expect(resolveOutputTemplate(opaque, null, false, fallback)).toBe(opaque);
  });

  it('never beats a pinned cue template', () => {
    expect(resolveOutputTemplate(null, look, true, fallback)).toBe(look);
  });

  it('never beats a content look', () => {
    expect(resolveOutputTemplate(null, look, false, fallback)).toBe(look);
  });

  it('does not break the transparency law on a keyed screen', () => {
    // An OPAQUE fallback on a KEYED channel would blot out the camera the band
    // exists to caption. The keyed screen keeps its own template.
    expect(resolveOutputTemplate(keyed, null, false, fallback)).toBe(keyed);
  });

  it('is optional — an absent fallback resolves exactly as before', () => {
    expect(resolveOutputTemplate(null, null, false)).toBe(null);
  });
});
