// RG-09 — the fallbacks that already existed, made visible.
//
// Relay degraded gracefully in half a dozen places and every one was invisible: the
// denoiser switching itself off on a microphone that will not run at 48 kHz, a
// CPU-only build decoding three times slower, no speech model at all. In each case
// Relay knew and the operator did not — so the symptom ("it isn't hearing
// anything") got attributed to the AI being bad, which is the most expensive
// possible misdiagnosis for this product.
//
// The two rules these tests hold:
//   1. Nothing is invented. A row appears only when a fact Relay measured says so.
//   2. Every row says what it means AND what to do.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { degradations, worstLevel, summarise, LEVELS } from './degraded.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

/** A healthy, listening machine. */
const OK = {
  sttLoaded: true,
  detectionOn: true,
  capturing: true,
  safeMode: false,
  denoise: true,
  gpuBackends: ['Metal'],
  macos: true,
  droppedPartials: 0,
  screensDown: [],
};

describe('nothing is invented', () => {
  it('a healthy machine reports nothing at all', () => {
    expect(degradations(OK)).toEqual([]);
    expect(worstLevel([])).toBeNull();
    expect(summarise([])).toBe('');
  });

  it('says nothing about a fact it has not got', () => {
    // Called before the first audio frame, before `system_hardware` answers, on a
    // plain browser with no backend. An advisory that fires on `undefined` is an
    // advisory an operator learns to scroll past.
    const unknown = degradations({ capturing: false });
    expect(unknown).toEqual([]);
  });

  it('does not claim the denoiser is off merely because nothing has measured it', () => {
    expect(degradations({ ...OK, denoise: undefined })).toEqual([]);
    expect(degradations({ ...OK, denoise: null })).toEqual([]);
    expect(degradations({ ...OK, denoise: false }).map((d) => d.id)).toEqual(['denoise']);
  });

  it('does not claim a CPU-only build before the build has been asked about', () => {
    expect(degradations({ ...OK, gpuBackends: null })).toEqual([]);
    expect(degradations({ ...OK, gpuBackends: [] }).map((d) => d.id)).toEqual(['gpu']);
  });
});

describe('what counts as blocked, and what counts as reduced', () => {
  it('no speech model is BLOCKED — and is described as a working manual tool', () => {
    // Not an error. The difference between an operator who carries on firing by
    // hand and one who assumes Relay is broken and stops.
    const [d] = degradations({ ...OK, sttLoaded: false });
    expect(d.level).toBe('blocked');
    expect(d.what).toMatch(/by hand works exactly as normal/i);
    expect(d.fix).toMatch(/download a speech model/i);
  });

  it('safe mode is BLOCKED and says where to turn it off', () => {
    const [d] = degradations({ ...OK, safeMode: true });
    expect(d.id).toBe('safemode');
    expect(d.level).toBe('blocked');
    expect(d.fix).toMatch(/Backup & Recovery/);
  });

  it('detection being off only counts while the microphone is live', () => {
    // Detection disarmed with nothing playing into it is not a degradation, it is
    // Tuesday. Reporting it would put a permanent caveat on an idle console.
    expect(degradations({ ...OK, detectionOn: false, capturing: false })).toEqual([]);
    expect(degradations({ ...OK, detectionOn: false }).map((d) => d.id)).toEqual(['detection']);
  });

  it('detection off is not reported on top of "no model" — one cause, one row', () => {
    // With no model there is nothing to detect from; two rows would send an
    // operator chasing a toggle that is not the problem.
    const ids = degradations({ ...OK, sttLoaded: false, detectionOn: false }).map((d) => d.id);
    expect(ids).toEqual(['stt']);
  });

  it('a slower build is REDUCED, and admits there is nothing to be done', () => {
    const [d] = degradations({ ...OK, gpuBackends: [] });
    expect(d.level).toBe('reduced');
    expect(d.fix).toMatch(/nothing you can change/i);
  });

  it('shed work says what was NOT lost', () => {
    // A count with no context reads as "Relay is dropping your service".
    const [d] = degradations({ ...OK, droppedPartials: 4 });
    expect(d.title).toMatch(/4 transcript updates skipped/);
    expect(d.what).toMatch(/Nothing final was lost/);
    expect(degradations({ ...OK, droppedPartials: 1 })[0].title).toMatch(/1 transcript update /);
  });

  it('screens that stopped answering are reported here too, and named', () => {
    const [d] = degradations({ ...OK, screensDown: ['Main screen'] });
    expect(d.title).toBe('Main screen is not responding');
    expect(d.what).toMatch(/Relay is still sending/);
    expect(degradations({ ...OK, screensDown: ['A', 'B'] })[0].title).toMatch(/2 screens/);
  });
});

describe('every row is actionable', () => {
  const every = degradations({
    sttLoaded: false,
    detectionOn: false,
    capturing: true,
    safeMode: true,
    denoise: false,
    gpuBackends: [],
    macos: true,
    droppedPartials: 3,
    screensDown: ['Main screen'],
  });

  it('produces one row per distinct cause', () => {
    expect(every.map((d) => d.id)).toEqual([
      'safemode',
      'stt',
      'denoise',
      'gpu',
      'shed',
      'screens',
    ]);
  });

  it('names the consequence and the next action, every time', () => {
    for (const d of every) {
      expect(LEVELS).toContain(d.level);
      expect(d.title.length).toBeGreaterThan(8);
      // "Degraded" on its own is a mood, not information.
      expect(d.what.length).toBeGreaterThan(30);
      expect(d.fix.length).toBeGreaterThan(10);
    }
  });

  it('blocked outranks reduced in the summary', () => {
    expect(worstLevel(every)).toBe('blocked');
    expect(summarise(every)).toMatch(/2 things are unavailable/);
    // One blocked thing is named rather than counted — a count of one is worse
    // than the sentence it replaces.
    expect(summarise(degradations({ ...OK, safeMode: true }))).toBe('Safe mode is on');
  });

  it('a reduced-only machine never reads as unavailable', () => {
    const reduced = degradations({ ...OK, denoise: false, droppedPartials: 2 });
    expect(worstLevel(reduced)).toBe('reduced');
    expect(summarise(reduced)).toMatch(/working, but not fully/);
  });
});

describe('where it is shown', () => {
  const app = read('src/App.svelte');

  it('lives in the shell, so it is right on every tab', () => {
    // A volunteer may well be in Settings when the model fails to load.
    expect(app).toMatch(/degradations\(\{/);
    expect(app).toMatch(/class="deg"/);
  });

  it('is collapsed to one line until opened', () => {
    // A permanent list of caveats across the top of a live console is a list an
    // operator stops reading.
    expect(app).toMatch(/aria-expanded=\{degOpen\}/);
    expect(app).toMatch(/summarise\(degraded\)/);
  });

  it('sits below the panic banner and above the update banners', () => {
    // A panic control that failed outranks everything; "something is working less
    // well" outranks "there is a new version".
    expect(app.indexOf('panicbar')).toBeLessThan(app.indexOf('class="deg"'));
    expect(app.indexOf('class="deg"')).toBeLessThan(app.indexOf('$updateAvailable && !$capturing'));
  });

  it('is never amber', () => {
    // Amber means ON AIR. Nothing here is about what a congregation is looking at.
    const css = read('src/app.css');
    const rule = css.slice(css.indexOf('.deg{'), css.indexOf('.upd.upd-bad{'));
    expect(rule).not.toMatch(/--v-amber/);
    expect(rule).toMatch(/--v-rose/);
  });
});

describe('one poller, one answer', () => {
  it('the channel health poll lives in the store, not in three views', () => {
    // Three timers asking one question would let three surfaces disagree about the
    // same screen for up to two seconds — the asymmetry RG-01 exists to end.
    const store = read('src/lib/stores/capture.js');
    expect(store).toMatch(/export function startChannelHealth/);
    for (const view of ['src/lib/views/Live.svelte', 'src/lib/views/Channels.svelte']) {
      expect(read(view)).not.toMatch(/setInterval\(pollStatus|setInterval\(pollChannelHealth/);
    }
  });

  it('starting it twice does not create a second timer', () => {
    const store = read('src/lib/stores/capture.js');
    const fn = store.slice(store.indexOf('export function startChannelHealth'));
    expect(fn.slice(0, 200)).toMatch(/if \(healthPoll\) return;/);
  });
});
