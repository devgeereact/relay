// ── RG-116 (P0) · THE PANEL, MOUNTED ────────────────────────────────────────
//
// `modelprice.test.js` proves the sentences are right. This file proves the operator
// gets them, which is a different claim and the one that was false: every figure this
// finding is about already existed — in a frozen audit (`docs/qa/audits/PERF.md`), in
// a field report, and in a register row — and none of it was anywhere near the panel
// where a church picks a model.
//
// Fourteen passing tests were once written against a component nothing rendered
// (CLAUDE.md, Testing). Nothing mounted `ModelSetup.svelte` at all until this file,
// on the surface that decides how well Relay hears a preacher.
//
//   npx vitest run src/lib/modelpanel.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const ModelSetup = (await import('./ModelSetup.svelte')).default;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));

// The shipped catalogue's own shape, with the three cases that matter: the
// recommended model (measured, both currencies), the larger model that costs no
// cadence and has NEVER been scored, and the largest one (measured, four hops).
const CATALOGUE = [
  {
    id: 'base',
    filename: 'ggml-base.bin',
    label: 'Multilingual (recommended)',
    detail: 'Runs on any laptop.',
    bytes: 147_951_465,
    recommended: true,
    installed: true,
    caution: null,
    decode_ms: 59,
    cadence_ms: 200,
    measured_on: 'an Apple Silicon Mac with graphics acceleration',
    accuracy: 'In one real service it heard 5 of 9 spoken references correctly — four wrong verses.',
  },
  {
    id: 'small',
    filename: 'ggml-small.bin',
    label: 'Multilingual, larger',
    detail: 'Three times the download.',
    bytes: 487_601_967,
    recommended: false,
    installed: false,
    caution: null,
    decode_ms: 152,
    cadence_ms: 200,
    measured_on: 'an Apple Silicon Mac with graphics acceleration',
    accuracy: null,
  },
  {
    id: 'large-v3-turbo',
    filename: 'ggml-large-v3-turbo.bin',
    label: 'Largest',
    detail: 'A 1.6 GB download.',
    bytes: 1_624_555_275,
    recommended: false,
    installed: false,
    caution: null,
    decode_ms: 597,
    cadence_ms: 800,
    measured_on: 'an Apple Silicon Mac with graphics acceleration',
    accuracy: 'Replayed over 85.5 minutes it got 6 of 8, with 2 wrong verses.',
  },
];

let host;
beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_models') return Promise.resolve(CATALOGUE);
    if (cmd === 'find_model_files') return Promise.resolve([]);
    if (cmd === 'service_lock') return Promise.resolve({ engaged: false });
    return Promise.resolve(null);
  });
  host = document.createElement('div');
  document.body.appendChild(host);
});
afterEach(() => host.remove());

/** The card for one catalogued model, found by the label an operator reads. */
function cardFor(label) {
  return [...host.querySelectorAll('.ms-opt')].find((c) =>
    c.querySelector('.ms-opt-t')?.textContent.includes(label),
  );
}

describe('every model states its price', () => {
  it('the update rate is on the card, not in an audit document', async () => {
    new ModelSetup({ target: host, props: {} });
    await settle();
    const base = cardFor('Multilingual (recommended)');
    expect(base, 'the recommended model has no card at all').toBeTruthy();
    expect(base.textContent).toContain('5 transcript updates a second');
  });

  it('THE LARGEST MODEL NAMES THE CADENCE IT TAKES — rule 32, in the product', async () => {
    // "A church that chose `turbo` chose a quarter of the cadence and nothing told
    // them." This is the line that tells them, on the panel where they choose.
    new ModelSetup({ target: host, props: {} });
    await settle();
    const turbo = cardFor('Largest');
    expect(turbo.textContent).toContain('1.25 transcript updates a second');
    expect(turbo.textContent).toContain('4× slower to update');
  });

  it('the larger model that is free says it is free', async () => {
    new ModelSetup({ target: host, props: {} });
    await settle();
    expect(cardFor('Multilingual, larger').textContent).toContain('costs no speed at all');
  });

  it('a measured wrong-verse count reaches the card', async () => {
    new ModelSetup({ target: host, props: {} });
    await settle();
    const base = cardFor('Multilingual (recommended)');
    expect(base.textContent).toContain('Wrong verses');
    expect(base.textContent).toContain('5 of 9');
  });

  it('AND AN UNMEASURED ONE ADMITS IT ON THE CARD, in its own line', async () => {
    // The one that matters most. A card with no accuracy line reads as a card with
    // nothing to worry about — and `small` is the model Relay's own lag warning
    // points at, whose only accuracy figure was withdrawn as invalid.
    new ModelSetup({ target: host, props: {} });
    await settle();
    const small = cardFor('Multilingual, larger');
    const line = [...small.querySelectorAll('.ms-price')].find((p) =>
      p.textContent.includes('Wrong verses'),
    );
    expect(line, 'the never-measured model has no wrong-verse line').toBeTruthy();
    expect(line.textContent).toContain('Never measured');
    expect(line.className).toContain('unknown');
  });
});

describe('the summary line does not re-make the claim it replaced', () => {
  it('it no longer says a bigger model hears more accurately', async () => {
    // The exact sentence that shipped: "A bigger model hears more accurately but
    // needs a faster computer." Two unmeasured claims, on the screen that decides how
    // well Relay hears a preacher. Word error rate has never been measured in any
    // language (`docs/LANGUAGES.md`) and this panel may not imply otherwise.
    new ModelSetup({ target: host, props: {} });
    await settle();
    const intro = host.querySelector('.ms-sub');
    expect(intro, 'the panel has no summary line').toBeTruthy();
    for (const phrase of ['hears more accurately', 'more accurate', 'needs a faster computer']) {
      expect(intro.textContent.toLowerCase()).not.toContain(phrase);
    }
    // And it points at the per-model evidence rather than asserting anything itself.
    expect(intro.textContent).toContain('update rate');
  });
});
