// ONE GATE CONTROL — the dial, and nothing else that sets the same fact.
//
// Settings → AI & Detection used to carry a second control over the detection
// gate: two sliders writing `auto_fire` and `suggest` directly, against the
// 0-100 dial on Live. They fought in four measurable ways (DECISIONS §96) —
// opposite directions, a range the dial cannot express, an inverse that discards
// `suggest`, and a label reading HYPER-AWARE at the end where Relay makes the
// FEWEST suggestions.
//
// Two controls over one fact cannot be reconciled by syncing, because every sync
// makes one of them lie. So this file holds the shape of the fix rather than the
// bug: ONE control, reached from two doors, over one value.
//
//   npx vitest run src/lib/onegate.test.js
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { codeOnly } from './codeonly.js';
import fs from 'node:fs';
import path from 'node:path';
import { get } from 'svelte/store';

const invoke = vi.fn();
const listeners = new Map();
const listen = vi.fn(async (name, fn) => {
  listeners.set(name, fn);
  return () => {};
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const { capture, initAudio, setSensitivity } = await import('./stores/capture.js');

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
/** Comments are not the surface — this repository's own scanning rule. */
const strip = (s) => codeOnly(s);

const SETTINGS = strip(read('src/lib/views/Settings.svelte'));
const DOCK = strip(read('src/lib/Dock.svelte'));
const STORE = strip(read('src/lib/stores/capture.js'));

describe('the gate has one control', () => {
  // The deleted door stays deleted. A registered command with no rendered path
  // to it is attack surface nobody is watching, which is why the command went
  // with the sliders rather than being left "in case".
  it('has no wrapper and no caller for the deleted two-slider command', () => {
    for (const [name, src] of [
      ['Settings.svelte', SETTINGS],
      ['Dock.svelte', DOCK],
      ['capture.js', STORE],
    ]) {
      expect(src, `${name} still reaches set_thresholds`).not.toMatch(/set_thresholds/);
      expect(src, `${name} still reaches setThresholds`).not.toMatch(/setThresholds/);
    }
  });

  // BOTH DOORS WRITE THE SAME COMMAND. Not "a similar one", and not each other:
  // `set_sensitivity` is the only hand-set path to the gate, and it is what makes
  // `apply_thresholds` — the gate, the anchor and the profile row, together —
  // unavoidable (R4-10).
  it('sends both doors through the one command', () => {
    expect(SETTINGS).toMatch(/setSensitivity\(/);
    expect(DOCK).toMatch(/setSensitivity\(/);
    expect(STORE).toMatch(/call\('set_sensitivity'/);
  });

  // The two thresholds are still SHOWN — an operator asking what a dial position
  // actually did has nowhere else to look — but shown as results. A `range` input
  // is the difference between a read-out and a second control, so the count of
  // them in this section is the thing worth pinning.
  it('renders the thresholds as read-outs, never as inputs', () => {
    const from = SETTINGS.indexOf('Detection sensitivity');
    const to = SETTINGS.indexOf('Voice profiles', from);
    expect(from).toBeGreaterThan(-1);
    expect(to).toBeGreaterThan(from);
    const section = SETTINGS.slice(from, to);

    const ranges = section.match(/type="range"/g) ?? [];
    expect(ranges.length, 'the gate section has more than one slider again').toBe(1);
    expect(section).toMatch(/aria-label="Detection sensitivity"/);
    // The figures, as definition data rather than form controls.
    expect(section).toMatch(/<dl[^>]*class="s-gatedl"/);
    expect(section).toMatch(/<dt>Auto-fire needs<\/dt>/);
    expect(section).toMatch(/<dt>Suggest needs<\/dt>/);
    // And the label that was backwards is gone rather than merely moved.
    expect(section).not.toMatch(/HYPER-AWARE/);
    // THE WORD "ABOVE" IS GONE WITH THE QUANTITY IT DESCRIBED (DECISIONS §117),
    // AND "NEEDS" REPLACED IT (§121). Two operator complaints, three days apart,
    // about the same pair: first that a figure printed under a slider reads as
    // that slider's setting and this one ran the other way; then, once it had
    // been inverted into a readiness, that a SUGGESTION was the larger number
    // and so read as outranking an auto-fire.
    //
    // "Needs" answers both, because a smaller number under it is plainly the
    // easier bar rather than the keener setting — and auto-fire, being the
    // stricter rule, is the larger figure. The arithmetic did not move; the word
    // did.
    expect(section).not.toMatch(/Auto-fire above|Suggest above/);
    // The scale is named, and so is the RELATIONSHIP. A bare 30 beside a bare 10
    // says nothing about which is the harder bar.
    expect(section).toMatch(/scored 0–100/);
    expect(section).toMatch(/Auto-fire needs 20 points more/);
  });
});

describe('one value, two places', () => {
  beforeEach(() => {
    invoke.mockReset();
    // NOT cleared: `initAudio` wires its listeners once per module (the
    // `outputListenersUp` guard), exactly as it does in the app, so clearing here
    // would test a re-subscription that never happens in a real session.
    capture.set({
      ...get(capture),
      available: false,
      sensitivity: 50,
      sensitivityKnown: false,
      gateOnDial: true,
      thresholds: { auto_fire: 0.5, suggest: 0.35 },
    });
  });

  // THE LAUNCH READ, and the reason it is a read and not only a subscription.
  // `setup` applies the active profile's LEARNED gate before the window exists,
  // so the emit that would have announced it has nobody to reach. A console that
  // only listened would open showing a learned gate drawn at whatever dial
  // position is nearest it, with no caveat anywhere — the exact state §96 exists
  // to report.
  it('takes the whole read-out at launch, drift included', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_thresholds')
        return { auto_fire: 0.596, suggest: 0.35, sensitivity: 38, on_dial: false };
      if (cmd === 'list_audio_devices') return [];
      if (cmd === 'stt_status') return { loaded: false, model: null, language: null };
      if (cmd === 'get_detection_enabled') return true;
      return null;
    });
    await initAudio();
    const s = get(capture);
    expect(s.sensitivity).toBe(38);
    expect(s.sensitivityKnown).toBe(true);
    expect(s.gateOnDial).toBe(false);
    expect(s.thresholds).toEqual({ auto_fire: 0.596, suggest: 0.35 });
  });

  // A launch that could not read the gate must not look like one that did. 50 is
  // both the shipped default and an ordinary real setting.
  it('claims nothing when the launch read fails', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_thresholds') throw new Error('poisoned lock');
      if (cmd === 'list_audio_devices') return [];
      if (cmd === 'stt_status') return { loaded: false, model: null, language: null };
      if (cmd === 'get_detection_enabled') return true;
      return null;
    });
    await initAudio();
    expect(get(capture).sensitivityKnown).toBe(false);
  });

  // MOVING EITHER DOOR MOVES BOTH, with no reload. Both surfaces derive from this
  // one store, so this is the assertion that covers the pair of them: the store
  // is written from what the ENGINE answered, never from what was asked for.
  it('writes the landed position, the thresholds and the drift answer together', async () => {
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'set_sensitivity') return Math.min(args.sensitivity, 80);
      if (cmd === 'get_thresholds')
        return { auto_fire: 0.36, suggest: 0.26, sensitivity: 80, on_dial: true };
      return null;
    });
    const landed = await setSensitivity(95);
    expect(landed).toBe(80);
    const s = get(capture);
    expect(s.sensitivity).toBe(80);
    expect(s.gateOnDial).toBe(true);
    expect(s.thresholds).toEqual({ auto_fire: 0.36, suggest: 0.26 });
  });

  // The learning moves the gate with nobody touching a control, and that is when
  // the drift answer changes. So it rides on the announcement that it moved.
  it('carries the drift answer on the event, and never defaults to the reassuring one', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_thresholds')
        return { auto_fire: 0.5, suggest: 0.35, sensitivity: 50, on_dial: true };
      if (cmd === 'list_audio_devices') return [];
      if (cmd === 'stt_status') return { loaded: false, model: null, language: null };
      if (cmd === 'get_detection_enabled') return true;
      return null;
    });
    await initAudio();
    const fire = listeners.get('detection://thresholds');
    expect(fire, 'nothing is listening for the gate moving').toBeTypeOf('function');

    fire({ payload: { auto_fire: 0.72, suggest: 0.35, sensitivity: 23, on_dial: false } });
    expect(get(capture).gateOnDial).toBe(false);
    expect(get(capture).sensitivity).toBe(23);

    // A malformed frame leaves the last answer alone rather than healing to
    // "on the dial" — the reassuring one is the one that must never be guessed.
    fire({ payload: { auto_fire: 0.72, suggest: 0.35 } });
    expect(get(capture).gateOnDial).toBe(false);
  });
});
