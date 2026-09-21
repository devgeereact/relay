// HOW LONG IS LEFT OF THE CLIP — and the refusal that is the point of it.
//
// The figure only ever comes from a SCREEN. The console holds its own player of
// the same file in the programme pane, and timing the clip off that one would
// keep counting while the wall was frozen. That is rule 35 on the one readout an
// operator times the next cue against, which is why `describeMediaClock` takes
// channel rows and nothing else — it has no way to ask the console's own video.
import { describe, it, expect } from 'vitest';
import {
  describeMediaClock,
  DRIFT_TOLERANCE_MS,
  mediaIdFromUrl,
  clipRemainingMs,
} from './mediaclock.js';

const row = (over = {}) => ({
  id: 1,
  name: 'Main screen',
  painting: true,
  media: { pos_ms: 60_000, dur_ms: 240_000, paused: false },
  ...over,
});

describe('when no screen is reporting a clip', () => {
  it('says so in words, rather than showing a dash or a zero', () => {
    // A dash reads as "this clip has no clock". A zero reads as "it has finished".
    // Both are claims, and both are wrong. Words send the operator to look at a
    // screen, which is the correct response.
    const r = describeMediaClock([]);
    expect(r.known).toBe(false);
    expect(r.remainingMs).toBeNull();
    expect(r.text).toMatch(/no screen/i);
  });

  it('a screen that is NOT painting contributes nothing, whatever it last said', () => {
    // A position without a fresh beat behind it is a countdown from a screen that
    // stopped answering, which is precisely what an operator would act on.
    expect(describeMediaClock([row({ painting: false })]).known).toBe(false);
  });

  it('a screen showing a verse contributes nothing either', () => {
    expect(describeMediaClock([row({ media: null })]).known).toBe(false);
  });

  it('and a report with no usable duration is refused here too', () => {
    // Dropped at the page, dropped at the engine, and dropped again here. Three
    // gates on one rule is not duplication: each is the last line for a different
    // caller, and this one also serves rows a test or an older build handed over.
    expect(describeMediaClock([row({ media: { pos_ms: 0, dur_ms: 0 } })]).known).toBe(false);
    expect(describeMediaClock([row({ media: { pos_ms: 5, dur_ms: -1 } })]).known).toBe(false);
  });
});

describe('when a screen is reporting', () => {
  it('says how long is left, not how far in it is', () => {
    const r = describeMediaClock([row()]);
    expect(r.known).toBe(true);
    expect(r.remainingMs).toBe(180_000);
    expect(r.text).toBe('3:00 left');
  });

  it('names the screen it believed', () => {
    expect(describeMediaClock([row()]).from).toBe('Main screen');
  });

  it('clamps a position past the end rather than counting backwards', () => {
    const r = describeMediaClock([row({ media: { pos_ms: 999_999, dur_ms: 5_000 } })]);
    expect(r.remainingMs).toBe(0);
  });

  it('says when it is held, because a clock that is not moving must not look like one that is', () => {
    const r = describeMediaClock([row({ media: { pos_ms: 60_000, dur_ms: 240_000, paused: true } })]);
    expect(r.paused).toBe(true);
    expect(r.text).toMatch(/held/);
  });
});

describe('when several screens report', () => {
  const two = (aRemaining, bRemaining) => [
    row({ id: 1, name: 'Main', media: { pos_ms: 240_000 - aRemaining, dur_ms: 240_000 } }),
    row({ id: 2, name: 'Lobby', media: { pos_ms: 240_000 - bRemaining, dur_ms: 240_000 } }),
  ];

  it('the SHORTEST remaining wins, because that is the question being asked', () => {
    // The operator is preparing for the moment the first screen runs out, not for
    // an average of several.
    const r = describeMediaClock(two(90_000, 30_000));
    expect(r.remainingMs).toBe(30_000);
    expect(r.from).toBe('Lobby');
    expect(r.screens).toBe(2);
  });

  it('ordinary drift between two players is not reported as a fault', () => {
    // Separate players of the same file are never in lockstep, and two screens
    // reporting on different ticks of the same beat are already apart through no
    // fault of their own.
    expect(describeMediaClock(two(60_000, 60_000 - (DRIFT_TOLERANCE_MS - 1))).disagree).toBe(false);
  });

  it('but a real gap IS reported, because that is a screen stalled', () => {
    // This is the half worth having. A screen buffering or stalled is exactly what
    // an operator needs to know before cueing something over it, and averaging it
    // away would hide the one fact worth surfacing.
    expect(describeMediaClock(two(60_000, 10_000)).disagree).toBe(true);
  });

  it('one screen saying it is held is enough to say held', () => {
    // Something an operator pressed has taken effect somewhere. A majority vote
    // would hide that, and the conservative reading is the useful one.
    const rows = two(60_000, 60_000);
    rows[1].media.paused = true;
    expect(describeMediaClock(rows).paused).toBe(true);
  });
});

// ── AND THAT LIVE ASKS THE SCREENS, NOT ITS OWN PLAYER ──────────────────────
//
// The rule above is only worth having if the surface feeds it the right thing.
// Live's programme pane renders through `TemplateRender`, so it holds a second
// `<video>` of the same file — and reading the clock off that one is the exact
// mistake this module exists to prevent. It would count down happily while the
// wall was frozen.
//
// SOURCE-SHAPE, and deliberately: `r2livepath.test.js:290` records that
// `Live.svelte` cannot be mounted meaningfully in this suite, so the rule itself
// is the only instrument available on this side. The behaviour is covered above
// against numbers; this holds the wiring between them.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

describe('the run surface asks the screens', () => {
  it('feeds the clock from channel health, not from its own preview', () => {
    const call = LIVE.match(/describeMediaClock\(([\s\S]*?)\n\s*\);/);
    expect(call, 'Live no longer calls describeMediaClock').not.toBeNull();
    expect(call[1]).toMatch(/channelHealth/);
    // If this ever reads a bound video element or the live content's own fields,
    // the readout has stopped being a fact about the screens.
    expect(call[1]).not.toMatch(/videoEl|currentTime|duration/);
  });

  it('shows the readout only while a clip is what is on the screens', () => {
    // Beside a verse it would be the last thing the PREVIOUS clip said, which is
    // a stale number in a slot an operator reads as current.
    expect(LIVE).toMatch(/mediaLive\s*=\s*!!\$live\?\.media_url/);
    expect(LIVE).toMatch(/\{#if mediaLive\}/);
  });

  it('spends no law colour on it', () => {
    // Amber is ON AIR and this is a fact about a clip, not a claim that a
    // congregation is looking at one. The tag above the pane already makes that
    // claim and is the only thing entitled to.
    const block = LIVE.slice(LIVE.indexOf('.mon-clip{'), LIVE.indexOf('.mon-name{'));
    expect(block).not.toMatch(/--v-amber|--v-amethyst|--v-cyan/);
  });
});

describe('reading the media id back out of a URL the engine built', () => {
  it('finds the id in an imported asset URL', () => {
    expect(mediaIdFromUrl('http://192.168.1.144:8032/media/7')).toBe(7);
    expect(mediaIdFromUrl('http://10.0.0.2:8032/media/812?x=1')).toBe(812);
  });

  it('answers null for a picture Relay SHIPS, which has no row', () => {
    // DECISIONS §90: a bundled asset has no file under `/media/<id>` at all, so
    // there is no id to read and guessing one would address somebody else's row.
    expect(mediaIdFromUrl('http://192.168.1.144:8032/bundled/dark-wood.jpg')).toBeNull();
  });

  it('answers null rather than guessing on anything else', () => {
    expect(mediaIdFromUrl(null)).toBeNull();
    expect(mediaIdFromUrl('')).toBeNull();
    expect(mediaIdFromUrl('http://x/media/abc')).toBeNull();
    expect(mediaIdFromUrl('http://x/media/0')).toBeNull();
  });
});

describe('the run surface can put the clip on the preacher screen too', () => {
  const LIVE_SRC = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  it('sends the id it read back, and takes it off by sending null', () => {
    // `sendStageMedia(null)` is the take-down, the same one door for both
    // directions the engine uses. A separate "clear" call would be a second door
    // onto one piece of state.
    const fn = LIVE_SRC.slice(LIVE_SRC.indexOf('async function toStage'));
    expect(fn.slice(0, 400)).toMatch(/sendStageMedia\(onStage \? null : liveMediaId\)/);
  });

  it('refuses a picture Relay ships rather than guessing an id', () => {
    // DECISIONS §90: a bundled asset has no row under `/media/<id>`, so there is
    // nothing to address. The control says so in its title instead of looking
    // pressable and doing nothing.
    expect(LIVE_SRC).toMatch(/disabled=\{liveMediaId == null\}/);
    expect(LIVE_SRC).toMatch(/A picture Relay ships cannot be sent on its own/);
  });

  it('shows the failure rather than swallowing it', () => {
    // A control that reported a success it did not achieve is the failure
    // `panic.test.js` exists for one surface up.
    const fn = LIVE_SRC.slice(LIVE_SRC.indexOf('async function toStage'));
    expect(fn.slice(0, 400)).toMatch(/clipErr = humanError\(e\)/);
  });
});

// ── THE PREACHER'S HALF OF THE SAME QUESTION (RG-213) ────────────────────────
//
// The operator's words: *"on the side can you have the media countdown so the
// stage screen tells the preacher when the media is almost done and they can be
// well prepared for next action"*.
//
// `describeMediaClock` above answers this for the OPERATOR, from what the
// congregation screens report on their beat. The stage page cannot ask that
// question: it has no channel health, it is a client like any other. What it
// does have is the clip itself, playing in front of the preacher — so the rule
// is about a player rather than about a set of screens, and it is a separate
// function for exactly that reason. Collapsing them would mean one of the two
// surfaces reading a number that is not about the picture it is showing.
describe('clipRemainingMs — the clock on the preacher’s own copy', () => {
  it('is what is left of the clip, in milliseconds', () => {
    expect(clipRemainingMs({ duration: 90, currentTime: 30 })).toBe(60_000);
  });

  it('is null when the player cannot yet say — never a zero', () => {
    // A zero reads as "it has finished", which is the one thing it must not say
    // about a clip that has not started. Every shape a video element takes
    // before `loadedmetadata` is an absence.
    expect(clipRemainingMs(null)).toBeNull();
    expect(clipRemainingMs({})).toBeNull();
    expect(clipRemainingMs({ duration: NaN, currentTime: 0 })).toBeNull();
    expect(clipRemainingMs({ duration: 0, currentTime: 0 })).toBeNull();
    // A live stream has no end, so there is nothing to count down to.
    expect(clipRemainingMs({ duration: Infinity, currentTime: 12 })).toBeNull();
  });

  it('never goes below zero, and a finished clip says zero rather than nothing', () => {
    expect(clipRemainingMs({ duration: 10, currentTime: 10 })).toBe(0);
    expect(clipRemainingMs({ duration: 10, currentTime: 11.4 })).toBe(0);
  });

  it('a looping clip still counts to the end of THIS pass', () => {
    // It is the right answer for the question being asked: the preacher wants to
    // know when the picture in front of them comes round again, and a loop that
    // reported `null` would leave the one figure they are watching blank.
    expect(clipRemainingMs({ duration: 20, currentTime: 5, loop: true })).toBe(15_000);
  });

  it('a held clip reports the time left where it stopped, not a falling figure', () => {
    // Nothing here reads the clock, so a paused player simply keeps reporting the
    // same number. Asserted because it is a promise the stage rail relies on.
    const held = { duration: 60, currentTime: 12, paused: true };
    expect(clipRemainingMs(held)).toBe(48_000);
    expect(clipRemainingMs(held)).toBe(48_000);
  });
});
