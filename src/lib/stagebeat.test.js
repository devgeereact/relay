// THE PREACHER'S TABLET ANSWERS FOR ITSELF, LIKE EVERY OTHER SCREEN.
//
// ── The defect (plan S11) ───────────────────────────────────────────────────
//
// `Stage.svelte` sent exactly ONE message for its whole lifetime — the `hello`
// at `onopen`. It never sent a `beat`. `Output.svelte` has reported every two
// seconds since `outputHealth.js` landed (`startBeat`, called at the END of
// both its transports), and `channels::OutputHealth` is keyed per CHANNEL — so
// a channel whose only client is a stage tablet held `last_beat_ms == null`
// for the whole service.
//
// What that costs is not a missing lamp. `describeScreen` answers `never` for
// a screen that is attached and has never reported painting, and
// `describeStageReach` turns that into *"has never reported painting — a Stage
// Timer needs the stage address (Outputs → Sharing)"*. So a CORRECTLY wired
// preacher's tablet, showing the reading perfectly, told the operator it was
// the one thing it was not: unwired. The instrument pointed at the fix for a
// different fault, during a service, on the surface an operator watches.
//
// This is rule 35 once more — a status that reads the same when the thing
// behind it is fine and when it is broken — and it is the console's half of
// the connection question, where the plan's S5 is the phone's half.
//
// ── Why this page and not a second mechanism ────────────────────────────────
//
// `startBeat` already takes `getWs` for exactly this reason: a kiosk client has
// a socket and no bridge. The stage page is a kiosk client. It needs no new
// protocol, no new frame kind and no Rust change — the hub has parsed inbound
// `beat` frames against a closed enum since the mechanism shipped. A stage-only
// health path would be a second answer to a question that already has one.
//
// Every test below was watched to fail against the pre-fix page, where the only
// frame the socket ever carries is the hello.
//
//   npx vitest run src/lib/stagebeat.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { BEAT_INTERVAL_MS } from './outputHealth.js';

const Stage = (await import('../Stage.svelte')).default;

let socket;
class FakeSocket {
  constructor() {
    socket = this;
    this.sent = [];
    // OPEN. `startBeat` sends over a socket only at readyState 1, deliberately:
    // a queued send on a reconnecting socket would report health at a moment the
    // screen demonstrably had none.
    this.readyState = 1;
  }
  send(m) { this.sent.push(m); }
  close() { this.closed = true; this.readyState = 3; }
}

const VERSE = { kind: 'content', reference: 'John 3:16', text: 'For God so loved the world' };

let host;
let app;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
});
afterEach(() => {
  vi.useRealTimers();
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

async function open(channel) {
  window.history.replaceState({}, '', channel == null ? '/stage.html' : `/stage.html?channel=${channel}`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
  await tick();
}

const send = (frame) => socket.onmessage({ data: JSON.stringify(frame) });
const beats = () => socket.sent.map((m) => JSON.parse(m)).filter((f) => f.kind === 'beat');
const lastBeat = () => beats().at(-1);

describe('the stage page reports that it is painting', () => {
  it('sends a beat naming its own channel, not only a hello', async () => {
    await open(2);
    const first = lastBeat();
    expect(first, 'the stage page sent no beat at all').toBeTruthy();
    expect(first.channel).toBe(2);
  });

  it('reports at once rather than leaving the operator two seconds of silence', async () => {
    // A screen that has just opened is the moment an operator is most likely to
    // be looking at it. `startBeat` ticks immediately and then on the interval.
    await open(2);
    expect(beats().length).toBeGreaterThanOrEqual(1);
  });

  it('keeps reporting on the shared interval', async () => {
    vi.useFakeTimers();
    await open(2);
    const before = beats().length;
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 3);
    expect(beats().length).toBeGreaterThan(before);
  });

  it('says content when a reading is up, and clear when it is not', async () => {
    // Fake timers BEFORE the mount: `startBeat`'s interval is created there, and
    // an interval made under real timers cannot be advanced by fake ones.
    vi.useFakeTimers();
    await open(2);
    expect(lastBeat().state).toBe('clear');
    send(VERSE);
    await tick();
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS);
    expect(lastBeat().state).toBe('content');
  });

  it('says black for a blacked screen even though the verse is still in the DOM', async () => {
    // `paintState` puts blackout above content on purpose: a blacked-out screen
    // showing a stale verse underneath is BLACK to the person looking at it, and
    // reporting `content` would describe the DOM rather than the room.
    vi.useFakeTimers();
    await open(2);
    send(VERSE);
    send({ kind: 'screen_state', screens: { 2: 'black' } });
    await tick();
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS);
    expect(lastBeat().state).toBe('black');
  });

  it('reports nothing for a page opened with no channel', async () => {
    // `stage.html` with no `?channel=` is unidentified and belongs to no screen.
    // A beat would attach health to a channel nobody chose.
    await open(null);
    expect(beats()).toEqual([]);
  });

  it('stops when the page is destroyed, so a closed tablet goes stale', async () => {
    vi.useFakeTimers();
    await open(2);
    app.$destroy();
    app = null;
    const after = beats().length;
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 3);
    expect(beats().length).toBe(after);
  });
});

// ── AND THE ANSWER IS WORTH SOMETHING (plan S5, S6) ────────────────────────
//
// The hub answers each `beat` with `{"kind":"beat_ack","at":<host epoch ms>}`,
// to that one client. Two findings wanted the same frame.
//
// **S5 — a socket is not a screen.** `connected` was set on `onopen` and never
// re-evaluated, so a phone that slept, roamed, or sat behind a NAT that had
// timed out kept a green `live` pip over frozen content. Silence cannot detect
// that on its own: the hub publishes only when something CHANGES, so no frames
// is the normal state of a quiet service. An unanswered beat is different — the
// page knows it asked.
//
// **S6 — the countdown was computed against the phone's clock.** `countdown_to`
// is an absolute epoch produced on the host; the stage page subtracted its own
// `Date.now()`. A tablet a minute out showed a minute of error to the person
// preaching, on the figure they are pacing a sermon against.
describe('what the phone does with the answer', () => {
  /** A content frame carrying a countdown aimed by a host `skew` ms ahead of us. */
  const countdownFrom = (skew, minutes) => ({
    kind: 'content',
    reference: 'Notices',
    countdown_to: Date.now() + skew + minutes * 60_000,
    countdown_from: Date.now() + skew,
  });

  const ack = (skew) => send({ kind: 'beat_ack', at: Date.now() + skew });

  it('shows the host countdown, not the one the phone would compute alone', async () => {
    vi.useFakeTimers();
    await open(2);
    // The phone is a minute BEHIND the host. Uncorrected it would read 6:00 on a
    // five-minute countdown, and the preacher would pace a sermon against it.
    const SKEW = 60_000;
    ack(SKEW);
    send(countdownFrom(SKEW, 5));
    await vi.advanceTimersByTimeAsync(1000);
    await tick();
    // A second has passed since it was aimed, so 4:59 is the right answer and
    // 5:59 is the bug. The band, not an exact string: this asserts the CLOCK
    // was corrected, and must not fail on the tick that carried it.
    expect(host.textContent).toMatch(/\b4:5\d\b|\b5:00\b/);
    expect(host.textContent).not.toMatch(/\b5:5\d\b|\b6:00\b/);
  });

  it('is not thrown by one slow round trip', async () => {
    // A single late ack is a latency sample, not a clock change. The offset is
    // a median, so one outlier cannot move what the preacher is reading.
    vi.useFakeTimers();
    await open(2);
    for (let i = 0; i < 4; i += 1) {
      ack(60_000);
      await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS);
    }
    ack(600_000); // one wild sample
    send(countdownFrom(60_000, 5));
    await vi.advanceTimersByTimeAsync(1000);
    await tick();
    expect(host.textContent).toMatch(/\b4:5\d\b|\b5:00\b/);
    // The wild sample would have put it nine minutes out. Scoped to the
    // countdown's own label: an unscoped match also catches the wall clock,
    // which legitimately reads 11:02.
    expect(host.textContent).not.toMatch(/Countdown\s+1\d:/);
  });

  it('stops claiming to be live when its beats stop being answered', async () => {
    vi.useFakeTimers();
    await open(2);
    ack(0);
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS);
    await tick();
    expect(host.textContent).toContain('live');
    // Three intervals with the socket still nominally open and nobody answering.
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 4);
    await tick();
    expect(host.textContent).not.toContain('live');
  });

  it('comes back to live when the answers resume', async () => {
    vi.useFakeTimers();
    await open(2);
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 4);
    await tick();
    expect(host.textContent).not.toContain('live');
    ack(0);
    await tick();
    expect(host.textContent).toContain('live');
  });

  it('does not call a page with no channel stale, because it never beat', async () => {
    // An unidentified `stage.html` sends no beat by design, so it is owed no
    // ack. Reporting it as stale would be a finding invented from an absence.
    vi.useFakeTimers();
    await open(null);
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 6);
    await tick();
    expect(host.textContent).not.toContain('not answering');
  });
});
