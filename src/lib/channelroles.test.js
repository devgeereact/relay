// WHAT A SCREEN IS FOR — `output_channels.role`, and the three readers of it.
//
// The defect this replaces: Live's programme pane picked "the main screen" with
// `channels.find((c) => c.render_target === 'native_window') ?? channels[0]`, so
// the pane's idea of the wall moved when a channel was renamed, deleted or wired
// differently, and said exactly the same thing whichever of the three had
// happened. Every test below was watched to fail against that expression.
//
//   npx vitest run src/lib/channelroles.test.js
import { describe, it, expect } from 'vitest';
import { programmeScreen, acceptsStageMessage, roleOf } from './channelroles.js';

const ch = (id, name, render_target, role = null) => ({
  id,
  name,
  render_target,
  role,
  template_id: id,
});

const SEEDED = [
  ch(1, 'Main screen', 'native_window', 'main'),
  ch(2, 'Stage display', 'network_client', 'stage'),
  ch(3, 'Streaming', 'network_client'),
  ch(4, 'Lobby screen', 'network_client'),
];

describe('the programme pane reads the role rather than guessing', () => {
  it('names the screen that holds `main`', () => {
    const r = programmeScreen(SEEDED);
    expect(r.channel.name).toBe('Main screen');
    expect(r.byRole).toBe(true);
    expect(r.label).toBe('as Main screen');
  });

  it('follows the role when the main screen is NOT a native window', () => {
    // A church running its wall through OBS has no `native_window` channel at
    // all. The heuristic fell through to `channels[0]` and previewed whatever
    // that happened to be; the role is a setting and says so.
    const obs = [
      ch(3, 'Streaming', 'network_client'),
      ch(9, 'House wall', 'network_client', 'main'),
    ];
    expect(programmeScreen(obs).channel.name).toBe('House wall');
    expect(programmeScreen(obs).byRole).toBe(true);
  });

  it('follows the role when a native window is NOT the main screen', () => {
    // The precise inversion of the old expression: a projector wired as a native
    // window that the operator has NOT made the main screen.
    const rig = [
      ch(1, 'Confidence monitor', 'native_window', 'stage'),
      ch(2, 'House wall', 'network_client', 'main'),
    ];
    expect(programmeScreen(rig).channel.name).toBe('House wall');
  });

  it('still previews something when no screen holds `main` — and SAYS SO', () => {
    // Rule 35: a pane that reads the same when the setting is missing as when it
    // is set is not reporting the setting. The fallback is kept because a blank
    // programme pane is worse; the sentence is what stops it being silent.
    const none = SEEDED.map((c) => ({ ...c, role: null }));
    const r = programmeScreen(none);
    expect(r.channel.name).toBe('Main screen');
    expect(r.byRole).toBe(false);
    expect(r.label).toBe('as Main screen · no main screen set');
  });

  it('is empty, not a crash, on an install with no channels at all', () => {
    expect(programmeScreen([])).toEqual({ channel: null, byRole: false, label: '' });
    expect(programmeScreen(null).channel).toBe(null);
    expect(programmeScreen(undefined).channel).toBe(null);
  });
});

describe('a Stage Message is refused by everything that is not a stage', () => {
  it('accepts only `stage`', () => {
    expect(acceptsStageMessage('stage')).toBe(true);
  });

  it('refuses the main screen, a screen with no role, and anything unknown', () => {
    // The congregation screens of a fresh install arrive here as `null`. A filter
    // whose default is yes is not a filter.
    for (const r of ['main', null, undefined, '', 'Stage', 'STAGE', 0, 1, {}]) {
      expect(acceptsStageMessage(r), `\`${String(r)}\` was let through`).toBe(false);
    }
  });
});

describe('the role map is read the way it arrives on the wire', () => {
  const ROLES = { 1: 'main', 2: 'stage' };

  it('matches a numeric channel id against JSON string keys', () => {
    // JSON object keys are strings; a page parses its channel out of its own URL
    // as a number. Comparing the two without a cast refuses everything, which
    // would look exactly like a filter that works.
    expect(roleOf(ROLES, 2)).toBe('stage');
    expect(roleOf(ROLES, '2')).toBe('stage');
    expect(roleOf(ROLES, 1)).toBe('main');
  });

  it('answers "no role" for a channel nobody named, and for no map at all', () => {
    expect(roleOf(ROLES, 3)).toBe(null);
    expect(roleOf(null, 2)).toBe(null);
    expect(roleOf(undefined, 2)).toBe(null);
    expect(roleOf('{"2":"stage"}', 2)).toBe(null);
  });

  it('treats channel 0 — a raw template preview, on no channel — as no role', () => {
    // `Output.svelte` parses `channel=` to 0 when the URL carries none. A preview
    // that belongs to no screen must not inherit a screen's job.
    expect(roleOf({ 0: 'stage' }, 0)).toBe(null);
  });
});
