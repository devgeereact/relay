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
import { describe, it, expect, vi } from 'vitest';
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
  screenSwitch,
  screenReporting,
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
    expect(describeScreen(row({ paint_state: 'clear' }), ON_AIR).note).toMatch(/screen says clear/);
    expect(describeScreen(row(), ON_AIR).note).toBe('screen: content');
  });

  // ── RG-129's half of the badge ──────────────────────────────────────────────
  //
  // The tests above cover a screen that has stopped ANSWERING. These cover the
  // other half of the same bug: a screen that answers punctually, on time, and
  // says it is showing nothing — while Relay is sending it a verse. Every one of
  // these reads `On Air`, in amber, if the ok-branch goes back to deciding from
  // `wall.live`, which is what it did until this wave.
  describe('THE OTHER HALF: the screen’s own word, not Relay’s belief, decides', () => {
    it('a screen that says CLEAR while a verse is live is not On Air, and not amber', () => {
      // The measured RG-129 failure: an output page that reconnected mid-service
      // came back blank and stayed blank. It kept beating the whole time — it was
      // answering, it was just answering "clear" — so every instrument Relay had
      // said On Air about a black screen with a congregation in front of it.
      const d = describeScreen(row({ paint_state: 'clear' }), ON_AIR);
      expect(d.label).toBe('Not confirmed');
      expect(d.kind).not.toBe('onair');
      expect(SCREEN_BADGE[d.kind]).not.toBe('amber');
      // Both claims, side by side. When they disagree, that disagreement IS the
      // finding, so neither half may be dropped.
      expect(d.note).toMatch(/Relay is sending content/);
      expect(d.note).toMatch(/screen says clear/);
    });

    it('a BLACKOUT that did not land does not read as a blackout', () => {
      // Rule 15 / DECISIONS §20 on the reporting side: a panic control may never
      // report a success it did not achieve. The screen is still painting the
      // verse and says so.
      const d = describeScreen(row({ paint_state: 'content' }), {
        rehearsing: false,
        live: true,
        black: true,
      });
      expect(d.label).toBe('Not confirmed');
      expect(d.note).toMatch(/blacked this screen out/);
      expect(d.note).toMatch(/screen says content/);
    });

    it('a screen still showing a verse after a clear does not read Ready', () => {
      // `clear_screens` returns Ok, nothing is on the programme, and the screen is
      // still painting the last verse. "Ready" would be the reassuring word over
      // the failure — rule 35 exactly.
      const d = describeScreen(row({ paint_state: 'content' }), {
        rehearsing: false,
        live: false,
        black: false,
      });
      expect(d.label).toBe('Not confirmed');
      expect(d.note).toMatch(/nothing is on the programme/);
    });

    it('…and when the two agree it says so plainly, in three words and no more', () => {
      expect(describeScreen(row({ paint_state: 'content' }), ON_AIR).label).toBe('On Air');
      const off = { rehearsing: false, live: false, black: false };
      expect(describeScreen(row({ paint_state: 'clear' }), off).label).toBe('Ready');
      expect(describeScreen(row({ paint_state: 'black' }), off).label).toBe('Ready');
      const blk = { rehearsing: false, live: true, black: true };
      expect(describeScreen(row({ paint_state: 'black' }), blk).label).toBe('Blackout');
      // A cleared screen satisfies a blackout too: both mean nothing of ours is
      // on it, which is the whole of the claim that badge makes.
      expect(describeScreen(row({ paint_state: 'clear' }), blk).label).toBe('Blackout');
    });

    it('a row that has not said what it is showing can never earn amber', () => {
      // Rust reads the state and the age off ONE beat, so a `painting` row always
      // carries a state. A row without one is a fixture — and a fixture must not
      // be able to earn a colour a screen would not.
      for (const bad of [undefined, null, 'ON AIR', '']) {
        const d = describeScreen(row({ paint_state: bad }), ON_AIR);
        expect(d.kind, `paint_state ${String(bad)}`).not.toBe('onair');
        expect(d.note).toMatch(/has not said/);
      }
    });

    it('a rehearsal still wins over everything, whatever the screen says', () => {
      // Amethyst means rehearsal and nothing else. A disagreement inside a
      // rehearsal is not a congregation-facing fault, and painting it as one
      // would spend the operator's attention on a room nobody is watching.
      for (const s of ['content', 'clear', 'black']) {
        const d = describeScreen(row({ paint_state: s }), { rehearsing: true, live: true });
        expect(d.label).toBe('Rehearsal');
      }
    });

    it('the unconfirmed word is GREY, so it is a fact and not a false alarm', () => {
      // It appears for a few seconds after every fire, while the screen's next
      // beat is on its way. Rose there would be an alarm that cries on every
      // correct fire, and an alarm like that gets ignored on the one that matters.
      const d = describeScreen(row({ paint_state: 'clear' }), ON_AIR);
      expect(SCREEN_BADGE[d.kind]).toBe('grey');
    });

    it('and an unconfirmed screen is not counted as live by the status bar', async () => {
      // `screenTally` counts `onair` rows. It must not be possible to reach that
      // count without the screen having said it is painting content.
      const { screenTally } = await import('./statusbar.js');
      const described = [
        describeScreen(row({ id: 1, paint_state: 'content' }), ON_AIR),
        describeScreen(row({ id: 2, paint_state: 'clear' }), ON_AIR),
      ];
      expect(screenTally(described)).toEqual({ live: 1, total: 2 });
    });
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
    // The first beat of a page's life has no previous tick, so it says nothing
    // about a gap rather than claiming a zero one (RG-119).
    //
    // The three media fields are NAMED here rather than the equality being
    // loosened, for the reason `timerwords.test.js` gives about its own payload:
    // this test's claim is that the page invents nothing, and `null` is what "this
    // screen is not playing a clip" looks like. Letting an unexamined field
    // through would retire the claim to save the test.
    expect(sent).toEqual([
      [
        'output_beat',
        {
          channelId: 3,
          state: 'content',
          sinceMs: null,
          hiddenMs: null,
          mediaPosMs: null,
          mediaDurMs: null,
          mediaPaused: null,
          // …and the media failure it did not have (O-4): named, for the same reason.
          mediaError: null,
        },
      ],
    ]);
  });

  it('the first beat omits the gap entirely on the socket, and later ones carry it', async () => {
    vi.useFakeTimers();
    const frames = [];
    const ws = { readyState: 1, send: (f) => frames.push(JSON.parse(f)) };
    const stop = startBeat({ channelId: 4, getState: () => 'content', getWs: () => ws });

    // An absent number reads as "the screen did not say", which is true of a
    // first beat. A zero would read as "it said it never went quiet".
    expect(frames).toEqual([{ kind: 'beat', channel: 4, state: 'content' }]);

    vi.advanceTimersByTime(BEAT_INTERVAL_MS);
    stop();
    expect(frames).toHaveLength(2);
    expect(frames[1].kind).toBe('beat');
    expect(Number.isInteger(frames[1].since_ms)).toBe(true);
    expect(frames[1].since_ms).toBeGreaterThanOrEqual(0);
    // Nothing hid this page, and saying so is the point: it is what separates a
    // window the OS covered from one whose beats were lost on the way.
    expect(frames[1].hidden_ms).toBe(0);
    vi.useRealTimers();
  });

  it('a page that keeps ticking into a dead socket still reports a ONE-INTERVAL gap', async () => {
    // The distinction RG-119 exists to make. `lastTickAt` advances on every tick
    // that runs, not on every send that lands, so a transport failure cannot be
    // mistaken for the OS having stopped the page.
    vi.useFakeTimers();
    const frames = [];
    const ws = { readyState: 1, send: () => { throw new Error('socket gone'); } };
    const sent = [];
    const stop = startBeat({
      channelId: 5,
      getState: () => 'content',
      getWs: () => ws,
      invoke: async (cmd, args) => sent.push(args),
    });
    vi.advanceTimersByTime(BEAT_INTERVAL_MS * 3);
    stop();
    vi.useRealTimers();
    await new Promise((r) => setTimeout(r, 0));
    frames.length = 0;
    const gaps = sent.slice(1).map((a) => a.sinceMs);
    expect(gaps.length).toBeGreaterThan(0);
    for (const g of gaps) expect(g).toBeLessThan(BEAT_INTERVAL_MS * 2);
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

describe('screenReporting — the heartbeat claim, and the two it must not collapse', () => {
  it('a screen answering inside the window reports yes, with the evidence', () => {
    const r = screenReporting(row());
    expect(r.word).toBe('yes');
    expect(r.note).toMatch(/screen: content/);
  });

  it('a screen that answered once and has gone quiet is NOT yes', () => {
    // The whole point of the row. `online` stays true for a projector showing a
    // dead renderer, so "is it attached" and "is it painting" are different
    // questions and only the second can go false on its own.
    const r = screenReporting(row({ painting: false, last_beat_ms: 30000 }));
    expect(r.word).toBe('stopped');
    expect(r.word).not.toBe('yes');
    expect(r.note).toMatch(/last answered 30s ago/);
  });

  it('NEVER is not NO — they are different faults and want different repairs', () => {
    // `never`: something IS attached and has not once said it is painting — a
    // browser source on the wrong URL, a page that threw on load.
    // `no`: nothing is attached to ask — the window was never opened.
    // One reassuring word over both is rule 35 exactly.
    expect(screenReporting(row({ painting: false, last_beat_ms: null })).word).toBe('never');
    expect(screenReporting(row({ online: false, painting: false, last_beat_ms: null })).word).toBe(
      'no',
    );
  });

  it('claims nothing before the first poll, and nothing about a target Relay cannot drive', () => {
    expect(screenReporting(null).word).toBe('—');
    expect(screenReporting(row({ supported: false, online: false })).word).toBe('—');
  });

  it('every fault has a word, so the row can never render undefined', () => {
    for (const st of [
      null,
      row(),
      row({ supported: false }),
      row({ online: false }),
      row({ painting: false, last_beat_ms: null }),
      row({ painting: false, last_beat_ms: 9000 }),
    ]) {
      expect(screenReporting(st).word).toBeTruthy();
      expect(typeof screenReporting(st).note).toBe('string');
    }
  });
});

/**
 * The same file with its prose removed.
 *
 * THIS HELPER IS A FINDING. The assertions below used to read the raw source, and
 * when the Outputs tab was moved off `FAULT_WORD` and onto `describeScreen` they
 * all still passed — because the replacement carries a COMMENT naming the defect
 * it replaced, and `expect(outputs).toMatch(/FAULT_WORD\[/)` cannot tell a use
 * from a description of one. A scanner that matches its own explanation is the
 * "ipc.test.js was wrong twice" failure in miniature: it looks exhaustive while
 * checking nothing. Precedent and prior art: `screenpreview.test.js`.
 */
const code = (src) =>
  src
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '');

describe('both surfaces read the same fact', () => {
  it('Live and Outputs decide from the backend`s `painting`, not from global state', () => {
    const live = code(read('src/lib/views/Live.svelte'));
    const outputs = code(read('src/lib/views/Channels.svelte'));

    // Live must go through the shared rule.
    expect(live).toMatch(/describeScreen\(/);
    // …and must no longer derive the output badge from what Relay believes it sent.
    const pane = live.slice(live.indexOf('$: outs = channels.map'), live.indexOf('ROW B'));
    expect(pane).not.toMatch(/\{#if \$live && !\$rehearsing && !\$screenBlack\}/);

    // Outputs must decide its word from the same helper — the SAME one, not
    // merely a helper from the same file. `FAULT_WORD[screenFault(st)]` is a
    // second ladder: it is blind to a rehearsal and to a blackout, so the two
    // surfaces could call one screen LIVE and Rehearsal in the same second.
    expect(outputs).toMatch(/describeScreen\(/);
    expect(outputs, 'the Outputs cards must not run a second health ladder').not.toMatch(
      /FAULT_WORD\[/,
    );
  });

  it('and the three inputs to that rule are NAMED in both, so neither pane can freeze', () => {
    // Svelte tracks the identifiers in a reactive expression. A helper closing
    // over `$channelHealth` is not tracked, and the pane would show its first
    // reading for the rest of the service — which is the same class of defect as
    // the badge that could not fail.
    for (const f of ['src/lib/views/Live.svelte', 'src/lib/views/Channels.svelte']) {
      const src = code(read(f));
      const call = src.slice(src.indexOf('describeScreen('), src.indexOf('describeScreen(') + 320);
      for (const name of ['$rehearsing', '$live', '$screenBlack', '$channelWaiting']) {
        expect(call, `${f}: ${name} is not named at the describeScreen call`).toContain(name);
      }
    }
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

// ─────────────────────────────────────────────────────────────────────────────
// RG-29 · switching one screen on or off from the run surface
//
// The Output Status pane's whole purpose is to report a screen that is down, and
// it offered no way to bring one back — an operator who read "Not responding" had
// to leave the run surface mid-service and hunt for the row in another tab.
//
// The rule is pure and shared with the Outputs tab, so the badge and the button
// can never end up describing different screens.
// ─────────────────────────────────────────────────────────────────────────────

const NATIVE = { id: 1, name: 'Main screen', render_target: 'native_window' };
const KIOSK = { id: 2, name: 'OBS', render_target: 'network_client' };

describe('RG-29 · turning a screen on and off', () => {
  it('offers to turn ON a native screen with no window open', () => {
    const w = screenSwitch({ supported: true, online: false }, NATIVE);
    expect(w.action).toBe('on');
    expect(w.label).toBe('Turn on');
    expect(w.why).toMatch(/No window/);
  });

  it('offers to turn OFF a screen that has a window, answering or not', () => {
    for (const st of [
      { supported: true, online: true, painting: true, last_beat_ms: 100 },
      // Not responding is still a window that exists — and turning it off and on
      // again is the repair the pane exists to make possible.
      { supported: true, online: true, painting: false, last_beat_ms: 999999 },
    ]) {
      expect(screenSwitch(st, NATIVE).action).toBe('off');
    }
  });

  it('offers NOTHING for a browser source, and says where to go instead', () => {
    const w = screenSwitch({ supported: true, online: true }, KIOSK);
    expect(w.action).toBeNull();
    // A handlerless button has shipped in this repository before. A control that
    // cannot work must say why rather than look available.
    expect(w.why).toMatch(/OBS|kiosk|phone/i);
  });

  it('offers nothing before the first health reading', () => {
    // `unknown` is "not asked yet", not "off". Offering "Turn on" for a screen
    // that may already be on would be a guess printed as a control.
    const w = screenSwitch(null, NATIVE);
    expect(w.action).toBeNull();
    expect(w.label).toBe('Checking…');
  });

  it('agrees with the badge about the same screen', () => {
    // The two must be derived from one fact. A screen with no window reads "No
    // window" on the badge and "Turn on" on the button — never one of each.
    const st = { supported: true, online: false };
    expect(describeScreen(st, {}, 0).label).toBe('No window');
    expect(screenSwitch(st, NATIVE).action).toBe('on');
  });
});

// ── THE BANNER HAS TO SAY WHICH SCREEN ───────────────────────────────────────
//
// The shell's degraded line read **"3 is not responding"**. `degraded.js` has
// documented its `screensDown` argument as *"names of screens"* since it was
// written; the producer (`App.svelte`) mapped `st.id`, because the backend row
// carried no name to map. A number is not something a volunteer can act on with a
// congregation waiting, and nothing else on any screen relates "3" back to
// "Streaming".
//
// Both halves are pinned, because the fix needed both: the field has to exist in
// Rust and the shell has to use it. Either one alone puts the number back.
describe('a screen that stops answering is named, not numbered', () => {
  it('the backend row carries the screen name', async () => {
    const { readFileSync } = await import('node:fs');
    const rust = readFileSync('src-tauri/src/main.rs', 'utf8');
    const struct = rust.slice(rust.indexOf('struct ChannelLiveness'));
    expect(struct.slice(0, struct.indexOf('}'))).toMatch(/\bname: String,/);
  });

  it('the shell maps health rows to that name', async () => {
    const { readFileSync } = await import('node:fs');
    const shell = readFileSync('src/App.svelte', 'utf8');
    const line = shell.slice(shell.indexOf('$: screensDown'));
    const decl = line.slice(0, line.indexOf(';'));
    expect(decl).toMatch(/st\.name/);
    expect(decl).not.toMatch(/=>\s*st\.id\b/);
  });
});

// 2026-09-21 · O-4 / M-3. A screen whose picture did not load must not be called
// On Air. The page says so on the beat it already sends; the desk reads it.
describe('a screen whose media failed to load', () => {
  it('carries the failure on the beat, both doors', async () => {
    vi.useFakeTimers();
    const frames = [];
    const ws = { readyState: 1, send: (f) => frames.push(JSON.parse(f)) };
    const sent = [];
    // The kiosk door: a socket takes the beat.
    const stop = startBeat({
      channelId: 6,
      getState: () => 'content',
      getWs: () => ws,
      getMediaError: () => 'picture not loading · http://10.0.0.5:8032/media/3',
    });
    stop();
    expect(frames[0].media_error).toBe('picture not loading · http://10.0.0.5:8032/media/3');
    // The native door: no socket, so the bridge takes it.
    const stop2 = startBeat({
      channelId: 6,
      getState: () => 'content',
      invoke: async (cmd, args) => sent.push([cmd, args]),
      getMediaError: () => 'picture not loading · http://10.0.0.5:8032/media/3',
    });
    await Promise.resolve();
    await Promise.resolve();
    stop2();
    vi.useRealTimers();
    expect(sent[0][1].mediaError).toBe('picture not loading · http://10.0.0.5:8032/media/3');
  });

  it('is not On Air on the desk, and the note names the failure', () => {
    const st = row({ media_error: 'picture not loading · http://10.0.0.5:8032/media/3' });
    const d = describeScreen(st, ON_AIR);
    expect(d.kind).toBe('down');
    expect(d.label).toBe('Not painting the picture');
    expect(d.note).toContain('media/3');
  });

  it('and a screen with no failure is still On Air', () => {
    expect(describeScreen(row({ media_error: null }), ON_AIR).kind).toBe('onair');
  });
});
