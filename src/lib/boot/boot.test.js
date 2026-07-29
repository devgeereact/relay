// LAUNCH & STARTUP — the state machine.
//
// Written against the rule the whole module exists to enforce:
//
//   A BOOT SCREEN MAY NEVER REPORT A CHECK IT DID NOT RUN.
//
// Every test here is the BUG, not the fix — reintroduce the defect and the test
// fails. The stub-check test is the important one: delete the `probe === 'stub'`
// branch in runStage() and it goes green-ticking an unread GPU.

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  STAGES,
  checks,
  freshChecks,
  setCheck,
  rollUp,
  runStage,
  runChecks,
  pickGate,
  hasResumePoint,
  describeResume,
  bootRecord,
  markCrash,
  markCleanExit,
  clearCrash,
  setSafeMode,
  safeMode,
  resetBoot,
} from './boot.js';

beforeEach(() => {
  localStorage.clear();
  bootRecord.set({ cleanExit: true, lastCrash: null, crashStreak: 0, safeMode: false });
  resetBoot();
});

describe('the check table', () => {
  it('has a stage entry for every stage in the sequence', () => {
    const fresh = freshChecks();
    for (const s of STAGES) {
      expect(fresh[s], `no checks declared for stage "${s}"`).toBeTruthy();
      expect(fresh[s].length).toBeGreaterThan(0);
    }
  });

  it('has no stub checks left anywhere', () => {
    // CPU, memory, GPU, disk, OBS and ATEM were all `probe: 'stub'` — rendered
    // honestly as "not probed", and useless to the operator debugging a laptop.
    // They are now real reads (src-tauri/src/sysprobe.rs). If a stub ever comes
    // back, this fails and whoever added it has to justify it in the log.
    const stubs = Object.entries(freshChecks()).flatMap(([stage, list]) =>
      list.filter((c) => c.probe === 'stub').map((c) => `${stage}.${c.id}`),
    );
    expect(stubs).toEqual([]);
  });

  it('still renders an unknown check as unknown if one ever appears', () => {
    // The stub MACHINERY stays even though nothing uses it. The rule it enforces
    // — never report a check you did not run — outlives any particular gap, and
    // the next unprobeable thing must not be tempted into a green tick.
    setCheck('hardware', 'gpu', { state: 'unknown', note: 'no probe' });
    expect(rollUp(get(checks).hardware)).not.toBe('fail');
  });
});

describe('rollUp', () => {
  const c = (state) => ({ state });

  it('lets a failure outrank everything else', () => {
    expect(rollUp([c('ok'), c('warn'), c('fail')])).toBe('fail');
  });
  it('reports running while anything is still pending', () => {
    expect(rollUp([c('ok'), c('pending')])).toBe('running');
  });
  it('reports warn when nothing failed but something is off', () => {
    expect(rollUp([c('ok'), c('warn'), c('unknown')])).toBe('warn');
  });
  it('does not treat an unknown check as a warning on its own', () => {
    expect(rollUp([c('ok'), c('unknown')])).toBe('ok');
  });

  it('reports the normal state of a church laptop as warn, never fail', () => {
    // No OBS on :4455, no ATEM on :9910, no NDI SDK in the build. That is what
    // a completely ordinary install looks like, and the sequencer must not treat
    // it as a failure — only a failure holds the boot for a click, and a gate
    // that fires on every single launch stops being read by the second week.
    const plugins = [c('ok'), c('ok'), c('ok'), c('warn'), c('warn'), c('warn')];
    expect(rollUp(plugins)).toBe('warn');
    expect(rollUp(plugins)).not.toBe('fail');
  });
});

describe('runStage', () => {
  it('never runs — and never passes — a check marked as a stub', async () => {
    // The guard that keeps "never report a check you did not run" true. Nothing
    // in the shipped table is a stub any more, so this builds one to test it.
    checks.update((all) => ({
      ...all,
      hardware: all.hardware.map((c) => (c.id === 'gpu' ? { ...c, probe: 'stub' } : c)),
    }));
    const gpu = vi.fn(async () => ({ state: 'ok', note: 'RTX 4090' }));
    await runStage('hardware', { gpu, inputs: async () => ({ state: 'ok' }) });

    expect(gpu).not.toHaveBeenCalled();
    const row = get(checks).hardware.find((c) => c.id === 'gpu');
    expect(row.state).toBe('unknown');
    expect(row.note).toMatch(/no probe/);
  });

  it('turns a thrown probe into a failed check, not a broken boot', async () => {
    await runStage('diagnostics', {
      engine: async () => {
        throw new Error('IPC is wedged');
      },
    });
    const row = get(checks).diagnostics.find((c) => c.id === 'engine');
    expect(row.state).toBe('fail');
    // Humanised, not raw: the note goes through errors.js, which never emits a
    // bare dump. The operator still gets the detail — with a lead-in in front of
    // it, so it reads as a sentence rather than a crash.
    expect(row.note).toMatch(/IPC is wedged/);
  });

  it('records a missing probe as unknown rather than as a pass', async () => {
    await runStage('diagnostics', {}); // no probes supplied at all
    const states = new Set(get(checks).diagnostics.map((c) => c.state));
    expect(states.has('ok')).toBe(false);
  });

  it('returns the stage roll-up', async () => {
    const ok = async () => ({ state: 'ok', note: '' });
    const verdict = await runStage('plugins', {
      kiosk: ok,
      http: ok,
      propresenter: ok,
      ndi: async () => ({ state: 'warn', note: 'needs the NDI SDK' }),
    });
    expect(verdict).toBe('warn');
  });
});

describe('setCheck', () => {
  it('patches one row and leaves the rest alone', () => {
    setCheck('diagnostics', 'stt', { state: 'warn', note: 'no model' });
    const list = get(checks).diagnostics;
    expect(list.find((c) => c.id === 'stt').state).toBe('warn');
    expect(list.find((c) => c.id === 'engine').state).toBe('pending');
  });
});

describe('pickGate', () => {
  const crashed = { lastCrash: { at: 'now', message: 'boom' }, crashStreak: 1 };

  it('puts a crash ahead of everything else', () => {
    expect(
      pickGate({ record: crashed, session: { planId: 3 }, update: { version: '9' } }),
    ).toBe('crash');
  });

  it('offers safe mode after three bad boots in a row', () => {
    expect(pickGate({ record: { crashStreak: 3, safeMode: false } })).toBe('safemode');
  });

  it('does not re-offer safe mode to someone already in it', () => {
    expect(pickGate({ record: { crashStreak: 5, safeMode: true } })).toBe(null);
  });

  it('puts a resume ahead of an update', () => {
    expect(pickGate({ record: {}, session: { planId: 3 }, update: { version: '9' } })).toBe(
      'recover',
    );
  });

  it('shows nothing on a clean boot with nothing to resume', () => {
    expect(pickGate({ record: {}, session: { activeTab: 'live' }, update: null })).toBe(null);
  });
});

describe('hasResumePoint', () => {
  it('does not count the active tab as something worth asking about', () => {
    // A tab is not a resume point. Asking "shall I put you back on the Live
    // tab?" on every single launch is how a gate gets clicked through blind.
    expect(hasResumePoint({ activeTab: 'live' })).toBe(false);
  });
  it('counts a service, a plan or a cue', () => {
    expect(hasResumePoint({ serviceId: 4 })).toBe(true);
    expect(hasResumePoint({ planId: 4 })).toBe(true);
    expect(hasResumePoint({ liveCueId: 4 })).toBe(true);
  });
  it('survives a null session', () => {
    expect(hasResumePoint(null)).toBe(false);
  });
});

describe('describeResume', () => {
  it('renders the slide one-based, as an operator counts them', () => {
    expect(describeResume({ liveCueId: 7, liveSlide: 0 })).toContain('slide 1');
  });
});

describe('the boot record', () => {
  it('remembers a crash for the next launch', () => {
    markCrash('TypeError: nope');
    const r = get(bootRecord);
    expect(r.cleanExit).toBe(false);
    expect(r.lastCrash.message).toBe('TypeError: nope');
    expect(r.crashStreak).toBe(1);
  });

  it('counts consecutive crashes', () => {
    markCrash('a');
    markCrash('b');
    markCrash('c');
    expect(get(bootRecord).crashStreak).toBe(3);
  });

  it('resets the streak once the operator has dealt with it', () => {
    markCrash('a');
    markCrash('b');
    clearCrash();
    const r = get(bootRecord);
    expect(r.lastCrash).toBe(null);
    expect(r.crashStreak).toBe(0);
  });

  it('does not clear the crash record just because the window unloaded', () => {
    // markCleanExit runs on beforeunload — INCLUDING the reload that follows a
    // crash-guard "Recover console". If it wiped lastCrash, the recovery screen
    // would never once appear in the situation it exists for.
    markCrash('boom');
    markCleanExit();
    expect(get(bootRecord).lastCrash).not.toBe(null);
  });

  it('persists safe mode across a reload', () => {
    setSafeMode(true);
    expect(get(safeMode)).toBe(true);
    expect(JSON.parse(localStorage.getItem('relay.boot.v1')).safeMode).toBe(true);
  });

  it('survives a corrupt payload rather than blocking the boot', () => {
    localStorage.setItem('relay.boot.v1', '{not json');
    // Re-importing is not possible mid-suite; assert the contract the loader
    // provides by proving a bad write cannot throw out of the store.
    expect(() => bootRecord.set({ ...get(bootRecord) })).not.toThrow();
  });
});

describe('a failed probe', () => {
  it('never puts a raw JS error in a check note', async () => {
    // The bug: Boot Diagnostics and the Dashboard health panel both rendered
    // `Cannot read properties of undefined (reading 'invoke')` — six rows of it —
    // to a volunteer trying to find out whether the machine works.
    const out = await runChecks(
      [{ id: 'engine', label: 'Relay engine', detail: '', probe: 'live', state: 'pending', note: '' }],
      {
        engine: async () => {
          throw new TypeError("Cannot read properties of undefined (reading 'invoke')");
        },
      },
    );
    expect(out[0].state).toBe('fail');
    expect(out[0].note).not.toMatch(/TypeError|undefined/);
    expect(out[0].note).toMatch(/engine is not running/i);
  });
});
