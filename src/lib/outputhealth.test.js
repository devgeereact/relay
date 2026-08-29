// RG-01 / RG-02 — the screens answer for themselves, and the console repeats
// what they said rather than what Relay hoped.
//
// THE BUG THESE ARE WRITTEN AGAINST, stated so a future reader can reintroduce it
// and watch these go red: Live's Output Status pane used to compute every badge
// from GLOBAL state —
//
//     {#if $live && !$rehearsing && !$screenBlack}  →  amber "On Air"
//
// — which is a restatement of what Relay believes it sent, wearing the costume of
// a report about what happened. Every one of these tests fails if that line comes
// back, because every one of them describes a screen that is NOT answering while
// content is live.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import {
  paintState,
  startBeat,
  screenFault,
  describeScreen,
  FAULT_WORD,
  SCREEN_BADGE,
  BEAT_INTERVAL_MS,
  BEAT_GRACE_MS,
  PAINT_STATES,
} from './outputHealth.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

/** A healthy `ChannelLiveness` row, as Rust serialises it. */
const row = (over = {}) => ({
  id: 1,
  online: true,
  clients: 1,
  detail: 'Serving · screen responding',
  supported: true,
  painting: true,
  last_beat_ms: 300,
  paint_state: 'content',
  ...over,
});
const ON_AIR = { rehearsing: false, live: true, black: false };

describe('what a screen is showing', () => {
  it('reports the three states Rust will accept, and nothing else', () => {
    expect(paintState({ black: false, visible: true, content: {} })).toBe('content');
    expect(paintState({ black: false, visible: false, content: {} })).toBe('clear');
    expect(paintState({ black: false, visible: true, content: null })).toBe('clear');
    expect(paintState({ black: true, visible: true, content: {} })).toBe('black');
    for (const s of ['content', 'clear', 'black'])
      expect(PAINT_STATES).toContain(s);
  });

  it('blackout beats content — the report describes the ROOM, not the DOM', () => {
    // A blacked-out screen still has the verse in its DOM underneath. Reporting
    // `content` would describe markup nobody can see, and an operator checking
    // that the blackout landed would be told it had not.
    expect(paintState({ black: true, visible: true, content: { text: 'John 3:16' } })).toBe(
      'black',
    );
  });
});

describe('screenFault — the half both surfaces share', () => {
  it('never claims anything before the first poll', () => {
    expect(screenFault(null)).toBe('unknown');
    expect(screenFault(undefined)).toBe('unknown');
  });

  it('separates "cannot", "nothing attached", "never answered" and "went silent"', () => {
    expect(screenFault(row({ supported: false, online: false }))).toBe('unsupported');
    expect(screenFault(row({ online: false, painting: false }))).toBe('offline');
    expect(screenFault(row({ painting: false, last_beat_ms: null }))).toBe('never');
    expect(screenFault(row({ painting: false, last_beat_ms: 30000 }))).toBe('silent');
    expect(screenFault(row())).toBe('ok');
  });

  it('treats a missing last_beat_ms as never answered, not as freshly answered', () => {
    // An absence is not a zero. The inverse would render an unknown screen as the
    // healthiest thing on the list.
    expect(screenFault(row({ painting: false, last_beat_ms: undefined }))).toBe('never');
  });

  it('gives every fault a word for the Outputs table', () => {
    for (const f of ['unknown', 'unsupported', 'offline', 'never', 'silent', 'ok'])
      expect(FAULT_WORD[f]).toBeTruthy();
  });
});

describe('describeScreen — the badge may never claim more than the screen did', () => {
  it('says On Air only when the screen is actually answering', () => {
    expect(describeScreen(row(), ON_AIR)).toMatchObject({ kind: 'onair', label: 'On Air' });
  });

  it('THE BUG: a silent screen with live content is NOT On Air', () => {
    const d = describeScreen(row({ painting: false, last_beat_ms: 30000 }), ON_AIR);
    expect(d.kind).toBe('down');
    expect(d.label).toBe('Not responding');
    // And it must not be amber, at any price. Amber is spent only on air.
    expect(SCREEN_BADGE[d.kind]).toBe('rose');
    expect(SCREEN_BADGE[d.kind]).not.toBe('amber');
  });

  it('a screen that has never answered is not accused while it is still starting', () => {
    const st = row({ painting: false, last_beat_ms: null });
    expect(describeScreen(st, ON_AIR, 1000).kind).toBe('idle');
    expect(describeScreen(st, ON_AIR, 1000).label).toBe('Waiting…');
  });

  it('…but silence stops being "not yet" once the grace period is spent', () => {
    const st = row({ painting: false, last_beat_ms: null });
    expect(describeScreen(st, ON_AIR, BEAT_GRACE_MS + 1).kind).toBe('down');
  });

  it('a screen that answered once and stopped is a fault immediately, grace or not', () => {
    // Grace exists for a screen that has not started yet. One that WAS working and
    // went quiet has already proved it can report, so there is nothing to wait for.
    const st = row({ painting: false, last_beat_ms: 9000 });
    expect(describeScreen(st, ON_AIR, 0).kind).toBe('down');
    expect(describeScreen(st, ON_AIR, 0).note).toMatch(/last answered 9s ago/);
  });

  it('rehearsal is amethyst and never amber, even with a healthy screen', () => {
    const d = describeScreen(row(), { rehearsing: true, live: true, black: false });
    expect(d.label).toBe('Rehearsal');
    expect(SCREEN_BADGE[d.kind]).toBe('amethyst');
  });

  it('a blackout reads Blackout, not On Air', () => {
    const d = describeScreen(row({ paint_state: 'black' }), {
      rehearsing: false,
      live: true,
      black: true,
    });
    expect(d.label).toBe('Blackout');
    expect(SCREEN_BADGE[d.kind]).not.toBe('amber');
  });

  it('repeats the screen’s own last word rather than ours', () => {
    expect(describeScreen(row({ paint_state: 'clear' }), ON_AIR).note).toBe('screen: clear');
  });

  it('claims nothing at all before the first poll', () => {
    expect(describeScreen(null, ON_AIR)).toMatchObject({ kind: 'unknown', label: 'Checking…' });
    expect(SCREEN_BADGE.unknown).toBe('grey');
  });

  it('an unavailable target is grey, not a fault the operator can fix', () => {
    // NDI is parked by decision. Painting it rose would send a volunteer hunting
    // for a broken cable that does not exist.
    const d = describeScreen(row({ supported: false, online: false }), ON_AIR);
    expect(d.label).toBe('Unavailable');
    expect(SCREEN_BADGE[d.kind]).toBe('grey');
  });
});

describe('startBeat', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));

  it('reports at once, so a screen just opened is not shown as silent', async () => {
    const sent = [];
    const stop = startBeat({
      channelId: 3,
      getState: () => 'content',
      invoke: async (cmd, args) => sent.push([cmd, args]),
    });
    await flush();
    stop();
    expect(sent).toEqual([['output_beat', { channelId: 3, state: 'content' }]]);
  });

  it('prefers the socket a kiosk page already has', async () => {
    const frames = [];
    const ws = { readyState: 1, send: (f) => frames.push(JSON.parse(f)) };
    const invoked = [];
    const stop = startBeat({
      channelId: 4,
      getState: () => 'black',
      getWs: () => ws,
      invoke: async (c) => invoked.push(c),
    });
    await flush();
    stop();
    expect(frames).toEqual([{ kind: 'beat', channel: 4, state: 'black' }]);
    expect(invoked).toEqual([]);
  });

  it('sends NOTHING down a socket that is not open — silence, never a queued lie', async () => {
    // A queued frame arrives seconds later and reports the screen as healthy at a
    // moment it demonstrably was not. There is no bridge on a kiosk page, so the
    // correct outcome is that the beat simply goes stale.
    const ws = { readyState: 0, send: () => expect.unreachable('must not send') };
    const stop = startBeat({ channelId: 4, getState: () => 'content', getWs: () => ws });
    await flush();
    stop();
  });

  it('says nothing when the page cannot say what it is showing', async () => {
    const sent = [];
    const stop = startBeat({
      channelId: 5,
      getState: () => {
        throw new Error('render broke');
      },
      invoke: async (c) => sent.push(c),
    });
    await flush();
    stop();
    expect(sent).toEqual([]);
  });

  it('drops a state Rust would not accept rather than sending it', async () => {
    const sent = [];
    const stop = startBeat({
      channelId: 5,
      getState: () => 'ON AIR',
      invoke: async (c) => sent.push(c),
    });
    await flush();
    stop();
    expect(sent).toEqual([]);
  });

  it('reports nothing for a raw template preview (channel 0)', async () => {
    const sent = [];
    const stop = startBeat({
      channelId: 0,
      getState: () => 'content',
      invoke: async (c) => sent.push(c),
    });
    await flush();
    stop();
    expect(sent).toEqual([]);
  });

  it('never throws, whatever the transport does', async () => {
    const stop = startBeat({
      channelId: 6,
      getState: () => 'content',
      getWs: () => {
        throw new Error('no socket');
      },
      invoke: async () => {
        throw new Error('no backend');
      },
    });
    await flush();
    expect(stop).not.toThrow();
  });
});

describe('the beat interval is one decision held in two languages', () => {
  it('matches channels::BEAT_INTERVAL_MS, and leaves three beats of grace', () => {
    // The JS ticks and Rust judges. If they drift, either every healthy screen
    // flickers into NOT RESPONDING or a dead one stays green — and both failures
    // are silent, which is why this is pinned across the files rather than trusted.
    const rs = read('src-tauri/src/channels.rs');
    const interval = Number(/BEAT_INTERVAL_MS: u64 = ([\d_]+)/.exec(rs)[1].replace(/_/g, ''));
    expect(BEAT_INTERVAL_MS).toBe(interval);

    // The staleness window must stay DERIVED from the interval rather than written
    // beside it as its own number. Two independently-reasonable constants side by
    // side is how they drift, and the drift is silent in both directions. The
    // three-beats-of-grace arithmetic itself is pinned on the Rust side, where it
    // can be evaluated instead of pattern-matched.
    expect(rs).toMatch(/BEAT_STALE_MS: u64 = BEAT_INTERVAL_MS \* 3/);
  });
});

describe('both surfaces read the same fact', () => {
  it('Live and Outputs decide from the backend`s `painting`, not from global state', () => {
    const live = read('src/lib/views/Live.svelte');
    const outputs = read('src/lib/views/Channels.svelte');

    // Live must go through the shared rule.
    expect(live).toMatch(/describeScreen\(/);
    // …and must no longer derive the output badge from what Relay believes it sent.
    const pane = live.slice(live.indexOf('Output Status'), live.indexOf('ROW B'));
    expect(pane).not.toMatch(/\{#if \$live && !\$rehearsing && !\$screenBlack\}/);

    // Outputs must decide its word from the same helper.
    expect(outputs).toMatch(/screenFault\(/);
    expect(outputs).toMatch(/FAULT_WORD\[/);
  });

  it('the output page reports on BOTH transports, not just the one that was easy', () => {
    // The twin-door rule. A kiosk-only beat would leave the projector — the screen
    // that matters most — with the status light that could not fail.
    const page = read('src/Output.svelte');
    expect(page).toMatch(/startBeat\(/);
    expect(page).toMatch(/getWs: \(\) => ws/);
    // Started on the shared path, so the desktop branch cannot skip it.
    expect(page.indexOf('startBeat(')).toBeGreaterThan(page.indexOf('startKiosk();'));
  });
});
