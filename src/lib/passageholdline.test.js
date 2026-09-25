// ── THE OPERATOR CAN SEE WHAT IS BEING HELD BACK (RG-306) ───────────────────
//
// §122's guard stops offering verses while a preacher is reading. That is what
// was asked for, and it is also, from the operator's chair, indistinguishable
// from a detector that has gone deaf — which is rule 35 stated exactly.
//
// `detection://held` carries the answer and `capture.js`'s `passageHold` holds
// it. Nothing rendered it, and a fact nobody can see is not a status. This file
// is the surface half.
//
// WHAT IT MAY SAY, and the limits are rule 18's: never a percentage (a held
// candidate's confidence is the least trustworthy number in the product — it is
// the score of a claim Relay decided not to act on), and never a promise colour.
// Ochre, because a hold is a caution: it warns, it is not a failure, and it
// promises nothing about a screen.
import { describe, it, expect, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn(() => Promise.resolve([]));
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const Dock = (await import('./Dock.svelte')).default;
const { passageHold } = await import('./stores/capture.js');
const DOCK = codeOnly(readFileSync(resolve('src/lib/Dock.svelte'), 'utf8'));

let app;
let host;

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  passageHold.set(null);
});

async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host });
  await tick();
  await tick();
  return host;
}

const HOLD = {
  passage: 'Psalms 107',
  reading: 'oh that men would praise the lord for his goodness',
  held: [
    { reference: 'Psalms 107:21', method: 'reading', matched_text: 'oh that men would praise the lord', reason: 'outside_the_reading' },
    { reference: 'Psalms 107:8', method: 'reading', matched_text: 'for his goodness', reason: 'already_on_screen' },
  ],
  trace_id: 'abc',
};

describe('the transcript card says when Relay is holding a reading together', () => {
  it('says nothing at all when nothing is held', async () => {
    await mount();
    expect(host.querySelector('.phold'), 'a hold line is drawn over no hold').toBeNull();
  });

  it('names the passage and how many it is holding', async () => {
    await mount();
    passageHold.set(HOLD);
    await tick();
    const line = host.querySelector('.phold');
    expect(line, 'nothing renders the hold').toBeTruthy();
    expect(line.textContent).toContain('Psalms 107');
    // The COUNT, because "holding" without a number reads as a state rather than
    // an amount, and the operator's question is how much they are not seeing.
    expect(line.textContent).toMatch(/\b2\b/);
  });

  it('and the wall-rule case reads differently from the outside-the-reading case', async () => {
    // Rule 35's own test. "Relay is holding something" is not a status if it
    // says the same thing for two different reasons — one is the verse already
    // being up, which is reassuring, and the other is a verse from elsewhere,
    // which the operator may want to override.
    await mount();
    passageHold.set({ ...HOLD, passage: null, held: [HOLD.held[1]] });
    await tick();
    const onScreen = host.querySelector('.phold').textContent;
    passageHold.set({ ...HOLD, held: [HOLD.held[0]] });
    await tick();
    const outside = host.querySelector('.phold').textContent;
    expect(onScreen).not.toBe(outside);
    expect(onScreen.toLowerCase()).toMatch(/screen|showing|already/);
  });

  it('prints no percentage, ever', async () => {
    await mount();
    passageHold.set(HOLD);
    await tick();
    expect(host.querySelector('.phold').textContent).not.toMatch(/%/);
  });

  it('wears the caution ink and neither promise colour', () => {
    // A hold warns and promises nothing about a screen — the textbook caution.
    // Amber is ON AIR, amethyst is rehearsal, and cyan is spent on the guess
    // marks three rules away in this same card.
    const style = DOCK.slice(DOCK.indexOf('<style>'));
    const rule = style.slice(style.indexOf('  .phold {'), style.indexOf('}', style.indexOf('  .phold {')));
    expect(rule, 'no .phold rule at all').not.toBe('');
    expect(rule).toMatch(/var\(--v-caution/);
    expect(rule).not.toMatch(/var\(--v-amber|var\(--v-amethyst/);
  });

  it('and it goes away with the hold, rather than outliving the reading', async () => {
    await mount();
    passageHold.set(HOLD);
    await tick();
    expect(host.querySelector('.phold')).toBeTruthy();
    // `capture.js` expires it after HOLD_TTL_MS; the card must follow the store
    // rather than keeping its own copy — a component-local copy of a fact the
    // store owns is the defect `detection://thresholds` was added to end.
    passageHold.set(null);
    await tick();
    expect(host.querySelector('.phold'), 'the line outlived the hold').toBeNull();
    expect(get(passageHold)).toBeNull();
  });
});
