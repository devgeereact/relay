// WHAT HAPPENED TO A CLAIM, AND WHO DECIDED IT.
//
// `detections` holds only what is still awaiting an operator — correct, and it
// must stay that way. The hole it leaves is the one the product turns on: an
// AUTO-FIRE never enters that list at all. A `Direct` hit above the bar goes
// straight to the screens and the pending suggestion is removed, so the operator
// saw a verse appear on the programme with nothing anywhere saying the AI put it
// there, or what kind of claim it was. Rule 18 asks precisely for that, and on
// the one path where the AI acts alone it was not answered.
//
// `resolvedDetections` is the receipt. These tests pin the three things it could
// get wrong, each of which is a lie of a different shape:
//
//   · calling a HUMAN's fire the AI's (rule 14, in the UI);
//   · saying "put on the screens" over a fire that threw (rule 15);
//   · growing without bound on a run surface.
//
//   npx vitest run src/lib/resolvedclaims.test.js
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
const listeners = new Map();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (name, cb) => {
    listeners.set(name, cb);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

const claim = (over = {}) => ({
  reference: 'Romans 8:28',
  method: 'Direct',
  confidence: 0.91,
  text: 'And we know that all things work together for good',
  in_library: true,
  status: 'suggested',
  ...over,
});

let store;
beforeEach(async () => {
  vi.resetModules();
  invoke.mockReset();
  listeners.clear();
  store = await import('./stores/capture.js');
});

/** Push one claim into the pending list the way the backend event does. */
async function arrive(d) {
  await store.startCapture().catch(() => {});
  const fire = listeners.get('detection://match');
  expect(fire, 'the detection listener is attached').toBeTypeOf('function');
  fire({ payload: d });
}

describe('the receipt names who acted', () => {
  it('AN AUTO-FIRE LEAVES A CARD — it is the only record the AI acted alone', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim({ status: 'auto' }));
    const log = get(store.resolvedDetections);
    expect(log).toHaveLength(1);
    expect(log[0]).toMatchObject({ reference: 'Romans 8:28', outcome: 'auto', method: 'Direct' });
    // And it does NOT sit in the pending list — an auto-fire is not an offer.
    expect(get(store.detections)).toHaveLength(0);
  });

  // The SAME event carries a manual fire, with status 'manual'. Writing that up
  // as the AI's decision is `persist_fire`'s bug (rule 14) reproduced in the UI —
  // and the one it would mislead is the operator who just pressed the button.
  it('a MANUAL fire is never written up as the AI — same event, different actor', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim({ status: 'manual' }));
    expect(get(store.resolvedDetections)).toHaveLength(0);
  });

  it('a still-pending suggestion writes no receipt — nothing has happened to it', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim());
    expect(get(store.detections)).toHaveLength(1);
    expect(get(store.resolvedDetections)).toHaveLength(0);
  });

  it('accepting records the OPERATOR, and carries the claim it was about', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim());
    invoke.mockResolvedValue({ auto_fire: 0.5, suggest: 0.35 });
    await store.confirmDetection('Romans 8:28');
    const log = get(store.resolvedDetections);
    expect(log[0]).toMatchObject({ outcome: 'accepted', reference: 'Romans 8:28' });
    // The words survive the round trip — the card still has something to show.
    expect(log[0].text).toMatch(/all things work together/);
    expect(get(store.detections)).toHaveLength(0);
  });

  // RULE 15, IN ANOTHER COAT. `confirmDetection`'s own doc comment records this
  // exact failure happening to the card: the operator pressed A, the card
  // vanished, a toast said it was live, and the wall was unchanged.
  it('A FIRE THAT THREW LEAVES NO RECEIPT — and the claim stays pending', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim());
    invoke.mockRejectedValue(new Error('screens refused it'));
    await expect(store.confirmDetection('Romans 8:28')).rejects.toThrow();
    expect(get(store.resolvedDetections)).toHaveLength(0);
    expect(get(store.detections)).toHaveLength(1);
  });

  it('dismissing records the operator too — a dismissal is a decision', async () => {
    invoke.mockResolvedValue({});
    await arrive(claim());
    await store.dismissDetection('Romans 8:28');
    expect(get(store.resolvedDetections)[0]).toMatchObject({
      outcome: 'dismissed',
      reference: 'Romans 8:28',
    });
  });

  it('is BOUNDED, newest first, and holds one entry per reference', async () => {
    invoke.mockResolvedValue({});
    await store.startCapture().catch(() => {});
    const fire = listeners.get('detection://match');
    for (const r of ['A 1:1', 'B 2:2', 'C 3:3', 'D 4:4', 'E 5:5'])
      fire({ payload: claim({ reference: r, status: 'auto' }) });
    const log = get(store.resolvedDetections);
    expect(log).toHaveLength(store.MAX_RESOLVED);
    expect(log[0].reference).toBe('E 5:5');
    // The same verse arriving twice is one receipt, not two.
    fire({ payload: claim({ reference: 'E 5:5', status: 'auto' }) });
    expect(get(store.resolvedDetections).filter((x) => x.reference === 'E 5:5')).toHaveLength(1);
  });
});
