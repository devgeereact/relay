// DELETING FROM THE RUNNING ORDER — DIRECT, COUNTED, AND NEVER A NATIVE DIALOG.
//
// The Planner could only delete a cue from the INSPECTOR, so removing three cues
// was three select-then-travel round trips across the desk. The operator drew the
// line themselves: *"do not hide destructive deletion behind ambiguity, but do
// not add friction that slows a live operator either."*
//
// ── WHAT THIS FILE HOLDS ────────────────────────────────────────────────────
//
//   1 · the selection algebra — one door for all three modifier combinations,
//       because three handlers deciding it separately is how the fourth gets it
//       wrong;
//   2 · where the inspector lands after a run is deleted, which is the case that
//       actually costs an operator something;
//   3 · the WORDS on a destructive control, which is the whole "not ambiguous"
//       half: one cue is named, several are counted, and the armed step restates
//       the fact rather than replacing it with a bare "Click again";
//   4 · RULE 41, on the surface that now has two destructive controls instead of
//       one. Tauri's webview does not implement `confirm()` — it returns `false`
//       without showing anything, so a two-step delete guarded by one deletes
//       NOTHING and reports success. `hardrules.test.js` holds this across the
//       tree; it is asserted here too because this change is the reason the
//       temptation exists on this file;
//   5 · `duplicateCue`'s defect: a comment claiming a cue is copied "in place"
//       over code that appends it to the end of the plan.
//
// ── HOW EACH WAS CHECKED ────────────────────────────────────────────────────
//
// Test the bug, not the fix. Each was watched red against a defect the obvious
// implementation has:
//
//   · a Shift-click with no anchor answering `[]` — the click is lost and
//     nothing says why.
//   · a range that moves its anchor to the last clicked row, so correcting an
//     overshoot starts a new run instead of shrinking the one you have.
//   · a picked set in CLICK order rather than plan order, so "Delete 3 cues"
//     counts right and reads wrong everywhere it is listed.
//   · `selectionAfterRemoval` answering `items[0]`, which is what the existing
//     single delete does: delete cue 12 of 14 and land back at cue 1.
//   · `deleteLabel(1)` saying "Delete" over a list of fourteen cues, which does
//     not say which one.
//   · `orderWithDuplicateInPlace` leaving the copy at the end — the shipped
//     behaviour, under a comment saying the opposite.
//
//   npx vitest run src/lib/planselect.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  selectionAfterClick,
  inPlanOrder,
  selectionAfterRemoval,
  deleteLabel,
  deletedMessage,
  addedMessage,
  orderWithDuplicateInPlace,
} from './planselect.js';
import { sectionsOf } from './plan.js';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { clearSession } = await import('./session.js');

// The self-detecting lifecycle gate the other Planner suites use. Without
// `resolve: { conditions: ['browser'] }` a mounted view fetches nothing, so these
// SKIP loudly rather than pass over a Planner that never loaded a plan.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, what, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
    await tick();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

const PLANNER = readFileSync(resolve(__dirname, './views/ServicePlanner.svelte'), 'utf8');

/** A plan of `n` cues, ids 1…n. */
const plan = (n) => Array.from({ length: n }, (_, i) => ({ id: i + 1, label: `Cue ${i + 1}` }));

// ─────────────────────────────────────────────────────────────────────────────
// 1 · ONE DOOR FOR THREE MODIFIER COMBINATIONS
// ─────────────────────────────────────────────────────────────────────────────
describe('what a click does to the picked set', () => {
  const items = plan(8);

  it('a PLAIN click is one cue, exactly as it was before multi-select existed', () => {
    // The case a hurried operator is always in. Nothing about this may change.
    const r = selectionAfterClick({ picked: [2, 3, 4], anchor: 2, items, id: 6 });
    expect(r.picked).toEqual([6]);
    expect(r.anchor).toBe(6);
  });

  it('CMD-click adds one, and clicking it again takes it back out', () => {
    let s = selectionAfterClick({ items, id: 2 });
    s = selectionAfterClick({ ...s, items, id: 5, additive: true });
    expect(s.picked).toEqual([2, 5]);
    s = selectionAfterClick({ ...s, items, id: 2, additive: true });
    expect(s.picked).toEqual([5]);
    // …down to nothing, which is a legitimate state and not a crash.
    s = selectionAfterClick({ ...s, items, id: 5, additive: true });
    expect(s.picked).toEqual([]);
  });

  it('SHIFT-click takes the RUN between the anchor and here, both ends included', () => {
    // "so a run of cues can go at once" — this is that.
    const s = selectionAfterClick({ picked: [3], anchor: 3, items, id: 6, range: true });
    expect(s.picked).toEqual([3, 4, 5, 6]);
  });

  it('…and it works upwards too', () => {
    const s = selectionAfterClick({ picked: [6], anchor: 6, items, id: 3, range: true });
    expect(s.picked).toEqual([3, 4, 5, 6]);
  });

  it('a second SHIFT-click SHRINKS the same run rather than starting a new one', () => {
    // The anchor is kept. An operator correcting an overshoot expects the run to
    // move, not to begin again from wherever the last one ended — which is how
    // every list in every operating system behaves.
    let s = selectionAfterClick({ picked: [2], anchor: 2, items, id: 7, range: true });
    expect(s.picked).toEqual([2, 3, 4, 5, 6, 7]);
    s = selectionAfterClick({ ...s, items, id: 4, range: true });
    expect(s.picked).toEqual([2, 3, 4]);
    expect(s.anchor).toBe(2);
  });

  it('a SHIFT-click with no anchor is a plain click, not a lost one', () => {
    // Shift-clicking as the first thing you do is a reasonable mistake, and
    // swallowing the click teaches nothing.
    const s = selectionAfterClick({ picked: [], anchor: null, items, id: 4, range: true });
    expect(s.picked).toEqual([4]);
    expect(s.anchor).toBe(4);
  });

  it('both modifiers at once takes the NARROWER answer', () => {
    // Ambiguous input, so the answer that cannot surprise somebody into picking
    // twenty cues wins.
    const s = selectionAfterClick({ picked: [1], anchor: 1, items, id: 7, additive: true, range: true });
    expect(s.picked).toEqual([1, 7]);
  });

  it('a click on a cue that is not in the plan changes nothing', () => {
    // A stale id from a row that has just been deleted under the pointer.
    const s = selectionAfterClick({ picked: [2, 3], anchor: 2, items, id: 99, additive: true });
    expect(s.picked).toEqual([2, 3]);
    expect(s.anchor).toBe(2);
  });

  it('the picked set is always in PLAN order, however it was clicked together', () => {
    // It is counted, named and deleted from — all three read top to bottom.
    let s = selectionAfterClick({ items, id: 7 });
    s = selectionAfterClick({ ...s, items, id: 2, additive: true });
    s = selectionAfterClick({ ...s, items, id: 5, additive: true });
    expect(s.picked).toEqual([2, 5, 7]);
    expect(inPlanOrder([8, 1, 4], items)).toEqual([1, 4, 8]);
    expect(inPlanOrder([4, 4, 1], items)).toEqual([1, 4]);
  });

  it('called with nothing at all it does not throw', () => {
    expect(selectionAfterClick()).toEqual({ picked: [], anchor: null });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · DELETING SOMETHING LEAVES YOU WHERE YOU WERE
// ─────────────────────────────────────────────────────────────────────────────
describe('what the inspector lands on after a delete', () => {
  const items = plan(14);

  it('deleting cue 12 of 14 lands on 13, NOT back at the top of the plan', () => {
    // The shipped behaviour is `items[0]`, which is a scroll and a hunt every
    // time, on the surface whose whole job is a list you work down.
    expect(selectionAfterRemoval(items, [12])).toBe(13);
  });

  it('deleting a run lands on the first cue after it', () => {
    expect(selectionAfterRemoval(items, [4, 5, 6])).toBe(7);
  });

  it('a run that is not contiguous still lands after its LAST member', () => {
    expect(selectionAfterRemoval(items, [2, 9])).toBe(10);
  });

  it('deleting the end of the plan lands on the new end', () => {
    expect(selectionAfterRemoval(items, [13, 14])).toBe(12);
    expect(selectionAfterRemoval(items, [14])).toBe(13);
  });

  it('deleting everything lands on nothing, and says so', () => {
    // `null` is a real answer. Falling back to `items[0]` here is what puts a
    // stale id in an inspector over a plan with nothing in it.
    expect(selectionAfterRemoval(items, items.map((i) => i.id))).toBeNull();
    expect(selectionAfterRemoval([], [1])).toBeNull();
    expect(selectionAfterRemoval(null, null)).toBeNull();
  });

  it('deleting NOTHING decides nothing, rather than moving to cue 1', () => {
    // The one input where there is no reason to move anybody. Answering the top
    // of the plan here would be this function's own bug, on its own doorstep.
    expect(selectionAfterRemoval(items, [])).toBeNull();
    expect(selectionAfterRemoval(items, null)).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · THE WORDS ON A DESTRUCTIVE CONTROL
// ─────────────────────────────────────────────────────────────────────────────
describe('a delete says what it will take before it takes it', () => {
  it('one cue is NAMED — “Delete” alone does not say which of fourteen', () => {
    expect(deleteLabel(1, 'Great Is Thy Faithfulness')).toBe('Delete “Great Is Thy Faithfulness”');
  });

  it('several cues are COUNTED', () => {
    expect(deleteLabel(3, 'Cue 4')).toBe('Delete 3 cues');
    expect(deleteLabel(12)).toBe('Delete 12 cues');
  });

  it('the armed step RESTATES the fact rather than replacing it with “Click again”', () => {
    // The second press is the one that actually does it, so it is the press that
    // most needs to know what it is about.
    expect(deleteLabel(1, 'Welcome', true)).toBe('Delete “Welcome” — click again');
    expect(deleteLabel(4, '', true)).toBe('Delete 4 cues — click again');
  });

  it('a very long cue name is trimmed rather than reflowing the pane', () => {
    const long = deleteLabel(1, 'Announcement about the church picnic on Saturday afternoon');
    expect(long.length).toBeLessThan(45);
    expect(long).toMatch(/…”$/);
  });

  it('a nameless single cue still reads as a sentence', () => {
    expect(deleteLabel(1, '')).toBe('Delete “this cue”'.replace('“this cue”', 'this cue'));
    expect(deleteLabel(1, '   ')).toBe('Delete this cue');
  });

  it('nothing picked is a plain label, not “Delete 0 cues”', () => {
    expect(deleteLabel(0)).toBe('Delete');
    expect(deleteLabel(NaN)).toBe('Delete');
    expect(deleteLabel(-2)).toBe('Delete');
  });

  it('the report afterwards is in the past tense and counts the same way', () => {
    expect(deletedMessage(1, 'Welcome')).toBe('Deleted “Welcome”.');
    expect(deletedMessage(3)).toBe('Deleted 3 cues.');
    expect(deletedMessage(1, '')).toBe('Deleted 1 cue.');
  });

  it('and adding reports too, so one click per row is visibly one cue', () => {
    // The add path's measured friction is the toggle and the search, not the
    // commit — but a commit with no acknowledgement is why an operator toggles
    // back to check. This is that acknowledgement.
    expect(addedMessage('Romans 8:28')).toBe('Added “Romans 8:28”.');
    expect(addedMessage('')).toBe('Added 1 cue.');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4 · RULE 41 — NEVER A NATIVE DIALOG, ON THE FILE THAT JUST GREW TWO MORE
//     DESTRUCTIVE CONTROLS
// ─────────────────────────────────────────────────────────────────────────────
describe('rule 41 on the Planner', () => {
  it('there is no native confirm, alert or prompt anywhere in the view', () => {
    // `confirm()` returns false without ever showing a dialog in the Tauri
    // webview, so a two-step delete guarded by one deletes NOTHING and reports
    // success. `hardrules.test.js` sweeps the tree; this is here because this
    // change is the reason the temptation exists on this file.
    const code = codeOnly(PLANNER);
    for (const fn of ['confirm', 'alert', 'prompt']) {
      expect(new RegExp(`(^|[^.\\w])${fn}\\s*\\(`).test(code), `native ${fn}() in the Planner`).toBe(false);
    }
  });

  it('both destructive controls are two-step, and the arm times out', () => {
    // The in-app arm/confirm `TemplateGallery` and the plan rail already use.
    // A timeout matters: an arm left standing is a destructive control one stray
    // press away from firing, minutes later, about a cue nobody is looking at.
    expect(PLANNER).toMatch(/cueDelArm/);
    expect(PLANNER).toMatch(/setTimeout\(\(\) => \(cueDelArm = null\), 3000\)/);
  });

  it('the row delete is a real, named control — not a bare glyph', () => {
    // A ✕ with no accessible name is a destructive control a screen reader
    // announces as "button". `qa-inventory` counts unnamed controls, and this is
    // the assertion that keeps that count at zero here.
    expect(PLANNER).toMatch(/class="sp-del[^"]*"/);
    expect(PLANNER).toMatch(/aria-label=\{deleteLabel\(/);
  });

  it('the bulk delete is only reachable when something is picked', () => {
    // Otherwise it is a destructive control that is always live and always means
    // something different depending on state nobody can see.
    expect(PLANNER).toMatch(/picked\.length > 1\}/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5 · THE DUPLICATE THAT SAID "IN PLACE" AND MEANT "AT THE END"
// ─────────────────────────────────────────────────────────────────────────────
describe('a duplicated cue lands beside the one it was copied from', () => {
  it('the copy moves out of the tail and in after its original', () => {
    // `add_plan_item` appends and there is no insert command, so the fix is the
    // reorder that already exists.
    expect(orderWithDuplicateInPlace([1, 2, 3, 4, 99], 2, 99)).toEqual([1, 2, 99, 3, 4]);
  });

  it('duplicating the last cue is a no-op on the order, correctly', () => {
    expect(orderWithDuplicateInPlace([1, 2, 3, 99], 3, 99)).toEqual([1, 2, 3, 99]);
  });

  it('duplicating the first cue puts the copy second', () => {
    expect(orderWithDuplicateInPlace([1, 2, 3, 99], 1, 99)).toEqual([1, 99, 2, 3]);
  });

  it('a missing id leaves the order alone rather than guessing', () => {
    // A reorder built from a guess is worse than no reorder: it persists.
    expect(orderWithDuplicateInPlace([1, 2, 3], 2, 99)).toEqual([1, 2, 3]);
    expect(orderWithDuplicateInPlace([1, 2, 99], 77, 99)).toEqual([1, 2, 99]);
    expect(orderWithDuplicateInPlace([1, 2, 99], 99, 99)).toEqual([1, 2, 99]);
    expect(orderWithDuplicateInPlace(null, 1, 2)).toEqual([]);
  });

  it('and the copy inherits its original’s SECTION, by construction', () => {
    // The part worth noticing. `add_plan_item` takes no `section_title`, so a
    // duplicate has none — and `sectionsOf` reads an empty title as "still in the
    // section above". Landing it after its original therefore puts it in its
    // original's section; landing it at the end put it in whatever section the
    // plan happened to finish in, which is the shipped bug's second half.
    const cues = [
      { id: 1, section_title: 'Gathering' },
      { id: 2, section_title: '' },
      { id: 3, section_title: 'Sending' },
    ];
    const copy = { id: 99, section_title: '' };
    const order = orderWithDuplicateInPlace([1, 2, 3, 99], 2, 99);
    const byId = new Map([...cues, copy].map((c) => [c.id, c]));
    const placed = sectionsOf(order.map((id) => byId.get(id)));
    expect(placed.map((s) => s.title)).toEqual(['Gathering', 'Sending']);
    expect(placed[0].items.map((i) => i.id)).toEqual([1, 2, 99]);

    // …whereas appending it — the shipped behaviour — files the copy under
    // "Sending", a section the operator never put it in.
    const appended = sectionsOf([1, 2, 3, 99].map((id) => byId.get(id)));
    expect(appended[1].items.map((i) => i.id)).toEqual([3, 99]);
  });

  it('the view calls it, and no longer claims something it does not do', () => {
    expect(PLANNER).toMatch(/orderWithDuplicateInPlace\(/);
    // The comment that was false. It may not come back.
    const claim = /Copy a cue in place — the quickest way to a second cue of the same shape\./;
    expect(claim.test(PLANNER) && !/orderWithDuplicateInPlace/.test(PLANNER)).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 6 · DRIVEN — THE ROW CONTROL, THE MODIFIERS, AND THE TWO PRESSES
// ─────────────────────────────────────────────────────────────────────────────
//
// Everything above is the algebra and a source scan. This mounts the Planner
// against a mock bridge and presses the buttons, because the claim "deleting is
// simple and direct" is about a control an operator reaches for, not about a
// function's return value — and the one guarantee that matters most here (a
// FIRST press must delete nothing) can only be checked by pressing it.
const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-20', cue_count: 5 };

const planCue = (id) => ({
  id,
  plan_id: 1,
  position: id,
  cue_type: 'scripture',
  label: `Cue ${id}`,
  payload_json: JSON.stringify({ reference: `Romans 8:${id}`, text: 'words' }),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  channels_json: null,
});

let cues = [];
let removed = [];
let host;
let app;

beforeEach(() => {
  cues = [1, 2, 3, 4, 5].map(planCue);
  removed = [];
  invoke.mockReset();
  invoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(cues);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'remove_plan_item':
        removed.push(args.id);
        cues = cues.filter((c) => c.id !== args.id);
        return Promise.resolve(null);
      case 'get_rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      default:
        return Promise.resolve([]);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true }));
  clearSession();
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  clearSession();
  document.body.innerHTML = '';
});

async function openPlan() {
  const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
  app = new ServicePlanner({ target: host });
  await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
  host.querySelector('.sp-railcard').click();
  await until(() => host.querySelectorAll('.sp-row').length === 5, 'the running order');
  // AND LET THE DESK FINISH OPENING. `onMount` opens the most recent plan by
  // itself, so a click on the rail in the same frame produces two `open()` calls
  // whose awaits interleave — and the second one's `selId = items[0].id` then
  // lands after whatever the test did in between. That is a harness race, not a
  // product defect (nobody clicks a rail card before the desk has drawn it), but
  // a test that does not wait it out asserts against a selection somebody else
  // is still writing.
  await until(() => host.querySelector('.sp-row.sel'), 'the first cue selected');
  await settle(80);
}

const rows = () => [...host.querySelectorAll('.sp-row')];
const click = (el, init = {}) => el.dispatchEvent(new MouseEvent('click', { bubbles: true, ...init }));

describe('deleting, driven', () => {
  itMounted('every row carries its own delete, named for its cue', async () => {
    await openPlan();
    const dels = [...host.querySelectorAll('.sp-del')];
    expect(dels).toHaveLength(5);
    expect(dels[2].getAttribute('aria-label')).toBe('Delete “Cue 3”');
  });

  itMounted('the FIRST press deletes nothing — it arms, and says so', async () => {
    // Rule 41's whole point. A two-step guarded by a native `confirm()` deletes
    // nothing and reports SUCCESS; a two-step that deletes on the first press is
    // the opposite failure and just as bad on a Tuesday evening.
    await openPlan();
    click(host.querySelectorAll('.sp-del')[2]);
    await tick();
    expect(removed).toEqual([]);
    expect(host.querySelectorAll('.sp-row')).toHaveLength(5);
    const armed = host.querySelector('.sp-del.arm');
    expect(armed).not.toBeNull();
    expect(armed.getAttribute('aria-label')).toBe('Delete “Cue 3” — click again');
  });

  itMounted('the SECOND press takes exactly that cue, and nothing else', async () => {
    await openPlan();
    const del = host.querySelectorAll('.sp-del')[2];
    click(del);
    await tick();
    click(host.querySelector('.sp-del.arm'));
    await until(() => removed.length, 'the delete');
    expect(removed).toEqual([3]);
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the reload');
  });

  itMounted('…and the inspector lands on the NEXT cue, not back at the top', async () => {
    // The measured annoyance: `items[0]` throws an operator to cue 1 every time.
    await openPlan();
    click(host.querySelectorAll('.sp-del')[2]);
    await tick();
    click(host.querySelector('.sp-del.arm'));
    await until(() => host.querySelectorAll('.sp-row').length === 4, 'the reload');
    const selected = host.querySelector('.sp-row.sel .sp-cuetitle');
    expect(selected.textContent.trim()).toBe('Cue 4');
  });

  itMounted('arming one row and clicking ANOTHER stands the first one down', async () => {
    // An arm left pointing at a cue nobody is looking at is a destructive control
    // one stray press from firing.
    await openPlan();
    click(host.querySelectorAll('.sp-del')[0]);
    await tick();
    expect(host.querySelector('.sp-del.arm')).not.toBeNull();
    click(rows()[4]);
    await tick();
    expect(host.querySelector('.sp-del.arm')).toBeNull();
    expect(removed).toEqual([]);
  });
});

describe('multi-select, driven', () => {
  itMounted('a plain click picks one and shows no bulk control', async () => {
    // The case a hurried operator is always in. Nothing may have changed.
    await openPlan();
    click(rows()[1]);
    await tick();
    expect(host.querySelector('.sp-picked')).toBeNull();
    expect(host.querySelectorAll('.sp-row.picked')).toHaveLength(1);
  });

  itMounted('SHIFT-click takes the run between, and the bar counts it', async () => {
    await openPlan();
    click(rows()[1]);
    await tick();
    click(rows()[3], { shiftKey: true });
    await tick();
    const marked = [...host.querySelectorAll('.sp-row.picked .sp-cuetitle')].map((e) => e.textContent.trim());
    expect(marked).toEqual(['Cue 2', 'Cue 3', 'Cue 4']);
    expect(host.querySelector('.sp-pickedn').textContent.trim()).toBe('3 cues selected');
  });

  itMounted('CMD-click picks scattered cues, and the bar counts those too', async () => {
    await openPlan();
    click(rows()[0]);
    await tick();
    click(rows()[2], { metaKey: true });
    click(rows()[4], { ctrlKey: true });
    await tick();
    expect(host.querySelectorAll('.sp-row.picked')).toHaveLength(3);
    expect(host.querySelector('.sp-pickedn').textContent.trim()).toBe('3 cues selected');
  });

  itMounted('the bulk delete is two-step as well, and takes the whole run at once', async () => {
    await openPlan();
    click(rows()[1]);
    await tick();
    click(rows()[3], { shiftKey: true });
    await tick();
    const bulk = host.querySelector('.sp-picked .sp-raildel');
    expect(bulk.textContent.trim()).toBe('Delete 3 cues');

    bulk.click();
    await tick();
    // First press: nothing gone, and the control says which press comes next.
    expect(removed).toEqual([]);
    expect(host.querySelector('.sp-picked .sp-raildel').textContent.trim()).toBe(
      'Delete 3 cues — click again',
    );

    host.querySelector('.sp-picked .sp-raildel').click();
    await until(() => removed.length === 3, 'the bulk delete');
    expect(removed).toEqual([2, 3, 4]);
    await until(() => host.querySelectorAll('.sp-row').length === 2, 'the reload');
    // The selection is put down, so the bar goes with it.
    expect(host.querySelector('.sp-picked')).toBeNull();
  });

  itMounted('the duplicate lands beside its original, not at the end of the plan', async () => {
    // The defect, driven. `add_plan_item` appends, so the only way this can be
    // right is the reorder — and the only way to see that it happened is to watch
    // what `reorder_plan` was handed.
    let nextId = 90;
    const base = invoke.getMockImplementation();
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'add_plan_item') {
        nextId += 1;
        cues = [...cues, { ...planCue(nextId), label: args.label }];
        return Promise.resolve(nextId);
      }
      if (cmd === 'reorder_plan') {
        const by = new Map(cues.map((c) => [c.id, c]));
        cues = args.ids.map((id) => by.get(id));
        return Promise.resolve(null);
      }
      return base(cmd, args);
    });

    await openPlan();
    click(rows()[1]); // Cue 2
    await tick();
    const dup = [...host.querySelectorAll('.sp-actions .r-btn')].find((b) =>
      b.textContent.includes('Duplicate'),
    );
    dup.click();
    await until(() => host.querySelectorAll('.sp-row').length === 6, 'the copy');
    await settle(80);

    const order = [...host.querySelectorAll('.sp-row .sp-cuetitle')].map((e) => e.textContent.trim());
    expect(order).toEqual(['Cue 1', 'Cue 2', 'Cue 2', 'Cue 3', 'Cue 4', 'Cue 5']);
    // …and the copy is what the inspector is now about, because it is the thing
    // the operator just made and is about to edit.
    expect(host.querySelectorAll('.sp-row')[2].classList.contains('sel')).toBe(true);
  });

  itMounted('Clear selection puts it down without touching the plan', async () => {
    await openPlan();
    click(rows()[0]);
    await tick();
    click(rows()[3], { shiftKey: true });
    await tick();
    const clear = [...host.querySelectorAll('.sp-picked .r-btn')].find((b) =>
      b.textContent.includes('Clear selection'),
    );
    clear.click();
    await tick();
    expect(removed).toEqual([]);
    expect(host.querySelectorAll('.sp-row')).toHaveLength(5);
    expect(host.querySelector('.sp-picked')).toBeNull();
  });
});
