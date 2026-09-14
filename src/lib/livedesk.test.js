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
// 3 · THE SCREENS MOVED, AND NOTHING WAS LEFT BEHIND
//
// Output Status left the studio row so the two monitors could be equal. It has
// rule 35's whole history behind it, so the move is only defensible if every
// control it carried is still on the run surface — the per-screen badge, the
// screen's own word, the on/off REPAIR (which is what an operator reaches for
// when a projector drops mid-service), the emergency announcement, and Open main
// output. This is the test that says so.
// ─────────────────────────────────────────────────────────────────────────────
describe('the inspector column', () => {
  it('sits beside the stage and holds the claims ABOVE the screens', async () => {
    new Live({ target: host, props: {} });
    await settle();
    const insp = host.querySelector('.insp-col');
    expect(insp).not.toBeNull();
    expect(insp.parentElement.className).toContain('desk');
    expect(insp.previousElementSibling.className).toContain('stage');

    const panes = [...insp.children].filter((e) => e.className.includes('pane'));
    expect(panes).toHaveLength(2);
    expect(panes[0].textContent).toContain('AI Detection');
    expect(panes[1].textContent).toContain('Output Status');
  });

  it('the screens pane kept every control it had in the studio row', async () => {
    // A screen that is OFF — the state the repair exists for. With no health row
    // at all `screenSwitch` deliberately offers nothing ("we have not asked yet"
    // is not "it is off"), so the pane would have nothing to press through no
    // fault of this change.
    cap.channelHealth.set({ 1: { id: 1, name: 'Main screen', supported: true, online: false } });
    new Live({ target: host, props: {} });
    await settle();
    const screens = [...host.querySelectorAll('.insp-col .pane')].at(-1);
    expect(screens.textContent).toContain('Main screen');
    // The repair, the emergency announcement, and the way to open the wall.
    expect(screens.querySelector('.out-sw')).not.toBeNull();
    expect(screens.querySelector('input[aria-label="Emergency announcement"]')).not.toBeNull();
    expect(screens.textContent).toContain('Open main output');
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
  it('a screen NAME gets its own row and one line — "Streaming" is not "Stream"/"ing"', async () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const rule = live.slice(live.indexOf('  .out-nm{'), live.indexOf('  .out-meta{'));
    // `text-overflow` does nothing without a block-level box, which is how the
    // chrome lamps hit the same wall.
    expect(rule).toMatch(/display:block/);
    expect(rule).toMatch(/white-space:nowrap/);
    expect(rule).toMatch(/text-overflow:ellipsis/);
    // The wrap rule that broke it mid-word is gone from the NAME.
    expect(rule).not.toMatch(/overflow-wrap:anywhere/);
    expect(rule).not.toMatch(/line-clamp/);

    // And in the DOM the name is alone on its grid row, with the badge below.
    cap.channelHealth.set({ 1: { id: 1, name: 'Main screen', supported: true, online: false } });
    new Live({ target: host, props: {} });
    await settle();
    const row = host.querySelector('.out');
    expect(row.querySelector('.out-nm').nextElementSibling.className).toContain('out-meta');
    // The whole name is still reachable, whatever the width does to it.
    expect(row.querySelector('.out-nm').getAttribute('title')).toBe('Main screen');
  });

  it('the plan panel SAYS it has more below it', async () => {
    const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
    const rule = live.slice(live.indexOf('  .plan{'), live.indexOf('  .rail{'));
    // The same `background-attachment: local` recipe `.outs` uses: the shadow at
    // an edge appears only while there is content past it.
    expect(rule).toMatch(/no-repeat local/);
    expect(rule).toMatch(/no-repeat scroll/);
    // And the body it sits in is a real scroller, not an overflow.
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
    expect(host.querySelector('.mon.prog .mon-name').className).not.toContain('live');
  });
});
