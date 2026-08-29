// RG-16 — practice, and the thing it deliberately is not.
//
// It is NOT a simulation of a service. Relay cannot produce a sermon: there is no
// preacher, no room, and no way to synthesise speech offline. Anything claiming to
// simulate one would be teaching a volunteer the shape of a fake.
//
// It is drills with the REAL controls, on the REAL surfaces, in rehearsal — so the
// muscle memory is built on the actual key. Three things these tests hold:
//
//   1. The panic controls come FIRST. Every sketch of operator training this
//      product has produced put "accept a suggestion" first, and that is backwards.
//   2. A drill is satisfied only when the operator does THAT drill.
//   3. A partial run is reported as a partial run.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import fs from 'node:fs';
import path from 'node:path';
import * as training from './training.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const store = await import('./stores/capture.js');
const { practice, startPractice, stopPractice } = await import('./practice.js');

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

beforeEach(() => {
  invoke.mockReset();
  store.capture.update((s) => ({ ...s, available: true }));
  store.serviceLock.set({ engaged: false, held_back: [] });
  store.rehearsing.set(false);
  practice.set({ session: training.newSession(), wasRehearsing: false, error: '' });
});

describe('the curriculum', () => {
  it('teaches the two that save a service FIRST', () => {
    // An operator who can clear a screen is safe to leave alone. One who can fire
    // beautifully and freezes when the wrong thing is up is not.
    expect(training.DRILLS.map((d) => d.id).slice(0, 2)).toEqual(['clear', 'black']);
  });

  it('every drill says what to do, not just what to achieve', () => {
    for (const d of training.DRILLS) {
      expect(d.title.length).toBeGreaterThan(10);
      expect(d.hint.length).toBeGreaterThan(20);
      expect(typeof d.check).toBe('function');
    }
  });

  it('teaches that dismissing is not a failure', () => {
    // A volunteer who believes dismissing is a mistake will accept suggestions they
    // do not want, which is worse for a congregation than a blank screen.
    expect(training.DRILLS.find((d) => d.id === 'suggestion').hint).toMatch(
      /not a failure/,
    );
  });

  it('asks the operator to LOOK at the rehearsal indicator rather than toggle it', () => {
    // They are already in rehearsal. Asking them to turn it on would teach the
    // wrong reflex; what they need is to have looked at the indicator once.
    const d = training.DRILLS.find((x) => x.id === 'rehearsal');
    expect(d.check({ kind: 'acknowledge' })).toBe(true);
    expect(d.check({ kind: 'clear' })).toBe(false);
  });
});

describe('only the current drill can be satisfied', () => {
  it('advances one at a time, in order', () => {
    let s = training.start(0);
    expect(training.current(s).id).toBe('clear');
    s = training.observe(s, { kind: 'clear' });
    expect(training.current(s).id).toBe('black');
    expect(s.done).toEqual(['clear']);
  });

  it('ignores an action for a LATER drill', () => {
    // Letting a later drill complete out of order would let somebody finish the
    // course without ever pressing the control it was there to teach — the failure
    // mode of every checklist that scores itself generously.
    let s = training.start(0);
    s = training.observe(s, { kind: 'dismiss' }); // the fifth drill
    expect(s.done).toEqual([]);
    expect(training.current(s).id).toBe('clear');
  });

  it('ignores everything once the session is over', () => {
    let s = training.stop(training.start(0));
    expect(training.observe(s, { kind: 'clear' })).toEqual(s);
  });

  it('finishes after the last drill', () => {
    let s = training.start(0);
    for (const e of ['clear', 'black', 'content', 'nav', 'dismiss', 'acknowledge'])
      s = training.observe(s, { kind: e });
    expect(s.active).toBe(false);
    expect(training.isComplete(s)).toBe(true);
    expect(training.summary(s)).toMatch(/the whole set/);
  });

  it('a skipped drill is not a done drill', () => {
    let s = training.skip(training.start(0));
    expect(s.done).toEqual([]);
    expect(training.current(s).id).toBe('black');
  });
});

describe('a partial run is reported as a partial run', () => {
  it('never congratulates somebody who skipped the panic drills', () => {
    // Telling them they are ready would be worse than saying nothing, because they
    // would believe it.
    let s = training.start(0);
    s = training.skip(s); // clear
    s = training.skip(s); // black
    s = training.observe(s, { kind: 'content' });
    s = training.stop(s);
    const msg = training.summary(s);
    expect(msg).toMatch(/clearing the screens or the blackout/);
    expect(msg).toMatch(/worth coming back for/);
    expect(msg).not.toMatch(/whole set/);
  });

  it('says plainly when nothing was completed', () => {
    expect(training.summary(training.stop(training.start(0)))).toMatch(/Nothing was completed/);
  });

  it('says nothing at all while it is still running', () => {
    expect(training.summary(training.start(0))).toBe('');
  });
});

describe('it runs in rehearsal, or it does not run', () => {
  it('refuses during a recorded service', () => {
    store.serviceLock.set({ engaged: true, held_back: [] });
    return startPractice().then((ok) => {
      expect(ok).toBe(false);
      expect(get(practice).error).toMatch(/End it before practising/);
      expect(get(practice).session.active).toBe(false);
    });
  });

  it('abandons if rehearsal will not engage — and says why', async () => {
    // The controls are real, so the sandbox has to be too. A drill that put a
    // practice verse on a congregation's wall would be the single most
    // embarrassing bug this product could ship.
    invoke.mockRejectedValue(new Error('poisoned lock'));
    expect(await startPractice()).toBe(false);
    expect(get(practice).error).toMatch(/will not put a practice verse on your screens/);
    expect(get(practice).session.active).toBe(false);
  });

  it('starts when rehearsal takes, and remembers it was off', async () => {
    invoke.mockImplementation(async () => {
      store.rehearsing.set(true);
      return null;
    });
    expect(await startPractice()).toBe(true);
    expect(get(practice).session.active).toBe(true);
    expect(get(practice).wasRehearsing).toBe(false);
  });

  it('puts rehearsal back, and shouts if it cannot', async () => {
    // Leaving the app in rehearsal without telling anybody is how a Sunday morning
    // starts with screens that never light up.
    invoke.mockImplementation(async () => {
      store.rehearsing.set(true);
      return null;
    });
    await startPractice();
    invoke.mockRejectedValue(new Error('nope'));
    await stopPractice();
    expect(get(practice).session.active).toBe(false);
    expect(get(practice).error).toMatch(/could not leave rehearsal/);
    expect(get(practice).error).toMatch(/Turn it off on the Live tab/);
  });

  it('leaves rehearsal ON if it was already on before practice', async () => {
    store.rehearsing.set(true);
    expect(await startPractice()).toBe(true);
    expect(get(practice).wasRehearsing).toBe(true);
    invoke.mockClear();
    await stopPractice();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('the drills watch the real controls', () => {
  it('the store reports the operator’s actions rather than the panel guessing', () => {
    // One stream. Four listeners of its own would be a second set of subscriptions
    // to the same events, and the two would drift about what counts as a clear.
    const s = read('src/lib/stores/capture.js');
    expect(s).toMatch(/export function onOperatorAction/);
    for (const kind of ["'clear'", "'black'", "'content'", "'dismiss'", "'nav'"])
      expect(s).toMatch(new RegExp(`noteOperatorAction\\(${kind}`));
  });

  it('a listener that throws cannot take a live control with it', () => {
    const s = read('src/lib/stores/capture.js');
    const bus = s.slice(s.indexOf('export function noteOperatorAction'));
    expect(bus.slice(0, 400)).toMatch(/try \{/);
    expect(bus.slice(0, 400)).toMatch(/catch/);
  });

  it('the instruction follows the operator between tabs', () => {
    // The drills use controls on Live, so a panel that only rendered on Help would
    // be a panel nobody can read while doing the thing it asks for.
    expect(read('src/App.svelte')).toMatch(/class="prac"/);
    expect(read('src/lib/views/Help.svelte')).toMatch(/Start practising/);
  });

  it('says out loud that it is not a simulation', () => {
    expect(read('src/lib/views/Help.svelte')).toMatch(/cannot simulate a sermon/);
  });
});
