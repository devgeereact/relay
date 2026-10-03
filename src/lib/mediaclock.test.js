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
// THE TRANSPORT MOVED TO THE SHELL (RG-237). A clip plays on every screen in the
// building whatever workspace the operator is on, so its controls sit beside
// Clear screens rather than on one surface out of six. The rule below did not
// change — only which file asks it.
// AND THEN OUT OF THE DOCK AGAIN (RG-254). It is a strip of its own in the
// shell now, rendered only while a clip is on the screens — the Controls card
// may never scroll, and it was spending most of its room on a transport for a
// thing that is not playing for most of a service. The rules below are
// unchanged; only the file that answers them is.
const DOCK = readFileSync(resolve(__dirname, 'ClipBar.svelte'), 'utf8');

describe('the shell asks the screens', () => {
  it('feeds the clock from channel health, not from its own preview', () => {
    const call = DOCK.match(/describeMediaClock\(([\s\S]*?)\n\s*\);/);
    expect(call, 'nothing calls describeMediaClock').not.toBeNull();
    expect(call[1]).toMatch(/channelHealth/);
    // If this ever reads a bound video element or the live content's own fields,
    // the readout has stopped being a fact about the screens.
    expect(call[1]).not.toMatch(/videoEl|currentTime|duration/);
  });

  it('shows the readout only while a clip is what is on the screens', () => {
    // Beside a verse it would be the last thing the PREVIOUS clip said, which is
    // a stale number in a slot an operator reads as current.
    expect(DOCK).toMatch(/clipLive\s*=\s*!!\$live\?\.media_url/);
    expect(DOCK).toMatch(/\{#if clipLive\}/);
  });

  it('spends no law colour on it', () => {
    // Amber is ON AIR and this is a fact about a clip, not a claim that a
    // congregation is looking at one. The tag above the pane already makes that
    // claim and is the only thing entitled to.
    const block = DOCK.slice(DOCK.indexOf('.clipbar {'), DOCK.indexOf('.ctlbody {'));
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
    const fn = DOCK.slice(DOCK.indexOf('async function toStage'));
    expect(fn.slice(0, 400)).toMatch(/sendStageMedia\(clipOnStage \? null : clipMediaId\)/);
  });

  it('refuses a picture Relay ships rather than guessing an id', () => {
    // DECISIONS §90: a bundled asset has no row under `/media/<id>`, so there is
    // nothing to address. The control says so in its title instead of looking
    // pressable and doing nothing.
    // In the dock since RG-237, with the rest of the transport.
    expect(DOCK).toMatch(/disabled=\{clipMediaId == null\}/);
    expect(DOCK).toMatch(/A picture Relay ships cannot be sent on its own/);
  });

  it('shows the failure rather than swallowing it', () => {
    // A control that reported a success it did not achieve is the failure
    // `panic.test.js` exists for one surface up.
    const fn = DOCK.slice(DOCK.indexOf('async function toStage'));
    expect(fn.slice(0, 400)).toMatch(/err = humanError\(e\)/);
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

// ── AND WHERE THE CLIP IS, WHICH IS WHAT A SCRUB NEEDS (RG-221) ─────────────
//
// `remainingMs` answers "how long is left", which is the operator's question
// during a service. A scrub asks two more: how long is the clip, and where is it
// now. They come from the same beat, from the same screen — the one the rest of
// this verdict is about — so they belong here rather than in a second reader
// that could pick a different screen.
describe('the clip’s length and position, for the control that moves it', () => {
  const row = (over = {}) => ({
    id: 1,
    name: 'Main screen',
    painting: true,
    media: { dur_ms: 120_000, pos_ms: 30_000, paused: false },
    ...over,
  });

  it('are the SAME screen the rest of the verdict is about', () => {
    const d = describeMediaClock([
      row({ id: 1, name: 'Main screen', media: { dur_ms: 120_000, pos_ms: 30_000 } }),
      // Further through the clip, so it is the one with the least left and the
      // one `from` already names.
      row({ id: 2, name: 'Lobby', media: { dur_ms: 120_000, pos_ms: 100_000 } }),
    ]);
    expect(d.from).toBe('Lobby');
    expect(d.durationMs).toBe(120_000);
    expect(d.positionMs).toBe(100_000);
  });

  it('are absent, never zero, when no screen is reporting a clip', () => {
    // A zero length would make a scrub bar that looks usable and cannot move
    // anything, which is the defect DECISIONS §69 closed seven controls of.
    const d = describeMediaClock([]);
    expect(d.durationMs).toBeNull();
    expect(d.positionMs).toBeNull();
  });
});

// ── AND THE CONTROLS THAT MOVE IT ARE ON THE DESK (RG-221) ──────────────────
//
// The rule is held against numbers above and in `mediafull.test.js`; this holds
// that Live actually offers the two controls, and the two judgements that make
// them usable rather than merely present.
describe('the rest of the transport is reachable from the run surface', () => {
  it('offers a scrub, named for somebody who cannot see it', () => {
    expect(DOCK, 'no scrub on the desk').toMatch(/aria-label="Scrub the clip"/);
  });

  it('and no LEVEL, which the operator asked to be rid of (RG-254)', () => {
    // *"also the level and all i dont think its needed"*. Nothing outside this
    // control ever read it: the wire field and its clamp stay, so a cue could
    // set one later, but the desk offers no control for a decision nobody makes
    // from the desk — the sound comes off the desk, not off the screens.
    //
    // This case used to REQUIRE the slider. It is kept, reversed, rather than
    // deleted: a rule that changed direction on an operator's instruction is
    // worth more as a record than as an absence.
    expect(DOCK, 'the level came back').not.toMatch(/aria-label="Clip volume on the screens"/);
  });

  it('sends the scrub on the DROP, never on every pixel of the drag', () => {
    // `on:input` fires per pixel and each one is a frame to every screen in the
    // building — a drag across a two-minute clip would be hundreds of broadcasts
    // and a wall that stutters while the handle moves.
    //
    // THE RULE IS ABOUT WHAT REACHES A SCREEN, not about which handlers exist,
    // and this case was written the narrower way. There IS an `on:input` since
    // RG-254 and there has to be: it is what keeps the handle under the
    // operator's finger instead of letting a two-second poll snap it back. What
    // matters is that it moves a local variable, and that `seek` — the only
    // thing that reaches the wire — hangs off `change`, which fires on the drop.
    const bar = DOCK.slice(DOCK.indexOf('aria-label="Scrub the clip"'));
    const tag = bar.slice(0, bar.indexOf('/>'));
    expect(tag).toMatch(/on:input=\{\(e\) => \(dragMs = /);
    expect(tag, 'the drag reaches the screens').not.toMatch(/send\(|setMediaTransport/);
    expect(tag).toMatch(/on:change=\{\(e\) => seek\(/);
  });

  it('and neither appears when no screen is reporting a clip', () => {
    // A scrub bar over an unknown length looks usable and can move nothing,
    // which is the defect DECISIONS §69 closed seven Settings controls of.
    // Asked of `clipPosition`'s answer since RG-255, which is the one place
    // that decides whether a position can honestly be shown at all — it is
    // `null` for an unknown clip AND for a beat too old to trust.
    expect(DOCK).toMatch(/\{#if posMs !== null\}/);
  });
});

// ── THE CLOCK IS ABOUT THE SCREEN THE OPERATOR IS WATCHING (RG-238) ─────────
//
// The operator: *"everything should work with whats live and whats on any
// screen ... what operators see is whats on screen"*. The readout said
// `0:25 left · from STAGE MONITOR` while the programme pane showed the main
// screen's clip — because the rule answers with whichever screen has the LEAST
// remaining, and a stage monitor that started a moment earlier wins that.
//
// The shortest remaining is still the right answer for the question *when does
// the first screen run out*. It is the wrong answer for *how long is left of
// what I am watching*, and the second is what a desk readout beside a Pause
// button is asked. So the MAIN screen answers when it has a clip, and the
// soonest answers when it does not — which is still every screen a church has
// not given a role to.
describe('which screen the clock is about', () => {
  const row = (id, name, remaining, over = {}) => ({
    id,
    name,
    painting: true,
    media: { dur_ms: 120_000, pos_ms: 120_000 - remaining, paused: false },
    ...over,
  });

  it('the main screen answers, even when another is further through', () => {
    const d = describeMediaClock(
      [row(1, 'Main screen', 90_000), row(2, 'STAGE MONITOR', 25_000)],
      { mainId: 1 },
    );
    expect(d.from).toBe('Main screen');
    expect(d.remainingMs).toBe(90_000);
  });

  it('and the soonest still answers when the main screen has no clip', () => {
    const d = describeMediaClock(
      [row(1, 'Main screen', 90_000, { media: null }), row(2, 'STAGE MONITOR', 25_000)],
      { mainId: 1 },
    );
    expect(d.from).toBe('STAGE MONITOR');
    expect(d.remainingMs).toBe(25_000);
  });

  it('with no main screen named, nothing changes — this is an addition, not a rewrite', () => {
    const rows = [row(1, 'Main screen', 90_000), row(2, 'STAGE MONITOR', 25_000)];
    expect(describeMediaClock(rows)).toEqual(describeMediaClock(rows, {}));
    expect(describeMediaClock(rows).from).toBe('STAGE MONITOR');
  });

  it('a disagreement is still measured across ALL of them', () => {
    // The spread is about the screens in the room, not about the one being
    // quoted: two screens far apart is one stalled or buffering, and that is
    // exactly what an operator needs before cueing something over it.
    const d = describeMediaClock(
      [row(1, 'Main screen', 90_000), row(2, 'STAGE MONITOR', 25_000)],
      { mainId: 1 },
    );
    expect(d.disagree).toBe(true);
    expect(d.screens).toBe(2);
  });
});
