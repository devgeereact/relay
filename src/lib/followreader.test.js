// ── FOLLOW THE READER · THE CHURCH'S ONE SWITCH (DECISIONS §118) ─────────────
//
// The operator's instruction of 2026-09-23 was that a verse Relay hears being
// READ goes up without being asked for, and that a church can turn that off. A
// setting with no control is a decision somebody made on the church's behalf; a
// control that writes a preference and leaves the engine doing the opposite is
// rule 15 in another coat.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { get } from 'svelte/store';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const { capture, setFollowTheReader } = await import('./stores/capture.js');

const ROOT = path.resolve(__dirname, '../..');
/** Comments are not the surface — this repository's own scanning rule. */
const SETTINGS = codeOnly(fs.readFileSync(path.join(ROOT, 'src/lib/views/Settings.svelte'), 'utf8'));

describe('the switch exists where the gate is explained', () => {
  it('is rendered in the AI & Detection section, beside the dial', () => {
    const from = SETTINGS.indexOf('Detection sensitivity');
    const to = SETTINGS.indexOf('Voice profiles', from);
    expect(from).toBeGreaterThan(-1);
    expect(to).toBeGreaterThan(from);
    const section = SETTINGS.slice(from, to);
    expect(section).toMatch(/Follow the reader/);
    // THE HOUSE SWITCH, not a hand-rolled one (REBRAND §12, RG-168).
    expect(section).toMatch(/<Switch[\s\S]{0,300}toggleFollowTheReader/);
    // …and it reaches the engine rather than a local flag. The chain is asserted
    // whole: a switch wired to a handler that sets nothing is the shape of the
    // seven dead Settings controls DECISIONS §69 removed.
    expect(SETTINGS).toMatch(/function toggleFollowTheReader\(\)[\s\S]{0,400}setFollowTheReader\(/);
    // A REFUSAL HAS TO BE READ. The command is behind the service lock, so the
    // reachable failure is an operator changing this mid-service — and a switch
    // that springs back in silence is a control that failed without saying so.
    expect(SETTINGS).toMatch(/followErr = humanError\(e\)/);
    expect(section).toMatch(/\{#if followErr\}/);
    // And the WORD beside it, read from the same store the switch throws from,
    // so the control cannot show a state the engine does not hold (rule 35).
    expect(section).toMatch(/\$capture\.followsReader \? 'on' : 'off'/);
  });

  // THE SENTENCE UNDER THE DIAL USED TO PROMISE SOMETHING THAT IS NO LONGER
  // TRUE: "Only a direct, high-confidence quotation can ever auto-fire." A
  // guarantee left standing after it stops holding is the defect, not a wording
  // nit — it is the one sentence on this page an operator would rely on.
  it('no longer claims only a direct match can reach a screen', () => {
    expect(SETTINGS).not.toMatch(/Only a direct, high-confidence quotation can ever auto-fire/);
  });
});

describe('setFollowTheReader', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('applies what the backend landed on, not what was asked for', async () => {
    invoke.mockImplementation(async (_cmd, args) => args.on);
    expect(await setFollowTheReader(false)).toBe(false);
    expect(get(capture).followsReader).toBe(false);
    expect(await setFollowTheReader(true)).toBe(true);
    expect(get(capture).followsReader).toBe(true);
    expect(invoke).toHaveBeenCalledWith('set_follow_the_reader', { on: true });
  });

  // GROUP 1 — IT THROWS. A service-lock refusal, or any other failure, must
  // reach the caller rather than leaving a switch that looks like it moved.
  it('throws rather than leaving the switch showing a position it did not reach', async () => {
    invoke.mockImplementation(async () => {
      throw new Error('a service is recording');
    });
    capture.update((s) => ({ ...s, followsReader: true }));
    await expect(setFollowTheReader(false)).rejects.toThrow(/recording/);
    expect(get(capture).followsReader).toBe(true);
  });
});
