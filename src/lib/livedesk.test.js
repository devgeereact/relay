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
const CHANNEL = { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 1 };

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
    // The emergency announcement, two-step, now on every workspace.
    expect(dock).toMatch(/aria-label="Emergency announcement"/);
    expect(dock).toMatch(/annArmed \? 'Confirm\?' : 'Send'/);
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

  // Arm/disarm is a different question from HOW READILY, and it belongs on the
  // panel it is about. Only the dial moved.
  it('but the gate can still be armed and disarmed from the claim panel', async () => {
    cap.capture.update((s) => ({ ...s, detectionOn: true }));
    new Live({ target: host, props: {} });
    await settle();
    const chip = host.querySelector('.det-ctl .btnchip');
    expect(chip).not.toBeNull();
    expect(chip.textContent).toContain('Armed');
    cap.capture.update((s) => ({ ...s, detectionOn: false }));
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
