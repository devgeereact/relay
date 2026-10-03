// A CUE'S SCREEN SET REACHES ALL FIVE CUE KINDS, OR IT REACHES NONE OF THEM
// HONESTLY.
//
// RG-161 gave a plan cue a `Screens` row, and `ServicePlanner.svelte` does not gate
// that row by cue type — it renders for all five kinds. Three of the five passed
// the set on to the backend. **Media and countdown did not**, because `fire_media`
// and `start_countdown` never took the argument, so `OutputContent.channels` was
// left `None` by `..Default::default()`.
//
// What that looked like to an operator: tick one screen of three, read the hint
// underneath saying "Other screens keep what they are showing", fire the cue, and
// watch all three take it. A control reporting a success it did not achieve, which
// is the failure `panic.test.js` exists for one surface up.
//
// It is the fourth time in this repository that a guarantee has been kept on some
// doors and skipped on its twin — rehearsal gating held on three of four kiosk
// publishers, the throw-vs-swallow contract on eight of nine group-1 wrappers, and
// `NavResult` was thrown away by `remote_api`. CLAUDE.md's instruction after the
// third was: enumerate every caller of the thing you fixed, and write the test on
// the surface that was missed. So this file asks about the KIND, not the command.
//
// TWO KINDS OF CHECK, and the second one needs its reason stated. The wrappers are
// exercised for real against a mocked bridge. The Live call sites are read as
// source, because `Live.svelte` cannot be mounted meaningfully in this suite at all
// — `r2livepath.test.js:290` records why, and `liveacceptance.test.js` reads the
// same file the same way for the same reason. A source-shape check does not prove
// runtime behaviour; the three `e2e.rs` cases beside this one do that for the
// commands. This one holds the wiring between them.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { fireMedia, startCountdown } = await import('./stores/capture.js');

const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

describe('the two wrappers that could not express a screen set', () => {
  beforeEach(() => {
    // A block body. `beforeEach(() => invoke.mockReset())` returns the mock, and
    // vitest treats a value returned from a hook as a teardown function — so it
    // would call `invoke()` after every test in this file.
    invoke.mockReset();
  });

  it('fireMedia carries the screens it was given', async () => {
    await fireMedia(7, null, true, [4]);
    expect(invoke).toHaveBeenCalledWith('fire_media', {
      id: 7,
      templateId: null,
      channels: [4],
    });
  });

  it('fireMedia says null when it was given no screens, which is every screen', async () => {
    // NOT `[]`. An empty list is a real answer meaning "no screen", and a media
    // cue written before targeting existed means the opposite of that.
    await fireMedia(7, null, true);
    expect(invoke).toHaveBeenCalledWith('fire_media', {
      id: 7,
      templateId: null,
      channels: null,
    });
  });

  it('startCountdown carries the screens it was given', async () => {
    await startCountdown(5, 'Service begins in', 'Welcome', null, true, null, null, [4]);
    expect(invoke).toHaveBeenCalledWith(
      'start_countdown',
      expect.objectContaining({ channels: [4] }),
    );
  });

  it('startCountdown says null when it was given no screens', async () => {
    await startCountdown(5, 'Service begins in', 'Welcome', null, true);
    expect(invoke).toHaveBeenCalledWith(
      'start_countdown',
      expect.objectContaining({ channels: null }),
    );
  });
});

describe('every plan cue kind hands its screen set to the backend', () => {
  // The block in `Live.svelte` that fires one plan cue: it computes `cueChannels`
  // once, then branches five ways. Slicing it rather than searching the whole file
  // keeps an unrelated later mention of the same identifier from answering for a
  // branch that does not pass it.
  const block = (() => {
    const start = LIVE.indexOf('const cueChannels = planChannelsOf(item.channels_json)');
    expect(start, 'the plan-fire block moved — this test is reading nothing').toBeGreaterThan(-1);
    const end = LIVE.indexOf("item.cue_type === 'song'", start);
    return LIVE.slice(start, LIVE.indexOf('}', end));
  })();

  it('the media branch passes cueChannels to fireMedia', () => {
    const call = block.match(/fireMedia\([^)]*\)/);
    expect(call, 'the media branch no longer calls fireMedia').not.toBeNull();
    expect(call[0]).toMatch(/cueChannels/);
  });

  it('the countdown branch passes cueChannels to startCountdown', () => {
    const call = block.match(/startCountdown\(([\s\S]*?)\n\s*\);/);
    expect(call, 'the countdown branch no longer calls startCountdown').not.toBeNull();
    expect(call[1]).toMatch(/cueChannels/);
  });

  it('and the three that already did have not lost it', () => {
    // The half that must not regress. These three were correct before this change
    // and are the reason the other two looked correct too.
    expect(block.match(/manualFire\([^)]*\)/)[0]).toMatch(/cueChannels/);
    const content = [...block.matchAll(/fireContent\([^)]*\)/g)].map((m) => m[0]);
    expect(content.length, 'the song and announce branches').toBeGreaterThan(0);
    for (const c of content) expect(c).toMatch(/cueChannels/);
  });
});
