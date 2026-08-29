// RG-15 — the check that walks the whole path, not twenty-one parts of it.
//
// The launch checks ask twenty-one good questions and every one of them is about a
// PART. All twenty-one pass on a machine where nothing works end to end: a
// microphone the operating system has muted, a model that mishears everything, a
// gate calibrated to a room that has since filled with people, an output window on
// a display that is asleep. A church finds that out at 10:31.
//
// Three things these tests hold:
//   1. A stage not reached is an ABSENCE, and only the FIRST one is blamed.
//   2. "Relay recognised something" and "Relay recognised what you said" are
//      different answers with different fixes.
//   3. It runs in rehearsal or it does not run — and it puts the machine back.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import * as walk from './pathcheck.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

/** Drive a complete, successful walk. */
function fullWalk() {
  let w = walk.newWalk();
  w = walk.onStarted(w, 0);
  w = walk.onAudio(w, { level: 0.2, isVoice: true }, 900);
  w = walk.onTranscript(w, { text: 'turn to john chapter three verse sixteen' }, 1400);
  w = walk.onDetection(w, { reference: 'John 3:16', status: 'auto' }, 1700);
  w = walk.onOutput(w, { reference: 'John 3:16' }, 1900);
  return w;
}

describe('the six stages', () => {
  it('a complete walk reaches all of them, in order', () => {
    const w = fullWalk();
    expect(walk.isComplete(w)).toBe(true);
    const { rows, firstMissing } = walk.progress(w);
    expect(rows.map((r) => r.id)).toEqual([
      'microphone',
      'audio',
      'transcript',
      'detection',
      'fire',
      'output',
    ]);
    expect(rows.every((r) => r.state === 'ok')).toBe(true);
    expect(firstMissing).toBeNull();
  });

  it('records the FIRST time a stage worked, not the last', () => {
    // Audio chunks arrive many times a second. Overwriting would report when Relay
    // last heard something, which says nothing about how long it took to start.
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { isVoice: true }, 900);
    w = walk.onAudio(w, { isVoice: true }, 5000);
    expect(w.reached.audio).toBe(900);
  });

  it('a level meter moving is not "Relay heard a voice"', () => {
    // The difference between those two is the whole of DECISIONS §19: the gate is
    // learned, and a room can be loud while no speech ever opens it.
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { level: 0.9, isVoice: false }, 500);
    expect(w.reached.audio).toBeUndefined();
  });

  it('an empty transcript is not a transcript', () => {
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onTranscript(w, { text: '   ' }, 500);
    expect(w.reached.transcript).toBeUndefined();
  });
});

describe('a stage never reached is an absence, and only the first one is blamed', () => {
  it('shows no time for a stage that did not happen — never 0', () => {
    // "0.0s" would read as instantaneous, which is the opposite of the truth.
    let w = walk.onStarted(walk.newWalk(), 0);
    const { rows } = walk.progress(w);
    expect(rows.find((r) => r.id === 'transcript').at).toBeUndefined();
    expect(read('src/lib/views/Dashboard.svelte')).toMatch(/r\.at === undefined \? ''/);
  });

  it('names ONE stage, not five', () => {
    // A check that lists five failures when one thing is broken has told the
    // operator nothing: four of them are consequences of the first.
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { isVoice: true }, 900);
    const v = walk.verdict(w, true);
    expect(v.ok).toBe(false);
    expect(v.sentence).toMatch(/speech model/);
    expect(v.sentence).not.toMatch(/screen is set up/);
  });

  it('blames the microphone when nothing was ever heard', () => {
    const w = walk.onStarted(walk.newWalk(), 0);
    expect(walk.verdict(w, true).sentence).toMatch(/never heard a voice/);
    // …and says the thing an operator would not guess: it listens for speech, not
    // for noise.
    expect(walk.verdict(w, true).sentence).toMatch(/not for noise/);
  });

  it('quotes what it heard when it could not find a reference in it', () => {
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { isVoice: true }, 900);
    w = walk.onTranscript(w, { text: 'john free sixty' }, 1400);
    expect(walk.verdict(w, true).sentence).toMatch(/“john free sixty”/);
  });

  it('treats a suggestion as a CORRECT outcome to report, not a fault', () => {
    // A paraphrase or an uncertain match is only ever offered. Reporting that as a
    // broken pipeline would send somebody to fix the thing that is working.
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { isVoice: true }, 900);
    w = walk.onTranscript(w, { text: 'for god so loved the world' }, 1400);
    w = walk.onDetection(w, { reference: 'John 3:16', status: 'suggested' }, 1600);
    expect(w.reached.detection).toBe(1600);
    expect(w.reached.fire).toBeUndefined();
    expect(walk.verdict(w, true).sentence).toMatch(/usually correct/);
  });

  it('says nothing at all while the walk is still running', () => {
    let w = walk.onStarted(walk.newWalk(), 0);
    expect(walk.verdict(w, false)).toEqual({ ok: null, sentence: '' });
  });
});

describe('"it worked" and "it got the right verse" are separate answers', () => {
  it('confirms both when both are true', () => {
    const v = walk.verdict(fullWalk(), false);
    expect(v.ok).toBe(true);
    expect(v.sentence).toMatch(/got the right verse/);
  });

  it('reports a working pipeline that MISHEARD as working, and says so', () => {
    // A pipeline that works and mishears is a different situation from a broken
    // one, and both are worth knowing before a service — but only one of them is
    // fixed by looking at cables.
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onAudio(w, { isVoice: true }, 900);
    w = walk.onTranscript(w, { text: 'john chapter three verse six' }, 1400);
    w = walk.onDetection(w, { reference: 'John 3:6', status: 'auto' }, 1700);
    w = walk.onOutput(w, { reference: 'John 3:6' }, 1900);
    const v = walk.verdict(w, false);
    expect(v.ok).toBe(true);
    expect(v.sentence).toMatch(/The whole path works/);
    expect(v.sentence).toMatch(/heard “John 3:6”, not John 3:16/);
  });

  it('output alone proves the gate, even if the events arrive out of order', () => {
    let w = walk.onStarted(walk.newWalk(), 0);
    w = walk.onOutput(w, { reference: 'John 3:16' }, 1900);
    expect(w.reached.fire).toBe(1900);
  });
});

describe('it runs in rehearsal, or it does not run', () => {
  const dash = read('src/lib/views/Dashboard.svelte');

  it('turns rehearsal on BEFORE anything else, and abandons if it will not take', () => {
    // The whole point is to fire a real verse through the real pipeline; the whole
    // danger is doing that twenty minutes before a service.
    const fn = dash.slice(dash.indexOf('async function startWalk'));
    expect(fn.indexOf('setRehearsal(true)')).toBeLessThan(fn.indexOf('startCapture'));
    expect(fn).toMatch(/will not fire a verse at your screens to test itself/);
  });

  it('puts the machine back the way it found it', () => {
    // A check that leaves the microphone live, or the app in rehearsal, has created
    // the fault it was looking for.
    const fn = dash.slice(dash.indexOf('async function stopWalk'));
    expect(fn.slice(0, 900)).toMatch(/restoreCapture/);
    expect(fn.slice(0, 900)).toMatch(/restoreRehearsal/);
  });

  it('reports a failure to LEAVE rehearsal, rather than swallowing it', () => {
    const fn = dash.slice(dash.indexOf('async function stopWalk'));
    expect(fn.slice(0, 1200)).toMatch(/could not leave rehearsal/);
  });

  it('is unavailable during a recorded service and in safe mode', () => {
    expect(dash).toMatch(/disabled=\{\$safeMode \|\| \$serviceLock\.engaged/);
  });
});
