// THE DOCK'S FIRST TWO CARDS — Live audio and Live transcript (L3).
//
// The operator's words were "LIVE AUDIO too should read accurately not just
// running in bits" and "LIVE TRANSCRIPT should be as on the artifacts …
// accurate and fast". Both turned out to be about the DISPLAY rather than about
// the engine, and this file holds what was actually wrong.
//
//   1. THE TRACE WAS DRAWN ON AN EVENT AXIS, NOT A TIME AXIS. One column per
//      `audio://chunk`, pushed from a `$:` block — so a column meant "one
//      delivery", a stalled path drew a frozen picture that read exactly like a
//      quiet room, and two identical consecutive readings advanced it by nothing
//      at all (Svelte does not invalidate a primitive that has not changed).
//   2. NOTHING SAID THE MICROPHONE HAD STOPPED DELIVERING. `quiet` and `−∞ dB`
//      were the same two words a silent room gets — rule 35, on the one card an
//      operator watches to decide whether Relay has gone deaf.
//   3. THE TRANSCRIPT WAS FOUR UNLABELLED LINES. No time, no mark on what is
//      still being said, no scroll — so anything an operator looked away from
//      was gone, and there was no way to tell a closed line from a revising one.
//   4. THE MICROPHONE WAS THREE WORKSPACES AWAY, in Settings → Audio.
//
// WHAT IS DELIBERATELY NOT ASSERTED: pixels. jsdom has no 2d canvas context, so
// `draw()` returns early here — which is exactly why the trace's arithmetic was
// extracted into three pure functions at module scope. They are what is tested;
// the drawing is the lead's headless render.
//
//   npx vitest run src/lib/dockaudio.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const Dock = await import('./Dock.svelte');
const { pushReading, pushEnvelope, readingKind, waveRuns, waveSegments, waveStale, WAVE_SPAN_MS, WAVE_GAP_MS, CLIP_AT } = Dock;
const src = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

let host;
let app;

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock.default({ target: host, props: {} });
  return host;
}

async function settle(ms = 10) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(null);
  cap.live.set(null);
  cap.stageAlert.set(null);
  cap.meter.set({ level: 0, isVoice: false });
  cap.transcript.set({ partial: '', finals: [], finalsAt: [] });
  cap.capture.update((s) => ({
    ...s,
    available: true,
    capturing: false,
    devices: [],
    inputDevice: '',
  }));
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

// ─────────────────────────────────────────────────────────────────────────────
// THE TRACE'S ARITHMETIC
// ─────────────────────────────────────────────────────────────────────────────

describe('the waveform reads time, not deliveries', () => {
  it('keeps one reading older than the span, so the left edge is real signal', () => {
    // Without it the trace begins at the first reading INSIDE the window and the
    // left-hand edge is a made-up zero rather than the signal running off the
    // side. The one kept point is what joins the two.
    const t0 = 1_000_000;
    let buf = [];
    for (const dt of [0, 5_000, 10_000, 15_000, 20_000, 25_000]) {
      buf = pushReading(buf, t0 + dt, 0.5);
    }
    // At t0+25_000 with a 20s span, t0 and t0+5_000 are both off the left edge —
    // the older of the two goes, the newer one stays as the edge.
    expect(buf.map((r) => r.t - t0)).toEqual([5_000, 10_000, 15_000, 20_000, 25_000]);
    expect(WAVE_SPAN_MS).toBe(20_000);
  });

  it('clamps a reading rather than drawing outside the box', () => {
    const buf = pushReading(pushReading([], 1, 4.2), 2, -3);
    expect(buf.map((r) => r.v)).toEqual([1, 0]);
  });

  it('starts again when the clock goes backwards', () => {
    // A resumed laptop or a corrected system time would otherwise place new
    // readings to the LEFT of old ones and draw the envelope inside out.
    const buf = pushReading(pushReading([], 5_000, 0.4), 1_000, 0.9);
    expect(buf).toEqual([{ t: 1_000, v: 0.9, kind: 'quiet' }]);
  });

  it('positions each reading at the time it was taken', () => {
    // `x` is 1 at `now` and 0 one span ago — so the axis is seconds, and the
    // same trace means the same thing whatever rate the bridge delivered at.
    // A generous gap here, because the spacing is what the NEXT test is about.
    const now = 100_000;
    const [seg] = waveSegments(
      [
        { t: now - 20_000, v: 0.2 },
        { t: now - 10_000, v: 0.4 },
        { t: now, v: 0.6 },
      ],
      now,
      WAVE_SPAN_MS,
      30_000,
    );
    expect(seg.map((p) => p.x)).toEqual([0, 0.5, 1]);
  });

  it('does NOT join two readings across audio nobody measured', () => {
    // THE RULE THIS FILE EXISTS FOR. `latency.rs` says a stage never reached is
    // an ABSENCE, not a zero; the same is true here. A line drawn across a gap
    // claims a quiet room, and a quiet room is the one thing it must not be
    // mistaken for — that is how Relay went silently deaf to a quiet preacher.
    const now = 100_000;
    const segs = waveSegments(
      [
        { t: now - 9_000, v: 0.5 },
        { t: now - 8_400, v: 0.5 },
        // …nothing for six seconds. The microphone, the device or the whole
        // capture path stopped; none of that audio exists.
        { t: now - 2_400, v: 0.5 },
        { t: now - 1_800, v: 0.5 },
      ],
      now,
    );
    expect(segs).toHaveLength(2);
    expect(segs[0]).toHaveLength(2);
    expect(segs[1]).toHaveLength(2);
    // And the break is where the silence was, not at an arbitrary point.
    expect(segs[0][1].x).toBeCloseTo(1 - 8_400 / WAVE_SPAN_MS, 6);
    expect(segs[1][0].x).toBeCloseTo(1 - 2_400 / WAVE_SPAN_MS, 6);
  });

  it('joins readings that arrive at the rate the bridge actually delivers them', () => {
    // `main.rs` emits every THIRD chunk and `audio::Chunker`'s hop is 200 ms, so
    // a healthy path delivers about every 600 ms. Ordinary jitter must never be
    // drawn as a fault, which is why the gap is three deliveries and not one.
    const now = 100_000;
    const buf = [0, 600, 1200, 1800, 2400].map((dt) => ({ t: now - 2_400 + dt, v: 0.3 }));
    expect(waveSegments(buf, now)).toHaveLength(1);
    expect(WAVE_GAP_MS).toBeGreaterThan(3 * 600);
  });

  it('a stale signal is a TIMEOUT ON ARRIVAL and says nothing about loudness', () => {
    // CLAUDE.md rule 12 / DECISIONS §19: nothing may compare a signal to an
    // absolute level. This takes no level at all — only two instants.
    expect(waveStale(1_000, 1_000 + WAVE_GAP_MS - 1)).toBe(false);
    expect(waveStale(1_000, 1_000 + WAVE_GAP_MS + 1)).toBe(true);
    expect(waveStale(NaN, 9e9)).toBe(false);
    // It takes two instants and a duration. Nothing about a signal reaches it,
    // which is the property, and it is asserted against the FUNCTION rather
    // than against the prose beside it.
    expect(String(waveStale)).not.toMatch(/level|rms|isVoice|db|loud/i);
  });

  it('draws no gate line, and compares nothing to a fixed level', () => {
    // The prototype draws the sensitivity gate as a dashed line across the trace
    // and Relay must not: a line at a fixed height would draw the voice gate as
    // a threshold it is not. `$meter.isVoice` is the gate's own answer and it is
    // the chip in the head, in words.
    const draw = src.slice(src.indexOf('function draw()'), src.indexOf('// ── THE SENSITIVITY'));
    expect(draw).not.toMatch(/sensitivity|setLineDash|threshold/i);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// WHAT THE CARD SAYS WHEN THE SIGNAL STOPS
// ─────────────────────────────────────────────────────────────────────────────

describe('a live microphone delivering nothing says so', () => {
  it('says `no signal` rather than the two words a silent room gets', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mount();
      cap.capture.update((s) => ({ ...s, capturing: true }));
      await tick();
      cap.meter.set({ level: 0.4, isVoice: true });
      await tick();
      // Still delivering: the chip and the figure are the reading.
      expect(host.querySelector('.dhead').textContent).toContain('VOICE');
      expect(host.querySelector('.dmeta.nosig')).toBeNull();

      // Nothing arrives for longer than three deliveries.
      await vi.advanceTimersByTimeAsync(WAVE_GAP_MS + 600);
      await tick();
      const head = host.querySelector('.dhead').textContent;
      expect(head).toContain('no signal');
      // And it does NOT go on reporting a level nobody measured.
      expect(head).not.toContain('VOICE');
      expect(head).not.toContain('dB');
    } finally {
      vi.useRealTimers();
    }
  });

  it('an UNCHANGED reading is still a reading, and still keeps the card alive', async () => {
    // DEFECT 1, and it is the one the operator could see. The trace was pushed
    // from `$: pushSample(lvl)`, and a Svelte reactive block re-runs only when
    // something it reads is INVALIDATED — `safe_not_equal` does not invalidate a
    // primitive that has not changed. So a run of identical readings (a muted
    // channel, a room that is genuinely silent, a device pinned at one value)
    // advanced the trace by nothing at all, and now reports a fault that is not
    // one.
    //
    // The store is read directly instead: `meter.set` builds a fresh object
    // every time, so every delivery notifies whatever the number happens to be.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mount();
      cap.capture.update((s) => ({ ...s, capturing: true }));
      await tick();
      // Eight identical deliveries at the rate the bridge actually sends them,
      // spanning well past the stale window.
      for (let i = 0; i < 8; i++) {
        cap.meter.set({ level: 0.25, isVoice: false });
        await vi.advanceTimersByTimeAsync(600);
        await tick();
      }
      expect(host.querySelector('.dhead').textContent).not.toContain('no signal');
    } finally {
      vi.useRealTimers();
    }
  });

  it('never says it while the microphone is closed — that is `not listening`', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mount();
      await tick();
      await vi.advanceTimersByTimeAsync(WAVE_GAP_MS * 3);
      await tick();
      expect(host.querySelector('.dmeta.nosig')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('with no engine at all it says that instead, and claims no measurement', async () => {
    // UNCHANGED AND MUST STAY. `quiet` and `−∞ dB` over a detached bridge read
    // exactly like a live microphone in a silent room.
    cap.capture.update((s) => ({ ...s, available: false }));
    mount();
    await settle();
    const head = host.querySelector('.dhead').textContent;
    expect(head).toContain('no engine');
    expect(head).not.toContain('quiet');
    expect(head).not.toMatch(/dB/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE MICROPHONE, ON THE RUN SURFACE
// ─────────────────────────────────────────────────────────────────────────────

describe('the microphone is chosen and opened from the card that shows its level', () => {
  it('offers the devices the engine reported, plus the system default', async () => {
    cap.capture.update((s) => ({
      ...s,
      devices: [{ name: 'MacBook Pro Microphone', is_default: true }, { name: 'Scarlett 2i2 USB' }],
    }));
    mount();
    await settle();
    const pick = host.querySelector('[aria-label="Microphone input device"]');
    expect(pick).not.toBeNull();
    expect([...pick.querySelectorAll('option')].map((o) => o.textContent)).toEqual([
      'System default',
      'MacBook Pro Microphone',
      'Scarlett 2i2 USB',
    ]);
  });

  it('writes the choice to the ONE store Settings reads, so the two cannot disagree', async () => {
    cap.capture.update((s) => ({ ...s, devices: [{ name: 'Scarlett 2i2 USB' }] }));
    mount();
    await settle();
    const pick = host.querySelector('[aria-label="Microphone input device"]');
    pick.value = 'Scarlett 2i2 USB';
    pick.dispatchEvent(new Event('change'));
    await settle();
    let seen;
    cap.capture.subscribe((s) => (seen = s.inputDevice))();
    expect(seen).toBe('Scarlett 2i2 USB');
  });

  it('opens capture with that device, and stops it again', async () => {
    cap.capture.update((s) => ({ ...s, devices: [{ name: 'Scarlett 2i2 USB' }], inputDevice: 'Scarlett 2i2 USB' }));
    mount();
    await settle();
    const sw = host.querySelector('[aria-label="Microphone"]');
    expect(sw.getAttribute('aria-checked')).toBe('false');
    sw.click();
    await settle();
    expect(called('start_capture')).toHaveLength(1);
    expect(called('start_capture')[0][1]).toEqual({ device: 'Scarlett 2i2 USB' });

    cap.capture.update((s) => ({ ...s, capturing: true }));
    await settle();
    expect(host.querySelector('[aria-label="Microphone"]').getAttribute('aria-checked')).toBe('true');
    host.querySelector('[aria-label="Microphone"]').click();
    await settle();
    expect(called('stop_capture')).toHaveLength(1);
  });

  it('will not let the device be changed under a running capture', async () => {
    // `start_capture` takes the device name as an argument, so a change made
    // mid-capture would move a label and nothing else. Same rule as Settings.
    cap.capture.update((s) => ({ ...s, devices: [{ name: 'Scarlett 2i2 USB' }], capturing: true }));
    mount();
    await settle();
    expect(host.querySelector('[aria-label="Microphone input device"]').disabled).toBe(true);
  });

  it('reports a failed stop rather than claiming the microphone is off', async () => {
    // `stopCapture` THROWS by contract and deliberately leaves `capturing` set
    // when the stop did not happen — so the switch keeps saying `live` over a
    // live microphone, which is the truth (rule 15, from the other end).
    cap.capture.update((s) => ({ ...s, capturing: true }));
    mount();
    await settle();
    invoke.mockRejectedValue({ kind: 'internal', message: 'the audio lock is poisoned' });
    host.querySelector('[aria-label="Microphone"]').click();
    await settle();
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
    expect(host.querySelector('[aria-label="Microphone"]').getAttribute('aria-checked')).toBe('true');
  });

  it('spends no amber on a microphone', async () => {
    // Amber is ON AIR and is never allowed to mean anything else (rule 18). A
    // live microphone is not a congregation looking at something.
    //
    // READ WITH THE COMMENTS STRIPPED. The reason this row is not amber is
    // written beside it, and a scanner that reads the prose about a rule
    // instead of the rule is a scanner that passes everything — `ipc.test.js`
    // has been wrong that way twice.
    const markup = codeOnly(src);
    const row = markup.slice(markup.indexOf('<span class="dcap">Mic</span>'), markup.indexOf('<span class="dcap">Sens</span>'));
    expect(row).not.toMatch(/amber/);
    // And on the rendered control, which is what an operator actually sees.
    cap.capture.update((s) => ({ ...s, capturing: true }));
    mount();
    await settle();
    const sw = host.querySelector('[aria-label="Microphone"]');
    // Svelte stamps a scoping class on every element it styles, so the list is
    // filtered to the ones this file put there rather than compared whole — the
    // hash changes whenever the stylesheet does, and a test that breaks on a
    // comment edit is one somebody deletes.
    expect([...sw.classList].filter((c) => !c.startsWith('svelte-')))
      .toEqual(['r-iconbtn', 'audtog', 'on']);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// C2 · THE ICON TOGGLE (operator instruction 2026-09-14)
//
//   "I will prefer to have an icon toggle button rather than having this big
//    switch on the audio section."
//
// The 38x21 `.r-switch` is gone from this card and the 26px shared icon button
// is in its place. The point of these assertions is that the SEMANTICS did not
// go with it: a visual preference is not a licence to downgrade a control that
// a screen reader currently announces as "microphone, switch, on".
//
// Each one was watched to fail with the pill put back.
// ─────────────────────────────────────────────────────────────────────────────
describe('C2 · the audio card wears icon toggles, with the switch semantics intact', () => {
  const TOGGLES = ['Microphone', 'Detection'];

  it('neither toggle is the sliding pill any more', async () => {
    mount();
    await settle();
    for (const label of TOGGLES) {
      const b = host.querySelector(`[aria-label="${label}"]`);
      expect(b, `no control labelled ${label}`).not.toBeNull();
      expect([...b.classList], `${label} is still a pill`).not.toContain('r-switch');
      // The SHARED square, never a shape this file drew for itself. `.audtog`
      // beside it may paint and position; `app.css` owns the box.
      expect([...b.classList], `${label} is not the shared icon button`).toContain('r-iconbtn');
    }
    // …and the card defines no switch of its own to replace it with.
    expect(src.slice(src.indexOf('<style>'))).not.toMatch(/\.audtog[^{]*\{[^}]*height:/);
  });

  it('it still announces as a switch, with its state and a name', async () => {
    mount();
    await settle();
    for (const label of TOGGLES) {
      const b = host.querySelector(`[aria-label="${label}"]`);
      expect(b.getAttribute('role'), `${label} is not announced as a switch`).toBe('switch');
      expect(b.getAttribute('aria-checked'), `${label} does not say its state`).toBeTruthy();
      // The state in words, so a hover answers the question too.
      expect(b.getAttribute('title'), `${label} has no title`).toBeTruthy();
    }
  });

  it('what it DRAWS is the state, not only what it is tinted', async () => {
    // A square that only changes colour is a square somebody has to learn. The
    // off state carries a strike through the glyph, so the control reads with no
    // colour at all — which is the whole difference between an icon toggle and a
    // coloured box.
    cap.capture.update((s) => ({ ...s, capturing: false, detectionOn: false }));
    mount();
    await settle();
    const strokes = (label) =>
      host.querySelector(`[aria-label="${label}"] svg`).querySelectorAll('path, circle, rect').length;
    const off = TOGGLES.map(strokes);
    cap.capture.update((s) => ({ ...s, capturing: true, detectionOn: true }));
    await settle();
    const on = TOGGLES.map(strokes);
    for (let i = 0; i < TOGGLES.length; i++) {
      expect(on[i], `${TOGGLES[i]} draws the same thing on as off`).toBeLessThan(off[i]);
    }
  });

  it('and spends no amber on either of them', () => {
    // Rule 18, on the row the switch used to be on. Comments stripped: the
    // reason this is not amber is written beside it, and a scanner that reads
    // the prose instead of the rule passes everything.
    const markup = codeOnly(src);
    const rows = markup.slice(markup.indexOf('<div class="audrow">'), markup.indexOf('</div>\n    </div>'));
    expect(rows).not.toMatch(/amber/);
    const style = src.slice(src.indexOf('<style>')).replace(/\/\*[\s\S]*?\*\//g, '');
    const rule = style.slice(style.indexOf('\n  .audtog {'), style.indexOf('\n  .dcap {'));
    expect(rule).not.toMatch(/--v-amber|--v-amethyst|--v-cyan/);
    expect(rule, 'the on state paints nothing at all').toMatch(/--v-emerald/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE TRANSCRIPT
// ─────────────────────────────────────────────────────────────────────────────

describe('the transcript is timestamped, and says which line is still being said', () => {
  it('stamps every closed line with the time it arrived', async () => {
    cap.transcript.set({
      partial: '',
      finals: ['Turn with me to Romans eight', 'And we know that all things'],
      finalsAt: ['10:31:04', '10:31:09'],
    });
    mount();
    await settle();
    const rows = [...host.querySelectorAll('.tbody .trl')];
    expect(rows.map((r) => r.querySelector('.tt').textContent)).toEqual(['10:31:04', '10:31:09']);
    expect(rows.map((r) => r.querySelector('.tx').textContent)).toEqual([
      'Turn with me to Romans eight',
      'And we know that all things',
    ]);
  });

  it('pairs a line with ITS OWN timestamp, however the rolling cap has shifted', async () => {
    // THE BUG THIS SHAPE EXISTS TO PREVENT. `finals` and `finalsAt` are stamped
    // in lockstep at the source; the one previous consumer aligned them by
    // LENGTH and drifted the moment the cap froze `finals.length` — every line
    // then carried the timestamp of a different line. Pairing by index across
    // the full arrays and slicing the PAIRS cannot reproduce it.
    const n = 60; // longer than the pane's own window, so the slice is doing work
    cap.transcript.set({
      partial: '',
      finals: Array.from({ length: n }, (_, i) => `line ${i}`),
      finalsAt: Array.from({ length: n }, (_, i) => `at ${i}`),
    });
    mount();
    await settle();
    for (const r of host.querySelectorAll('.tbody .trl')) {
      const i = r.querySelector('.tx').textContent.replace('line ', '');
      expect(r.querySelector('.tt').textContent).toBe(`at ${i}`);
    }
  });

  it('marks what is still being said `now`, with a caret, and never stamps it', async () => {
    // It has not closed yet, and stamping it would date a line whisper is still
    // revising several times a second.
    cap.transcript.set({ partial: 'and we know that all things work', finals: ['Turn with me'], finalsAt: ['10:31:04'] });
    mount();
    await settle();
    const cur = host.querySelector('.tbody .trl.cur');
    expect(cur).not.toBeNull();
    expect(cur.querySelector('.tt').textContent).toBe('now');
    expect(cur.querySelector('.tx').textContent).toContain('and we know that all things work');
    expect(cur.querySelector('.caret')).not.toBeNull();
    // It is the LAST row: newest at the bottom, and the one still open is newer
    // than every closed one.
    expect([...host.querySelectorAll('.tbody .trl')].pop()).toBe(cur);
  });

  it('shows more than a keyhole, and scrolls itself to the live line', async () => {
    mount();
    await settle();
    const body = host.querySelector('.tbody');
    // jsdom lays nothing out, so `scrollHeight` is 0 and the assertion that can
    // be made honestly is that the pane IS the scroller and IS being driven.
    expect(body.classList.contains('r-scroll')).toBe(true);
    expect(src).toMatch(/afterUpdate\(\(\) => \{\s*if \(trStuck && trBody\) trBody\.scrollTop = trBody\.scrollHeight;/);
    // NEVER `tick()` from a reactive block on the surface that updates several
    // times a second — that is rule 1, and it freezes the webview with no error.
    const script = src.slice(src.lastIndexOf('<script>'));
    expect(script.slice(0, script.indexOf('</script>'))).not.toMatch(/\$:[\s\S]{0,120}\btick\(/);

    cap.transcript.set({ partial: '', finals: Array.from({ length: 30 }, (_, i) => `l${i}`), finalsAt: [] });
    await settle();
    expect(host.querySelectorAll('.tbody .trl').length).toBeGreaterThan(4);
  });

  it('says which of two silences it is looking at', async () => {
    // "Relay has gone deaf" and "the preacher has not spoken" need opposite
    // responses, so the empty pane may not read the same for both.
    mount();
    await settle();
    expect(host.querySelector('.tbody').textContent).toContain('not listening');
    cap.capture.update((s) => ({ ...s, capturing: true }));
    await settle();
    expect(host.querySelector('.tbody').textContent).toContain('listening…');
  });

  it('a line still being said is the SELECTION colour, never a colour-law one', () => {
    // Cyan means the AI has guessed at a verse, amber means a congregation is
    // looking at something, amethyst means rehearsal. A half-heard sentence is
    // none of the three — it is the thing being worked on.
    const style = src.slice(src.indexOf('  .trl.cur'), src.indexOf('  .trl.cur') + 220);
    expect(style).toMatch(/--v-sel/);
    expect(style).not.toMatch(/--v-amber|--v-cyan|--v-amethyst/);
  });
});

// ── THE WAVEFORM IS A WAVEFORM NOW, AND IT HAS THREE COLOURS ───────────────
//
// The operator, 2026-09-20: *"Make the Live Audio wave be professional and
// colour coded, as this is not giving an original live audio wave."* They were
// right and the fault was not in the drawing. `start_capture` emitted every
// THIRD chunk carrying ONE rms number, so the trace was a reading every ~600 ms
// joined with straight lines: about one point per spoken word. It was a level
// history labelled as a waveform.
//
// The backend now sends every SECOND chunk (so the readings cover the timeline
// once, with no overlap and no gap) carrying `audio::CHUNK_PEAKS` peaks across
// its 400 ms — one reading per 25 ms, which is inside a syllable.
describe('one chunk is a shape, not a number', () => {
  it('spreads a chunk’s peaks across the time the chunk covers', () => {
    // All sixteen stamped at the arrival time would pile on one pixel and draw a
    // vertical spike per delivery: a different wrong picture, not a right one.
    const peaks = [0.1, 0.2, 0.3, 0.4];
    const buf = pushEnvelope([], 10_000, peaks, false, 0.25, 400);
    expect(buf.map((r) => r.v)).toEqual(peaks);
    expect(buf.map((r) => r.t)).toEqual([9_700, 9_800, 9_900, 10_000]);
  });

  it('puts the newest reading at the present, not 25 ms ago', () => {
    const buf = pushEnvelope([], 10_000, [0.1, 0.9], false, 0.5, 400);
    expect(buf[buf.length - 1].t).toBe(10_000);
  });

  it('falls back to one reading when the engine sent no envelope', () => {
    // An older backend, or a build without the peaks. The console must draw what
    // it always drew rather than a flat line.
    for (const none of [undefined, null, []]) {
      const buf = pushEnvelope([], 10_000, none, true, 0.42, 400);
      expect(buf).toEqual([{ t: 10_000, v: 0.42, kind: 'voice' }]);
    }
  });
});

describe('the colour is measured, never guessed', () => {
  it('asks the VOICE GATE whether this is speech, and never a level', () => {
    // Rule 12 (DECISIONS §19): nothing may compare a signal to an absolute level
    // to decide what speech is. The same quiet reading is emerald or steel purely
    // according to what the gate said about the chunk it came from.
    expect(readingKind(0.04, true)).toBe('voice');
    expect(readingKind(0.04, false)).toBe('quiet');
    expect(readingKind(0.8, false)).toBe('quiet');
  });

  it('calls full scale clipping, whatever the gate thought', () => {
    // The ONE absolute fact in audio: the sample had nowhere left to go. Saying
    // so is not a threshold Relay invented.
    expect(readingKind(1, true)).toBe('clip');
    expect(readingKind(1, false)).toBe('clip');
    expect(readingKind(CLIP_AT, false)).toBe('clip');
    expect(readingKind(CLIP_AT - 0.01, true)).toBe('voice');
  });

  it('has no fourth colour, and amber is not among the three', () => {
    // Amber is ON AIR (rule 18) and is never spent on a microphone. A "hot but
    // not clipping" band would be exactly the absolute threshold rule 12 removed.
    const kinds = new Set();
    for (const v of [0, 0.3, 0.6, 0.9, 1]) for (const g of [true, false]) kinds.add(readingKind(v, g));
    expect([...kinds].sort()).toEqual(['clip', 'quiet', 'voice']);
  });
});

describe('a segment is drawn in runs of one colour', () => {
  const pt = (x, v, kind) => ({ x, v, kind });

  it('splits where the colour changes, not per reading', () => {
    const runs = waveRuns([
      pt(0, 0.1, 'quiet'), pt(0.1, 0.1, 'quiet'),
      pt(0.2, 0.5, 'voice'), pt(0.3, 0.6, 'voice'),
      pt(0.4, 1, 'clip'),
    ]);
    expect(runs.map((r) => r.kind)).toEqual(['quiet', 'voice', 'clip']);
  });

  it('repeats the boundary point so the shapes meet with no gap', () => {
    // Four points, so BOTH runs have width and survive the one-point filter.
    const runs = waveRuns([
      pt(0, 0.1, 'quiet'), pt(0.3, 0.2, 'quiet'),
      pt(0.6, 0.9, 'voice'), pt(0.9, 0.8, 'voice'),
    ]);
    expect(runs).toHaveLength(2);
    // Two filled shapes that merely abut leave a hairline of background between
    // them at every voicing change, which on a live trace is most of them.
    expect(runs[runs.length - 1].pts[0]).toEqual(runs[0].pts[runs[0].pts.length - 1]);
  });

  it('draws nothing for a run of one point, which has no width', () => {
    expect(waveRuns([pt(0.5, 0.4, 'voice')])).toEqual([]);
    expect(waveRuns([])).toEqual([]);
  });

  it('a clip lasting 25 ms is red for 25 ms, not for the whole chunk', () => {
    // The reason the split is inside a segment rather than per delivery.
    const buf = pushEnvelope([], 10_000, [0.3, 0.3, 1, 0.3], true, 0.5, 400);
    const runs = waveRuns(waveSegments(buf, 10_000)[0]);
    expect(runs.map((r) => r.kind)).toEqual(['voice', 'clip', 'voice']);
  });
});

// ── THE TRANSCRIPT KEEPS THE WHOLE SERVICE, AND PAINTS A WINDOW OF IT ──────
//
// *"I want all transcript to be kept, not just what you hear before another
// minute ... keep all, starting from different seconds and minute and hour."*
// (operator, 2026-09-20). `capture.js` now keeps every closed line with no cap.
//
// This is the other half. A keyed `{#each}` lays out every row it is handed, and
// handing it a whole service costs a layout pass over ~800 rows several times a
// minute for a box that shows about eight. So the card paints a WINDOW of the
// newest lines and grows it when the operator scrolls near the top, which is the
// only moment more of them can be seen.
describe('the transcript card paints a window, and the window grows', () => {
  const DOCK = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');
  const CODE = codeOnly(DOCK);

  it('renders the NEWEST lines, never the oldest', () => {
    // A window taken off the front would show the start of the service for ever
    // while the preacher talked.
    expect(CODE).toContain('allLines.slice(-trShown)');
  });

  it('extends the window when the operator reaches the top of it', () => {
    // Without this the cap is simply a shorter cap, and scrolling back stops at
    // a wall with more session behind it and no way to reach it.
    expect(CODE).toMatch(/trShown \+= TR_PAGE/);
    expect(CODE).toMatch(/trShown < allLines\.length/);
  });

  it('never shrinks the window mid-service', () => {
    // The only assignment that lowers it is the new-service reset below. A
    // window that shrank while the operator was reading would take the line
    // they were looking at off the screen.
    const lowers = [...CODE.matchAll(/trShown\s*(=|-=)\s*([^;\n]+)/g)].map((m) => m[0]);
    expect(lowers.every((l) => /TR_PAGE/.test(l))).toBe(true);
  });

  it('starts again for a new service rather than creeping up for ever', () => {
    expect(CODE).toMatch(/allLines\.length < trShown - TR_PAGE/);
  });

  it('does not reintroduce a cap on the STORE', () => {
    // The store is the session's memory; this file may cap what it PAINTS and
    // must never cap what is kept.
    const CAP = readFileSync(resolve(process.cwd(), 'src/lib/stores/capture.js'), 'utf8')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/^[ \t]*\/\/.*$/gm, '');
    expect(CAP).not.toMatch(/finals: \[[^\]]*\]\.slice\(/);
    expect(CAP).not.toMatch(/finalsAt: \[[^\]]*\]\.slice\(/);
  });
});
