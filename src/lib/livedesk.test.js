// W1 · LIVE — THE RUN SURFACE, MOUNTED.
//
// `slidegridwiring.test.js` reads the source and asks whether the wiring is
// right. This file mounts the component and asks whether the SURFACE is: does a
// cell actually paint a slide, does a press still mean what it meant once there
// is a rendered thumbnail under the finger, and does a claim card say which kind
// of claim it is carrying.
//
// The order matters. Reading the source proves the code says the right thing;
// mounting proves the operator gets it. Fourteen passing tests were once written
// against a component nothing rendered (CLAUDE.md, Testing), which is the same
// gap from the other end.
//
//   npx vitest run src/lib/livedesk.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const Live = (await import('./views/Live.svelte')).default;

const settle = (ms = 80) => new Promise((r) => setTimeout(r, ms));

// THE REAL BUILT-IN, not a hand-made stand-in. A thumbnail's whole claim is
// that it renders what the wall renders, so the template under the test has to
// be one a wall actually uses.
const { BUILTINS } = await import('./templates.js');
const TPL = BUILTINS[0];
// `role: 'main'` is what a fresh install seeds. It used to be absent here and the
// pane still named this channel, because the old expression picked it off
// `render_target` — which is the guess DECISIONS §89 replaced. A fixture that
// leaves it out is now an install where nobody has set a main screen, and the
// head says so.
const CHANNEL = {
  id: 1,
  name: 'Main screen',
  render_target: 'native_window',
  template_id: 1,
  role: 'main',
};

const VERSES = [
  { verse: 1, reference: 'Psalms 23:1', text: 'The LORD is my shepherd; I shall not want.' },
  { verse: 2, reference: 'Psalms 23:2', text: 'He maketh me to lie down in green pastures.' },
  { verse: 3, reference: 'Psalms 23:3', text: 'He restoreth my soul.' },
];

const claim = (over = {}) => ({
  reference: 'Romans 8:28',
  method: 'direct',
  confidence: 0.91,
  text: 'And we know that all things work together for good',
  matched_text: 'romans eight twenty eight',
  in_library: true,
  at: Date.now(),
  ...over,
});

let host;
beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
    if (cmd === 'list_templates') return Promise.resolve([TPL]);
    if (cmd === 'chapter_verses') return Promise.resolve(VERSES);
    if (cmd === 'list_plans') return Promise.resolve([]);
    if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
    if (cmd === 'rehearsal') return Promise.resolve(false);
    if (cmd === 'get_sensitivity') return Promise.resolve(50);
    return Promise.resolve(null);
  });
  // A run surface with a model loaded — otherwise `ModelSetup` mounts into the
  // detection panel and asks the backend for a model list this harness has none of.
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  host = document.createElement('div');
  document.body.appendChild(host);
  // Warm the bridge before mounting. Under vitest the very FIRST
  // `await import('@tauri-apps/api/core')` against the mock resolves to
  // undefined, so whichever guarded read happens to go first loses its answer —
  // a harness artefact (the real app imports a real module), but one that would
  // leave `$templates` empty and make every assertion below about an unstyled
  // fallback rather than about a template a wall uses.
  return cap.loadTemplates();
});
afterEach(() => {
  host.remove();
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
});

// ─────────────────────────────────────────────────────────────────────────────
// 1 · A CELL IS THE WALL IN MINIATURE
// ─────────────────────────────────────────────────────────────────────────────
describe('the slide grid paints slides', () => {
  it('a cell RENDERS the verse — not its label in a grey box', async () => {
    cap.live.set({ reference: 'Psalms 23:1', text: 'The LORD is my shepherd', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();

    const cells = host.querySelectorAll('.sg-cell');
    expect(cells.length).toBe(VERSES.length);

    // TemplateRender's root is `<div class="stage">`, absolutely positioned
    // inside the thumb. If the cell were still drawing text, there would be none.
    const painted = host.querySelectorAll('.sg-thumb .stage');
    expect(painted.length).toBe(VERSES.length);

    // And it is the WORDS, from the renderer, not the cell's own label.
    expect(host.querySelector('.sg-thumb').textContent).toContain('The LORD is my shepherd');
  });

  // GAP 8, asked directly: a rendered thumbnail must not swallow the press. The
  // 190ms arbitration is proven pure in `slidegrid.test.js`; this asks whether
  // the DOM the operator actually clicks still reaches it.
  it('a single press still SENDS, through a rendered thumbnail', async () => {
    cap.live.set({ reference: 'Psalms 23:1', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    invoke.mockClear();

    host.querySelectorAll('.sg-cell')[1].click();
    // REAL timers, because the beat is the thing under test. 500ms against a
    // 190ms beat: a 70ms margin flaked once on a loaded machine, and a run
    // surface's timing test that fails at random is worse than no test —
    // somebody eventually reruns it until it is green instead of reading it.
    await settle(500);

    const fired = invoke.mock.calls.filter(([c]) => c === 'manual_fire');
    expect(fired).toHaveLength(1);
    expect(fired[0][1].reference).toBe('Psalms 23:2');
  });

  it('A DOUBLE PRESS STILL PREVIEWS AND NOTHING GOES TO AIR', async () => {
    cap.live.set({ reference: 'Psalms 23:1', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    invoke.mockClear();

    const cell = host.querySelectorAll('.sg-cell')[2];
    cell.click();
    cell.dispatchEvent(new window.MouseEvent('dblclick', { bubbles: true }));
    await settle(500);

    expect(invoke.mock.calls.filter(([c]) => c === 'manual_fire')).toHaveLength(0);
    // It went to the preview instead, which is the whole point of the beat.
    await tick();
    expect(host.querySelector('.sg-cell:nth-child(3)').className).toContain('cued');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · THE CLAIM COLUMN
// ─────────────────────────────────────────────────────────────────────────────
describe('the AI detection column', () => {
  it('shows SEVERAL claims at once — one at a time was never the truth', async () => {
    cap.detections.set([claim(), claim({ reference: 'Isaiah 40:31', method: 'semantic' })]);
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelectorAll('.clm')).toHaveLength(2);
    // Both are decidable where they sit. The second used to be a one-line row.
    expect(host.querySelectorAll('.clm .act.go')).toHaveLength(2);
    expect(host.querySelectorAll('.clm .act.no')).toHaveLength(2);
  });

  // RULE 18. A cosine is not a probability, and a number that lies is worse than
  // no number. This is the one assertion on this surface that is a safety rule.
  it('A PARAPHRASE CARRIES NO PERCENTAGE, and a heard claim carries a bar', async () => {
    cap.detections.set([
      claim({ reference: 'Romans 8:28', method: 'direct', confidence: 0.91 }),
      claim({ reference: 'Isaiah 40:31', method: 'semantic', confidence: 0.88 }),
    ]);
    new Live({ target: host, props: {} });
    await settle();

    const [d, g] = host.querySelectorAll('.clm');
    expect(d.className).not.toContain('guess');
    expect(d.querySelector('.conf')).not.toBeNull();

    expect(g.className).toContain('guess');
    expect(g.querySelector('.conf')).toBeNull();
    expect(g.textContent).not.toMatch(/\d+\s*%/);
    // And it is CYAN, never amethyst (which means rehearsal) and never amber.
    expect(g.querySelector('.cbadge').className).toContain('p');
  });

  // The hole this closed: an auto-fire never enters `detections`, so the only
  // thing an operator saw was a verse appearing on the programme with nothing
  // saying the AI put it there.
  it('AN AUTO-FIRE LEAVES A CARD THAT NAMES THE AI', async () => {
    cap.resolvedDetections.set([{ ...claim(), outcome: 'auto', resolvedAt: Date.now() }]);
    new Live({ target: host, props: {} });
    await settle();

    const card = host.querySelector('.clm');
    expect(card.className).toContain('done');
    expect(card.textContent).toContain('Auto-fired');
    // A receipt is not an offer: nothing on it can be pressed.
    expect(card.querySelector('.act')).toBeNull();
  });

  it('a decided claim says WHO decided it, in each of the three wordings', async () => {
    const now = Date.now();
    cap.resolvedDetections.set([
      { ...claim({ reference: 'A 1:1' }), outcome: 'accepted', resolvedAt: now },
      { ...claim({ reference: 'B 2:2' }), outcome: 'dismissed', resolvedAt: now },
    ]);
    new Live({ target: host, props: {} });
    await settle();
    const text = host.querySelector('.det').textContent;
    expect(text).toContain('Accepted — put on the screens');
    expect(text).toContain('Dismissed by the operator');
  });

  // A pending claim is a decision the operator still owes; a receipt is a record
  // of one already made. In a bounded column the record must never push the
  // decision out.
  it('a pending claim is never pushed out of the column by a receipt', async () => {
    const now = Date.now();
    cap.resolvedDetections.set(
      ['R1 1:1', 'R2 2:2', 'R3 3:3', 'R4 4:4'].map((r) => ({
        ...claim({ reference: r }),
        outcome: 'auto',
        resolvedAt: now,
      })),
    );
    cap.detections.set([claim({ reference: 'Pending 9:9' })]);
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.clm .clm-ref').textContent.trim()).toBe('Pending 9:9');
  });

  // And the cap falls on the RECEIPTS. The store bounds suggestions at six and
  // prunes them at 45 seconds — the two limits that belong to a suggestion — so
  // a column cap that hid one of those would be hiding a decision the operator
  // still owes in order to keep showing a record of one already made.
  it('every pending claim is drawn; the receipts are what get trimmed', async () => {
    const now = Date.now();
    cap.resolvedDetections.set(
      ['R1 1:1', 'R2 2:2', 'R3 3:3', 'R4 4:4'].map((r) => ({
        ...claim({ reference: r }),
        outcome: 'auto',
        resolvedAt: now,
      })),
    );
    cap.detections.set(
      ['P1 1:1', 'P2 2:2', 'P3 3:3', 'P4 4:4', 'P5 5:5'].map((r) => claim({ reference: r })),
    );
    new Live({ target: host, props: {} });
    await settle();
    const refs = [...host.querySelectorAll('.clm .clm-ref')].map((e) => e.textContent.trim());
    expect(refs).toEqual(['P1 1:1', 'P2 2:2', 'P3 3:3', 'P4 4:4', 'P5 5:5']);
    expect(host.querySelectorAll('.clm.done')).toHaveLength(0);
  });

  // A reference that parsed against no verse keeps its card — it is the
  // operator's evidence that a number was misheard — and the control that cannot
  // take it says so instead of failing after the press.
  it('a claim with no verse behind it is SHOWN, and its Accept is disabled', async () => {
    cap.detections.set([claim({ reference: 'Psalms 999:1', in_library: false })]);
    new Live({ target: host, props: {} });
    await settle();
    const card = host.querySelector('.clm');
    expect(card.querySelector('.act.go').disabled).toBe(true);
    expect(card.textContent).toContain('Nothing to send');
    // Dismiss is NOT disabled — the operator can always clear it.
    expect(card.querySelector('.act.no').disabled).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · THE COLUMN IS THE AI'S CLAIMS, AND FOUR THINGS LEFT IT
//
// `docs/REBRAND.md` §2 gives the run surface a 286px right column and puts ONE
// thing in it: what the AI thinks it heard. Relay's carried four more — a second
// scripture box with its own Fire, an Output Status pane, the emergency
// announcement and Open main output — so the claims an operator is meant to be
// reading competed with three panels about something else.
//
// All four MOVED. That is the only defensible version of this change, and it is
// what these tests hold: the column is one pane, and every control it used to
// carry is rendered somewhere an operator can still reach.
//
// Watched to fail: put any of the four back and the first test counts two panes.
// ─────────────────────────────────────────────────────────────────────────────
describe('the inspector column', () => {
  it('sits beside the stage and holds the claims — and nothing else', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const insp = host.querySelector('.insp-col');
    expect(insp).not.toBeNull();
    expect(insp.parentElement.className).toContain('desk');
    expect(insp.previousElementSibling.className).toContain('stage');

    const panes = [...insp.children].filter((e) => e.className.includes('pane'));
    expect(panes).toHaveLength(1);
    expect(panes[0].textContent).toContain('AI Detection');

    // The four that left, each by the thing that would still render it here.
    cap.channelHealth.set({ 1: { id: 1, name: 'Main screen', supported: true, online: false } });
    await settle();
    expect(insp.querySelector('.out'), 'a per-screen status row').toBeNull();
    expect(insp.querySelector('input[aria-label="Manual scripture reference"]')).toBeNull();
    expect(insp.querySelector('input[aria-label="Emergency announcement"]')).toBeNull();
    expect(insp.textContent).not.toContain('Open main output');
  });

  // NOTHING BECAME UNREACHABLE. This is the level `scripts/qa-inventory.mjs`
  // polices — a rendered control, not a wrapper — so it is asserted against the
  // files that render them rather than against the store.
  it('every control the column carried is rendered somewhere else', () => {
    const rail = readFileSync(resolve(__dirname, 'LiveRail.svelte'), 'utf8');
    const dock = readFileSync(resolve(__dirname, 'Dock.svelte'), 'utf8');
    const channels = readFileSync(resolve(__dirname, 'views/Channels.svelte'), 'utf8');

    // The reference fire, ranges included, in §9's one box.
    expect(rail).toMatch(/on:click=\{\(\) => onReference\(q\.trim\(\)\)\}/);
    // THE EMERGENCY ANNOUNCEMENT IS THE ONE THAT DID NOT LAND ANYWHERE, and
    // that is now deliberate rather than an oversight. It left this column for
    // Quick tools, and on 2026-09-14 the operator had it removed from there too
    // (L3). So the honest assertion is the opposite of the one that used to sit
    // here: no rendered control anywhere reaches `push_announcement`.
    //
    // This is a real consequence and it is asserted rather than left implicit —
    // `capture.js::pushAnnouncement` and the Rust command were both DELETED
    // rather than left registered with nothing rendering them, following the
    // same precedent as the five commands deleted on 2026-08-30 (a command
    // nothing calls is attack surface nobody is watching). This test is what
    // says so out loud now that the pair is gone.
    for (const f of ['LiveRail.svelte', 'Dock.svelte', 'views/Live.svelte']) {
      const s = readFileSync(resolve(__dirname, f), 'utf8');
      expect(s, `${f} still renders an announcement control`)
        .not.toMatch(/aria-label="Emergency announcement"/);
    }
    expect(dock).not.toMatch(/annArmed/);
    // The per-screen repair, and the way to open the wall.
    expect(channels).toMatch(/openChannelOutput/);
    expect(channels).toMatch(/closeChannelOutput/);
  });

  // The two facts that are about the ROOM rather than about one screen. A lamp
  // answers for one screen and cannot compose either sentence, so both stayed.
  it('the whole-room warnings stayed on the run surface', () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    expect(live).toMatch(/\{#if nowhereToShow\}/);
    expect(live).toMatch(/\{#if fitWarning\}/);
    expect(live).toMatch(/aria-live="polite">\{downAnnounce\}/);
  });

  // Rule 35: a status line that reads the same when the thing behind it is
  // broken as when it is fine is not a status line.
  it('the gate says what it is DOING, and says something else when it cannot', async () => {
    cap.capture.update((s) => ({ ...s, detectionOn: true, capturing: true }));
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.det-meta').textContent.trim()).toBe('auto-fire on');
    host.innerHTML = '';

    cap.capture.update((s) => ({ ...s, detectionOn: true, capturing: false }));
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.det-meta').textContent.trim()).toBe('not listening');
    host.innerHTML = '';

    cap.capture.update((s) => ({ ...s, detectionOn: false, capturing: true }));
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.det-meta').textContent.trim()).toBe('detection off');
    cap.capture.update((s) => ({ ...s, detectionOn: false, capturing: false }));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4 · ONE DIAL, IN ONE PLACE
//
// Live carried a second copy of the sensitivity dial. The dock's copy is on
// EVERY workspace (docs/REBRAND.md §2 puts it beside the signal it is about),
// which is the whole argument: an operator who has stopped trusting the AI is
// as likely to be in Templates as on Live. Two controls for one gate is two
// places to disagree about it — the same shape as the two status badges rule 35
// was written for. Agreed with the shell agent, who verified the dock's copy is
// on the combined branch before either of us deleted anything.
// ─────────────────────────────────────────────────────────────────────────────
describe('the sensitivity dial', () => {
  it('Live does not draw one — the dock does, on every workspace', async () => {
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelectorAll('input[type="range"]')).toHaveLength(0);
    expect(host.querySelector('[aria-label="Detection sensitivity"]')).toBeNull();
  });

  // L4 · AND NEITHER IS THE ARM SWITCH, NOW. This used to assert the opposite:
  // the dial moved to the dock and an `Armed` chip stayed on the claim panel. The
  // dock then grew the ARMED switch beside the dial (`shellchrome.test.js`:
  // "detection is ONE switch, in the card about the signal"), which left two
  // controls for one gate, one row apart. Two controls for one setting is two
  // places for them to disagree, which is rule 35's family; the chip is gone and
  // the dock's switch is the control.
  //
  // WHAT THIS PANEL KEEPS IS THE STATE, not a quieter copy of the control. The
  // head's `.det-meta` distinguishes four situations the chip could not, and it
  // is what the run surface reads to know whether the AI is armed at all.
  it('carries NO second arm control — the dock owns the switch', async () => {
    cap.capture.update((s) => ({ ...s, detectionOn: true }));
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.det-ctl')).toBeNull();
    expect(host.querySelector('.btnchip')).toBeNull();
    // Nothing on this surface may reach the command the dock's switch owns.
    const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    expect(src).not.toMatch(/setDetection\(/);
    cap.capture.update((s) => ({ ...s, detectionOn: false }));
  });

  it('but the ARM STATE is still on the run surface, and says which failure it is', async () => {
    // A state line that reads the same when the thing behind it is broken as when
    // it is fine is not a status line (rule 35). Armed-and-listening is the only
    // state that says `auto-fire on`; a disarmed detector says so in its own words
    // and never wears the armed colour.
    cap.capture.update((s) => ({
      ...s,
      detectionOn: true,
      capturing: true,
      stt: { ...s.stt, loaded: true },
    }));
    new Live({ target: host, props: {} });
    await settle();
    const meta = host.querySelector('.det-meta');
    expect(meta).not.toBeNull();
    expect(meta.textContent.trim()).toBe('auto-fire on');
    expect(meta.classList.contains('on')).toBe(true);

    cap.capture.update((s) => ({ ...s, detectionOn: false }));
    await settle();
    expect(host.querySelector('.det-meta').textContent.trim()).toBe('detection off');
    expect(host.querySelector('.det-meta').classList.contains('on')).toBe(false);
    cap.capture.update((s) => ({ ...s, capturing: false }));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4b · NOTHING ON THIS COLUMN IS CUT IN HALF
//
// Both found by rendering at 1440×960, neither visible to any source-reading
// test. A screen's name broken mid-word reads as a different screen, and a
// panel that scrolls with no scrollbar reads as a panel that is broken.
// ─────────────────────────────────────────────────────────────────────────────
describe('the 286px column does not clip', () => {
  // THE SCREEN-NAME CLIPPING PAIR MOVED WITH THE PANE THEY WERE ABOUT. They held
  // `.out-nm` and `.out-kind` / `.out-note` in Live's Output Status rows — the
  // pane §2 removed from this column. A screen's name is still ellipsised in two
  // places and both are tested where they live: the chrome lamps
  // (`shellchrome.test.js`) and the Outputs cards. Re-asserting them here would
  // be a third opinion about a screen on the one surface rule 35 says must not
  // hold one, which is the whole reason the pane left.
  //
  // What this file still owes the column is that the thing left IN it fits.
  it('a claim card is readable at 286px — the reference is never broken mid-word', async () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const rule = live.slice(live.indexOf('  .clm-ref{'), live.indexOf('  .clm-ref{') + 400);
    expect(rule).toMatch(/overflow:hidden|text-overflow:ellipsis|min-width:0/);

    cap.detections.set([claim({ reference: '1 Thessalonians 5:16', in_library: true })]);
    new Live({ target: host, props: {} });
    await settle();
    const card = host.querySelector('.insp-col .clm');
    expect(card).not.toBeNull();
    expect(card.querySelector('.clm-ref').textContent).toBe('1 Thessalonians 5:16');
  });

  // THE INTEGRATOR'S RULING, applied to this half of the tree: a glyph in a value
  // slot cannot tell "nothing there" from "we have not asked yet" — the same
  // defect as "up to date" over a dead update channel, and what R3-13 already
  // holds for an unbound template row. An EMPTY slot makes no claim at all, which
  // is the honest thing for a slot with nothing to report.
  it('no em dash stands in for a value anywhere on this surface', async () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    expect(live).not.toMatch(/'—'/);
    expect(live).not.toMatch(/>—</);

    // The programme's reference slot is simply absent when nothing is live — and
    // the state is still said, twice, in words.
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.mon.prog .mon-name')).toBeNull();
    expect(host.querySelector('.mon.prog .tag').textContent).toContain('Clear');
    expect(host.querySelector('.mon.prog .screen').textContent).toContain('Screens clear');
  });

  // THE SCROLLER-SHADOW RECIPE went with the two panes it was written for — the
  // Output Status list and the plan rail, both of which left this surface. What
  // it was guarding is still true of the pane that remains: a body that scrolls
  // must actually be a scroller rather than an overflow, or the content past the
  // fold is unreachable on a booth laptop.
  it('a pane body is a real scroller, not an overflow', () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const body = live.slice(live.indexOf('  .pane-body{'), live.indexOf('  .pane-body::'));
    expect(body).toMatch(/min-height:0/);
    expect(body).toMatch(/overflow-y:auto/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5 · THE MONITORS SAY WHICH IS WHICH
// ─────────────────────────────────────────────────────────────────────────────
describe('preview and programme', () => {
  it('the programme names the screen it is rendering AS', async () => {
    cap.live.set({ reference: 'Romans 8:28', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.mon.prog .mon-as').textContent).toContain('Main screen');
    expect(host.querySelector('.mon.prog .mon-name').textContent).toContain('Romans 8:28');
  });

  // AMBER IS ON AIR AND IS NEVER ALLOWED TO LIE. Nothing on air, no amber —
  // which is the state a console spends most of a Sunday in.
  it('a clear wall wears no amber', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const prog = host.querySelector('.mon.prog');
    expect(prog.className).not.toContain('onair');
    // The reference slot is not there at all — see the em-dash test above. There
    // is no element to carry amber, which is stronger than one that carries none.
    expect(host.querySelector('.mon.prog .mon-name')).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6 · L2 — THE RUN SURFACE, MATCHED TO THE PROTOTYPE
//
// The console was rendered beside the prototype's Live workspace and compared
// pane by pane. Each of these is a difference that was SEEN in that render, not
// one reasoned about from the source — so each is written against the surface an
// operator reads rather than against the rule that produces it.
// ─────────────────────────────────────────────────────────────────────────────
describe('L2 · the studio head reads as one statement', () => {
  const liveSrc = () => readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  // The prototype's head is `PROGRAM · ON AIR · AS MAIN SCREEN` in one mono run.
  // Ours was a chip in the body face with a sentence-case tail after it, so the
  // two halves of one fact read as two different kinds of remark.
  it('the state chip and the screen it renders as share one face', () => {
    const src = liveSrc();
    const tag = src.slice(src.indexOf('  .tag{'), src.indexOf('  .tag{') + 260);
    expect(tag).toMatch(/font-family:var\(--f-mono\)/);
    expect(tag).toMatch(/text-transform:uppercase/);
    const as = src.slice(src.indexOf('  .mon-as{'), src.indexOf('  .mon-as{') + 240);
    expect(as).toMatch(/text-transform:uppercase/);
  });

  // THE CAPITALS ARE THE STYLESHEET'S, NOT THE STRINGS'. `text-transform` leaves
  // the accessibility tree alone, so the repository's plain voice survives in the
  // one place it has to — what a screen reader says, and what this file's own
  // `toContain('Main screen')` above reads.
  it('the words themselves stay in Relay’s voice', async () => {
    cap.live.set({ reference: 'Romans 8:28', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    expect(host.querySelector('.mon.prog .mon-as').textContent).toContain('as Main screen');
    // The markup interpolates the channel's own name in Relay's own sentence;
    // nothing anywhere writes the capitals out. (The comment above the element
    // quotes the prototype's rendered head, which is why this reads the ELEMENT
    // rather than the file.)
    const src = liveSrc();
    const el = src.slice(src.indexOf('<span class="mon-as'), src.indexOf('<span class="mon-as') + 420);
    expect(el).toContain('{programme.label}');
    expect(el).not.toMatch(/AS MAIN SCREEN/);
    // The sentence itself is composed in `channelroles.js`, which is now the one
    // place that decides WHICH screen this pane is a preview of (DECISIONS §89).
    // It is read here too, because the words moving out of this file is exactly
    // how a capitalised string could reappear without this test noticing.
    const roles = readFileSync(resolve(__dirname, 'channelroles.js'), 'utf8');
    expect(roles).toContain('`as ${byRole.name}`');
    expect(roles).not.toMatch(/AS MAIN SCREEN|`AS \$/);
  });

  // AND THE HEAD SAYS WHEN IT IS GUESSING. The old expression answered the same
  // way whether a main screen had been chosen, renamed or deleted — one sentence
  // over three different situations, which is rule 35. The fallback is kept
  // because a blank programme pane is a worse answer; it is drawn as a fallback.
  it('says so when no screen has been set as the main screen', async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels')
        return Promise.resolve([{ ...CHANNEL, role: null }]);
      if (cmd === 'list_templates') return Promise.resolve([TPL]);
      if (cmd === 'list_plans') return Promise.resolve([]);
      if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      if (cmd === 'rehearsal') return Promise.resolve(false);
      if (cmd === 'get_sensitivity') return Promise.resolve(50);
      return Promise.resolve(null);
    });
    new Live({ target: host, props: {} });
    await settle();
    const el = host.querySelector('.mon.prog .mon-as');
    expect(el.textContent).toContain('no main screen set');
    expect(el.className).toContain('guessed');
  });

  // The reference is the one figure on this head read from across a booth.
  it('the reference is set in the mono face and hard right', async () => {
    cap.live.set({ reference: 'Romans 8:28', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    const ref = host.querySelector('.mon.prog .mon-name');
    expect(ref.className).toContain('r-mono');
    // …and it is still the LAST thing in the head, after the spring.
    const head = [...host.querySelectorAll('.mon.prog .mon-bar > *')];
    expect(head[head.length - 1]).toBe(ref);
    // Amber only because a congregation is looking at it — the law is untouched.
    expect(ref.className).toContain('live');
  });

  // The separator joins two facts that are BOTH present, and carries no value of
  // its own, so it is never read aloud.
  it('the separator is never read aloud', async () => {
    cap.live.set({ reference: 'Romans 8:28', text: 'x', translation: 'KJV' });
    new Live({ target: host, props: {} });
    await settle();
    const sep = host.querySelector('.mon.prog .mon-sep');
    expect(sep).not.toBeNull();
    expect(sep.getAttribute('aria-hidden')).toBe('true');
  });

  // A monitor with nothing on it is a machine reporting about itself. The
  // prototype sets that in mono; ours was in the body face, so an empty screen
  // read like a sentence someone had written.
  it('an empty monitor speaks in the machine’s face, without an em dash', () => {
    const src = liveSrc();
    const blank = src.slice(src.indexOf('  .screen-empty{'), src.indexOf('  .screen-empty{') + 320);
    expect(blank).toMatch(/font-family:var\(--f-mono\)/);
    // The prototype writes `— nothing cued —`; this repository does not use the
    // dash, and this file's own em-dash rule above is the reason.
    expect(src).not.toMatch(/—\s*[Nn]othing cued/);
  });
});

describe('L2 · the slides head says what it is and what a press does', () => {
  const liveSrc = () => readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  // `14 Sep`, not `2026-09-14`. The year on a head that names the plan being run
  // right now reads as a record id and eats the width the plan's NAME needs.
  it('a plan date on this head is short', async () => {
    const { shortDate } = await import('./views/Live.svelte');
    expect(shortDate('2026-09-14')).toBe('14 Sep');
    expect(shortDate('2026-01-02')).toBe('2 Jan');
    expect(shortDate('2026-12-31')).toBe('31 Dec');
  });

  // NOTHING IS REINTERPRETED. A hand-edited row or an import can hold whatever a
  // person typed, and a head that quietly reshaped it would print a date nobody
  // entered. Absence is still absence — `{#if gridSubtitle}` draws nothing.
  it('anything that is not a plain ISO date comes back verbatim', async () => {
    const { shortDate } = await import('./views/Live.svelte');
    expect(shortDate('Sunday morning')).toBe('Sunday morning');
    expect(shortDate('2026-13-01')).toBe('2026-13-01');
    expect(shortDate('14/09/2026')).toBe('14/09/2026');
    expect(shortDate(undefined)).toBe('');
    expect(shortDate(null)).toBe('');
  });

  // WEST OF GREENWICH THIS IS THE WHOLE BUG. `new Date('2026-09-14')` is UTC
  // midnight, and rendering it locally moves it to the 13th — a plan dated the
  // day before itself, on the head an operator runs the service from.
  it('the date is parsed as digits, never through the Date constructor', () => {
    const src = liveSrc();
    const at = src.indexOf('export function shortDate(');
    const fn = src.slice(at, at + 500);
    expect(fn).not.toMatch(/new Date\(/);
    expect(fn).not.toMatch(/toLocale[A-Za-z]*\s*\(/);
  });

  // The count and the sentence were two spans in two faces; the prototype sets
  // them as one mono run with the count leading, because "how many" is what an
  // operator is looking for when they glance here mid-service.
  it('the count LEADS one line, and that line is the hint', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const hint = host.querySelector('.sg-head .sg-hint');
    expect(hint).not.toBeNull();
    const words = hint.textContent.replace(/\s+/g, ' ').trim();
    expect(words).toMatch(/^\d+ · single click goes to air · double click previews$/);
    // The count is the first thing in it and is the grid's own number.
    expect(hint.querySelector('.cnt').textContent.trim())
      .toBe(String(host.querySelectorAll('.sg-cell').length));
    // …and there is no SECOND count left behind outside the line.
    expect(host.querySelectorAll('.sg-head .cnt')).toHaveLength(1);
  });

  // ITEM 9. The view controls were the loudest thing in a browsing rail whose
  // whole job is finding a verse, and the prototype's rail carries nothing of the
  // kind. They moved to the head of the pane they actually reclaim space for.
  //
  // T2: the pair became ONE. `Normal | Compact` was removed on the operator's
  // instruction, and this test's real subject is the PLACE, so it keeps that and
  // narrows its claim rather than being deleted with the control.
  //
  // WAVE 4: a second control joined it, and the list is asserted exactly so the
  // arrival is deliberate rather than absorbed. The slide sizer belongs in this
  // slot for the same reason Full screen does — it changes how the console LOOKS
  // and never what reaches a screen — and it is deliberately NOT the density
  // segment coming back: that changed spacing and type, this changes the width of
  // the picture an operator is reading the words off. `slidesizer.test.js` holds
  // its behaviour, including that it moves the grid track and not `.sg-thumb`.
  it('the full-screen control left the rail and is still reachable', async () => {
    new Live({ target: host, props: {} });
    await settle();
    // Gone from the rail column.
    expect(host.querySelector('.rail-col .view-ctl')).toBeNull();
    // Present on the slides head, with its name intact.
    const ctl = host.querySelector('.sg-head .view-ctl');
    expect(ctl).not.toBeNull();
    expect([...ctl.querySelectorAll('button')].map((b) => b.textContent.trim()))
      .toEqual(['−', '+', 'Full screen']);
    // The two glyphs are NAMED — a bare − is punctuation to a screen reader.
    expect([...ctl.querySelectorAll('button')].map((b) => b.getAttribute('aria-label')))
      .toEqual(['Smaller slide cells', 'Bigger slide cells', null]);
  });

  // ── THE DENSITY CONTROL IS DELETED, NOT HIDDEN (T2) ──────────────────────
  //
  // A removal pass is exactly where a control goes missing instead of going
  // away: the segment stops rendering, the handler and the CSS stay, and the
  // next reader finds half a feature and cannot tell which half was intended.
  // So this asserts the absence on all four surfaces at once — the markup, the
  // script, the stylesheet, and the persisted session key that fed it.
  it('nothing of the density control is left behind', async () => {
    new Live({ target: host, props: {} });
    await settle();
    // No segment on the slides head, and no density group anywhere.
    //
    // NOT a bare `.seg` query, and the reason is worth writing down: `LiveRail`
    // renders `class="seg lr-seg"`, a vestigial token that matches no rule in
    // the app — Svelte scopes styles per component, so Live's `.seg` never
    // reached the rail, which is why the rail has its own `.lr-seg` rules. A
    // bare query here would fail on the rail's collection switch and say
    // nothing about the control this test is actually about.
    expect(host.querySelector('.sg-head .seg')).toBeNull();
    expect(host.querySelector('[aria-label="Console density"]')).toBeNull();
    const labels = [...host.querySelectorAll('button')].map((b) => b.textContent.trim());
    expect(labels).not.toContain('Compact');
    expect(labels).not.toContain('Normal');
    // …and the root no longer carries the class those rules hung off.
    expect(host.querySelector('.con.compact')).toBeNull();

    // The source, because a dead rule renders as nothing and asserting on the
    // DOM alone cannot tell "removed" from "never matched".
    const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    expect(src).not.toMatch(/\$session\.liveDensity/);
    expect(src).not.toMatch(/setDensity/);
    // Both names survive in PROSE about their removal — that is the point of
    // the prose — so these look for a RULE: the name at the start of a line,
    // which is the only place a selector can begin in this stylesheet.
    expect(src).not.toMatch(/^\s*\.con\.compact[\s{:]/m);
    expect(src).not.toMatch(/^\s*\.seg[\s{:]/m);
  });
});

describe('L2 · the transport says what it walks, in the rack’s own face', () => {
  it('the caption is mono capitals over two lines', () => {
    const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const at = src.indexOf('  .rack-cap{');
    const rule = src.slice(at, at + 300);
    expect(rule).toMatch(/font-family:var\(--f-mono\)/);
    expect(rule).toMatch(/text-transform:uppercase/);
    // The break is decided here rather than left to whichever font loaded: at
    // mono capitals the phrase is within a pixel or two of the rack's width.
    expect(src).toMatch(/walks the<br \/>programme/);
  });

  // THE MODE BADGE IS RELAY'S AND STAYS. The prototype has no equivalent, and
  // the same key silently meaning two things is how the wrong thing reaches a
  // congregation (CLAUDE.md — the transport is MODE-AWARE and says so).
  it('the mode badge survives the prototype’s caption', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const mode = host.querySelector('.rack .rack-mode');
    expect(mode).not.toBeNull();
    expect(['SLIDE', 'VERSE']).toContain(mode.textContent.trim());
  });
});

describe('L2 · a press answers on the way down', () => {
  const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
  const NO_PREF = '@media (prefers-reduced-motion: no-preference){';
  const REDUCE = '@media (prefers-reduced-motion: reduce){';

  // `:active` begins at pointerdown and ends at release — the same beat the
  // prototype reproduces by adding `.press` on a pointerdown listener. Waiting
  // for `click` puts the feedback after the action it is feedback for.
  it('TAKE, the arrows and a cell all answer on `:active`', () => {
    const motion = src.slice(src.indexOf(NO_PREF), src.indexOf(REDUCE));
    expect(motion).toMatch(/\.take:active:not\(:disabled\)\{transform:scale\(\.955\)\}/);
    expect(motion).toMatch(/\.rk:active:not\(:disabled\)\{transform:scale\(\.96\)\}/);
    expect(motion).toMatch(/\.sg-cell:active:not\(:disabled\) \.sg-thumb\{transform:scale\(\.985\)\}/);
  });

  // ONLY transform, opacity and filter — the three the compositor can do without
  // a layout pass. A press that reflowed the grid would be worse than none.
  it('nothing but a transform moves, and nothing animates a box', () => {
    const motion = src.slice(src.indexOf(NO_PREF), src.indexOf(REDUCE));
    expect(motion).not.toMatch(/(width|height|margin|padding|top|left):/);
  });

  // REDUCED MOTION IS A CUT, NOT A DELETION. An operator who asked for no
  // animation still has to be able to tell a press from a dead button, so the
  // same press reads as a brightness step instead of a movement.
  it('reduced motion keeps the feedback and drops the movement', () => {
    const at = src.indexOf(REDUCE);
    // END AT THE BLOCK'S OWN BRACE, not at whatever rule happens to follow it.
    // This read to `'\n  .rk{'` until B1 folded `.rk` into the shared `.r-btn`
    // and deleted that rule — `indexOf` then returned -1, the slice ran to the
    // end of the file, and a test about six lines started reporting on six
    // hundred. It failed loudly here, which is luck: the same anchor could as
    // easily have slipped forward over a `transform` and passed.
    const end = src.indexOf('\n  }', at);
    expect(end, 'the reduced-motion block is not closed where expected').toBeGreaterThan(at);
    const block = src.slice(at, end);
    // The scanner can still SEE the rules it is judging — without this, a slice
    // that narrowed to nothing would satisfy the `not.toMatch` below and report
    // a guarantee it never checked.
    expect(block).toMatch(/\.take:active/);
    expect(block).toMatch(/\.rk:active/);
    expect(block).toMatch(/filter:brightness/);
    expect(block).not.toMatch(/transform:/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 7 · L2 ROUND 2 — FOUR TEXT DEFECTS, MEASURED ON A RENDER AT 2000×1175
//
// The branch was rendered against the prototype screenshot. Everything here is a
// defect in L2's own work that no existing test could see, because each one is a
// string composed at RUNTIME out of parts that are individually correct.
// ─────────────────────────────────────────────────────────────────────────────

// A plan whose NAME already carries its date — the shape the render caught, and
// the one a church actually types. `announce`, `media` and `scripture` cues, so
// the kind chips can be read off the grid in the same mount.
const DATED_PLAN = { id: 1, title: 'Sunday morning · 7 September', plan_date: '2026-09-07', cue_count: 3 };
const PLAIN_PLAN = { id: 1, title: 'Sunday morning', plan_date: '2026-09-07', cue_count: 3 };
const PLAN_CUES = [
  { id: 11, cue_type: 'announce', label: 'Welcome & Notices', payload_json: '{"body":"Welcome"}' },
  { id: 12, cue_type: 'scripture', label: 'Psalms 23:1', payload_json: '{"reference":"Psalms 23:1","text":"The LORD is my shepherd","verse":1}' },
  { id: 13, cue_type: 'media', label: 'Opening loop', payload_json: '{}' },
];

async function mountPlan(plan = DATED_PLAN) {
  const sess = await import('./session.js');
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
    if (cmd === 'list_templates') return Promise.resolve([TPL]);
    if (cmd === 'list_plans') return Promise.resolve([plan]);
    if (cmd === 'plan_items') return Promise.resolve(PLAN_CUES);
    if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
    if (cmd === 'rehearsal') return Promise.resolve(false);
    if (cmd === 'get_sensitivity') return Promise.resolve(50);
    // The repeat badge's own question. A non-zero answer to ANY reference, so a
    // badge that should not be drawn cannot hide behind a zero.
    if (cmd === 'verse_repeat_count') return Promise.resolve(2);
    return Promise.resolve(null);
  });
  sess.setSession({ planId: plan.id, liveCueId: null, liveSlide: 0, liveOnAir: false });
  const app = new Live({ target: host, props: {} });
  await settle(140);
  return { app, sess };
}

describe('L2/2 · one date, said once', () => {
  it('a plan whose NAME carries the date does not get it again', async () => {
    const { sess } = await mountPlan(DATED_PLAN);
    const caps = [...host.querySelectorAll('.sg-head .sg-cap')].map((e) => e.textContent.trim());
    // The plan's own name, and nothing after it.
    expect(caps).toEqual(['· Sunday morning · 7 September']);
    // Said plainly: the head does not print September twice.
    const head = host.querySelector('.sg-head').textContent;
    expect(head.match(/Sep/gi) ?? []).toHaveLength(1);
    sess.setSession({ planId: null });
  });

  it('…and a plan named without one still gets the short form', async () => {
    const { sess } = await mountPlan(PLAIN_PLAN);
    const caps = [...host.querySelectorAll('.sg-head .sg-cap')].map((e) => e.textContent.trim());
    expect(caps).toEqual(['· Sunday morning', '· 7 Sep']);
    sess.setSession({ planId: null });
  });

  it('recognises the forms a name would state a date in', async () => {
    const { dateStatedIn } = await import('./views/Live.svelte');
    for (const title of [
      'Sunday morning · 7 September',
      'Sunday morning · 7 Sep',
      'Sunday 7th September',
      'September 7 — evening',
      'Carols 2026-09-07',
      'SUNDAY MORNING · 7 SEPTEMBER',
    ]) {
      expect(dateStatedIn(title, '2026-09-07')).toBe(true);
    }
  });

  // A MONTH ALONE IS NOT A DATE. `September series` beside a plan dated the 7th
  // is two different facts, and the head must keep printing the second.
  it('and does not mistake a month, a year or another day for this date', async () => {
    const { dateStatedIn } = await import('./views/Live.svelte');
    expect(dateStatedIn('September series', '2026-09-07')).toBe(false);
    expect(dateStatedIn('Sunday morning', '2026-09-07')).toBe(false);
    expect(dateStatedIn('Sunday morning · 8 September', '2026-09-07')).toBe(false);
    expect(dateStatedIn('Harvest 2026', '2026-09-07')).toBe(false);
    // Nothing to compare is never a match — an absent date must not silently
    // suppress a cap that would have said something.
    expect(dateStatedIn('Sunday morning', '')).toBe(false);
    expect(dateStatedIn('', '2026-09-07')).toBe(false);
    expect(dateStatedIn(null, '2026-09-07')).toBe(false);
  });
});

describe('L2/2 · the preview head names the cue once', () => {
  // A one-slide cue names its only slide after itself, and the head joined the
  // two with a `·`: `WELCOME & NOTICES · WELCOME & NOTICES`. A separator between
  // a thing and itself invents a second fact out of one.
  it('a one-slide cue is not joined to itself', async () => {
    const { sess } = await mountPlan();
    const name = host.querySelector('.mon.prev .mon-name');
    expect(name).not.toBeNull();
    expect(name.textContent.trim()).toBe('Welcome & Notices');
    expect(name.textContent).not.toMatch(/·/);
    sess.setSession({ planId: null });
  });

  // …AND A CUE WITH REAL SLIDE NAMES STILL SHOWS BOTH. The join is what an
  // operator needs for `Amazing Grace · Verse 2`; only the echo is wrong.
  it('two real names are still joined', async () => {
    const { cueLabel } = await import('./views/Live.svelte');
    expect(cueLabel('Amazing Grace', 'Verse 2')).toBe('Amazing Grace · Verse 2');
    expect(cueLabel('Welcome & Notices', 'Welcome & Notices')).toBe('Welcome & Notices');
    expect(cueLabel('Welcome', ' welcome ')).toBe('Welcome');
    expect(cueLabel('Welcome', '')).toBe('Welcome');
    expect(cueLabel('', 'Verse 2')).toBe('Verse 2');
  });

  // `shown 2×` IS A CLAIM ABOUT A VERSE. It was asked about `previewLabel`, which
  // for a plan cue is a composed display name — so `verseRepeatCount` was put a
  // question about a NOTICE and the head printed its answer. The badge is right
  // for a verse and meaningless for a notice; it is now gated on there being a
  // reference to have repeated.
  it('no repeat badge over a cue that has no reference to repeat', async () => {
    const { sess } = await mountPlan();
    expect(host.querySelector('.mon.prev .mon-name').textContent.trim()).toBe('Welcome & Notices');
    expect(host.querySelector('.mon.prev .mon-repeat')).toBeNull();
    // …and the question was never asked, so it is not merely hidden.
    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('verse_repeat_count');
    sess.setSession({ planId: null });
  });

  it('but a detected verse still carries it', async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
      if (cmd === 'list_templates') return Promise.resolve([TPL]);
      if (cmd === 'list_plans') return Promise.resolve([]);
      if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      if (cmd === 'rehearsal') return Promise.resolve(false);
      if (cmd === 'get_sensitivity') return Promise.resolve(50);
      if (cmd === 'verse_repeat_count') return Promise.resolve(2);
      return Promise.resolve(null);
    });
    cap.detections.set([claim()]);
    new Live({ target: host, props: {} });
    await settle(140);
    const badge = host.querySelector('.mon.prev .mon-repeat');
    expect(badge).not.toBeNull();
    expect(badge.textContent.trim()).toBe('shown 2×');
  });
});

describe('L2/2 · a cell says its kind in a whole word', () => {
  // `NOTE`, `SCR`, `BG`. The last names nothing an operator would recognise, and
  // all three are abbreviations `plan.js::slidesOf` invents for the plan rail's
  // narrow chip — where `plan.js`'s own table already records that a truncation
  // is "a name nobody chose".
  it('the chips are words, not three-letter inventions', async () => {
    const { sess } = await mountPlan();
    const tags = [...host.querySelectorAll('.sg-cell .sg-tag')].map((e) => e.textContent.trim());
    expect(tags).toEqual(['NOTICE', 'SCRIPTURE', 'MEDIA']);
    for (const dead of ['NOTE', 'SCR', 'BG']) expect(tags).not.toContain(dead);
    sess.setSession({ planId: null });
  });

  // ONE DOOR (rule 36). A second table here is how a cell and a running-order row
  // come to disagree about what a cue is.
  it('the word comes from plan.js’s one taxonomy, not from a second table here', () => {
    const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    expect(src).toMatch(/const kindOf = \(c\) => typeOf\(c\?\.ctype\)\.label;/);
    // No hand-written kind words anywhere in this file.
    expect(src).not.toMatch(/'SCRIPTURE'|'NOTICE'|'MEDIA'|'COUNTDOWN'/);
  });

  // AN UNRECOGNISED CUE SAYS SO. Never a fallback to scripture — the one kind the
  // AI is allowed to fire by itself (`plan.js`, `typeOf`).
  it('a cue_type this build does not know reads UNKNOWN', async () => {
    const sess = await import('./session.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
      if (cmd === 'list_templates') return Promise.resolve([TPL]);
      if (cmd === 'list_plans') return Promise.resolve([PLAIN_PLAN]);
      if (cmd === 'plan_items')
        return Promise.resolve([
          { id: 21, cue_type: 'announcement', label: 'From an older build', payload_json: '{"body":"x"}' },
        ]);
      if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      if (cmd === 'rehearsal') return Promise.resolve(false);
      if (cmd === 'get_sensitivity') return Promise.resolve(50);
      return Promise.resolve(null);
    });
    sess.setSession({ planId: 1, liveCueId: null, liveSlide: 0, liveOnAir: false });
    new Live({ target: host, props: {} });
    await settle(140);
    const tags = [...host.querySelectorAll('.sg-cell .sg-tag')].map((e) => e.textContent.trim());
    expect(tags).toEqual(['UNKNOWN']);
    sess.setSession({ planId: null });
  });

  // A WHOLE WORD THAT WILL NOT FIT IS TRUNCATED, NOT RENAMED.
  it('the chip ellipsis rather than overflowing its cell', () => {
    const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const at = src.indexOf('  .sg-tag{');
    const rule = src.slice(at, at + 420);
    expect(rule).toMatch(/max-width:calc\(100% - 10px\)/);
    expect(rule).toMatch(/text-overflow:ellipsis/);
  });
});

describe('L2/2 · the slides head at a booth laptop’s width', () => {
  const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  // The head carries five things and fits at 2000px. Below that ONE of them has
  // to yield, and it must be the teaching sentence — not `Close plan`, which
  // stops a plan running, and not the view controls, which are real features.
  it('the sentence yields first, by rule rather than by luck', () => {
    const rung = src.slice(src.indexOf('@media (max-width:1400px){'));
    const block = rung.slice(0, rung.indexOf('}\n  /*'));
    expect(block).toMatch(/\.sg-say\{display:none\}/);
    // Nothing else in the head is touched by that rung.
    expect(block).not.toMatch(/\.mini|\.view-ctl|\.view-fs|\.cnt|\.sg-head h2/);
  });

  // THE COUNT IS A FACT AND STAYS AT EVERY WIDTH. It is a separate span from the
  // sentence for exactly this reason; one span could not do both.
  it('the count is not inside the part that hides', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const hint = host.querySelector('.sg-head .sg-hint');
    expect(hint.querySelector('.cnt')).not.toBeNull();
    expect(hint.querySelector('.sg-say .cnt')).toBeNull();
    // …and the sentence keeps its leading space, so the two do not run together
    // for a screen reader. Svelte drops a literal one at an element boundary,
    // which is why the text is an expression.
    expect(hint.textContent.replace(/\s+/g, ' ')).toContain('0 · single click');
  });

  // NOTHING IS LOST TO A HOVER OR A SCREEN READER — the ladder's own rule.
  it('the sentence survives on the hint and on every cell', async () => {
    const { sess } = await mountPlan();
    expect(host.querySelector('.sg-head .sg-hint').getAttribute('title'))
      .toMatch(/single click sends the slide to the programme/i);
    const cell = host.querySelector('.sg-cell:not(:disabled)');
    expect(cell.getAttribute('title')).toMatch(/double click to preview/i);
    sess.setSession({ planId: null });
  });

  // `Close plan` never wraps or shrinks — it is the way OUT of a running plan.
  it('Close plan holds its size in the head', async () => {
    const { sess } = await mountPlan();
    expect(host.querySelector('.sg-head .mini.ghost').textContent.trim()).toBe('Close plan');
    // `:global(...)`, because `.mini` is now a class on the shared
    // `ui/Button.svelte` rather than on an element in this file, and Svelte drops
    // a scoped selector that matches nothing in the component's own markup. The
    // claim is unchanged: the way OUT of a running plan does not shrink or wrap
    // when the head gets crowded. The MOUNTED half above is the stronger of the
    // two assertions and is untouched.
    expect(src).toMatch(/\.sg-head :global\(\.mini\)\{flex:0 0 auto\}/);
    sess.setSession({ planId: null });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// N · THE INSPECTOR ACTS ON THE CLAIM IT OPENED
// ─────────────────────────────────────────────────────────────────────────────
//
// `DetectionInspector`'s own doc comment states the contract: "Interrogate ONE
// claim — the card that was pressed, not `dets[0]`." Opening kept it. Acting did
// not: the footer's two buttons called `acceptTop`/`dismissTop`, which are
// `dets[0]` — correct for the `A` and `D` keys, wrong for a button inside a panel
// about a different verse.
//
// So an operator who opened "why this match?" on the SECOND card and pressed the
// amber "Accept & fire" put the FIRST card's verse on the congregation's screens.
// This is the surface people open because they are being careful.
//
// Both tests below were watched to fail against `await acceptTop()` /
// `await dismissTop()`, which is the whole point of writing them.
describe('the detection inspector acts on the claim that was opened', () => {
  const twoClaims = [
    claim({ reference: 'Romans 8:28' }),
    claim({ reference: 'Psalms 23:1', matched_text: 'psalm twenty three' }),
  ];

  async function openSecondClaim() {
    cap.detections.set(twoClaims);
    new Live({ target: host, props: {} });
    await settle();
    const links = [...host.querySelectorAll('.inspect-link')];
    expect(links.length).toBeGreaterThanOrEqual(2);
    links[1].click(); // the SECOND card — not dets[0]
    await settle();
    return host.querySelector('[role="dialog"]');
  }

  it('Accept & fire sends the inspected verse, not the top card', async () => {
    const dlg = await openSecondClaim();
    expect(dlg).not.toBeNull();
    expect(dlg.textContent).toContain('Psalms 23:1');

    invoke.mockClear();
    [...dlg.querySelectorAll('button')]
      .find((b) => /accept/i.test(b.textContent))
      .click();
    await settle();

    const fired = invoke.mock.calls.filter(([c]) => c === 'confirm_detection');
    expect(fired.length).toBe(1);
    expect(fired[0][1].reference).toBe('Psalms 23:1');
  });

  it('Dismiss drops the inspected claim, not the top card', async () => {
    const dlg = await openSecondClaim();
    invoke.mockClear();
    [...dlg.querySelectorAll('button')]
      .find((b) => /dismiss/i.test(b.textContent))
      .click();
    await settle();

    const dropped = invoke.mock.calls.filter(([c]) => c === 'dismiss_detection');
    expect(dropped.length).toBe(1);
    expect(dropped[0][1].reference).toBe('Psalms 23:1');
  });
});
