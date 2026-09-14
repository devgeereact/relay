// THE LIBRARY INSPECTOR — what is true of the thing in hand (REBRAND §10).
//
// The right column used to be a second run surface. It is now the pane that
// answers "what did I just select, and what are the two things I can do with
// it". Three of its claims are the kind that are only wrong in front of people,
// and each is here:
//
//   1. CUEING IS NOT A TAKE. `Cue in Live` stages the item on the Up Next queue
//      and reaches no output. If it ever fires, the whole point of moving the
//      Library's single click off the fire path is undone one button later.
//   2. A LYRIC PROJECTS THE LYRIC. The section label is the operator's
//      bookkeeping and must not reach the preview, which is drawn by the same
//      `TemplateRender` that paints the wall (DECISIONS §73). Scripture is the
//      opposite: the reference IS content.
//   3. "IN THIS PLAN" HAS THREE ANSWERS, not two. "not yet" over a plan nobody
//      opened is a claim from an absence, which is rule 35.
//
// Each assertion below was watched to fail with its own defect reintroduced —
// see the note above each one.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const Inspector = (await import('./views/library/Inspector.svelte')).default;
const { session, setSession } = await import('./session.js');

const VERSE = {
  kind: 'scripture',
  title: 'John 3:16',
  titleLabel: 'Reference',
  translation: 'KJV',
  words: 'For God so loved the world',
  slide: { reference: 'John 3:16', text: 'For God so loved the world', translation: 'KJV' },
  reference: 'John 3:16',
  plan: { cueType: 'scripture', label: 'John 3:16', payload: {} },
};

const SECTION = {
  kind: 'song',
  title: 'Chorus',
  titleLabel: 'Name',
  hotkey: 'c',
  words: 'Great is thy faithfulness',
  // The pane that produces this sets `reference: null` for a song slide, because
  // a congregation is not reading the section name.
  slide: { reference: null, text: 'Great is thy faithfulness' },
  reference: 'Great Is Thy Faithfulness · Chorus',
  songId: 7,
  plan: { cueType: 'song', label: 'Great Is Thy Faithfulness', payload: {} },
};

let host;
let app;

function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Inspector({ target: host, props: { item: null, ...props } });
  return host;
}

const btn = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.includes(text));
/** The value beside a named row. */
const value = (name) => {
  const row = [...host.querySelectorAll('.rw-nv')].find((r) =>
    r.querySelector('.rw-nvk')?.textContent.trim().startsWith(name),
  );
  return row?.querySelector('.rw-nvv')?.textContent.trim();
};

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  setSession({ planId: null });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

describe('nothing selected is a state with its own words', () => {
  it('says so, rather than rendering an empty frame', () => {
    mount();
    expect(host.textContent).toMatch(/Select an item to preview it/i);
    expect(btn('Cue in Live')).toBeUndefined();
  });
});

describe('cueing is not a take', () => {
  // REVERTED TO CHECK: replacing `onQueueChange([...])` in `cue()` with a call to
  // `manualFire` makes this fail on the `invoke` assertion — which is the whole
  // reason the assertion is on `invoke` and not only on the queue.
  it('Cue in Live stages the item and reaches no backend at all', async () => {
    let queue = [];
    mount({ item: VERSE, queue, onQueueChange: (q) => (queue = q) });
    await settle();
    const before = invoke.mock.calls.length;

    btn('Cue in Live').click();
    await settle();

    expect(queue.map((q) => q.reference)).toEqual(['John 3:16']);
    // Not one further call. Cueing is a decision about what is NEXT, and the
    // congregation is looking at whatever they were looking at before.
    expect(invoke.mock.calls.length).toBe(before);
  });

  it('and says, in words, that the item is not on a screen', async () => {
    mount({ item: VERSE, queue: [], onQueueChange: () => {} });
    await settle();
    btn('Cue in Live').click();
    await tick();
    expect(host.querySelector('.li-msg').textContent).toMatch(/not on a screen/i);
  });

  it('an already-cued item offers the way back out', async () => {
    mount({ item: VERSE, queue: [{ reference: 'John 3:16' }], onQueueChange: () => {} });
    await settle();
    expect(btn('Remove from Up Next')).toBeTruthy();
    expect(btn('Cue in Live')).toBeUndefined();
  });
});

describe('the preview shows what the ROOM will see, not what the operator calls it', () => {
  // REVERTED TO CHECK: passing `item.title` as the preview's `reference` for a
  // song — which is what five hand-rolled copies of this markup did before
  // `pipeline::Fire` — puts "Chorus" in the rendered frame and fails this.
  it('a song section does not put its label in the frame', async () => {
    mount({ item: SECTION });
    await settle();
    const frame = host.querySelector('.li-frame');
    expect(frame.textContent).toMatch(/Great is thy faithfulness/);
    expect(frame.textContent, 'the section label is operator-only').not.toMatch(/Chorus/);
  });

  it('but the operator still sees the label, and the key that fires it', async () => {
    mount({ item: SECTION });
    await settle();
    expect(value('Name')).toBe('Chorus');
    expect(value('Fires with')).toBe('C');
  });

  it('a verse DOES carry its reference — scripture is the opposite case', async () => {
    mount({ item: VERSE });
    await settle();
    expect(host.querySelector('.li-frame').textContent).toMatch(/John 3:16/);
    expect(value('Translation')).toBe('KJV');
  });
});

describe('"in this plan" has three answers', () => {
  it('with no plan open it says so, rather than saying "not yet"', async () => {
    // The defect this exists for: an absence read as a negative. An operator told
    // "not yet" reaches for Add to plan; there is no plan for it to go into.
    mount({ item: VERSE });
    await settle();
    expect(value('In this plan')).toBe('no plan open');
    expect(btn('Add to plan').disabled).toBe(true);
    expect(btn('Add to plan').getAttribute('title')).toMatch(/Planner/i);
  });

  it('with a plan open that does not hold it, "not yet"', async () => {
    setSession({ planId: 3 });
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_plans') return Promise.resolve([{ id: 3, title: 'Sunday' }]);
      if (cmd === 'plan_items') return Promise.resolve([{ id: 1, label: 'Psalms 23:1' }]);
      return Promise.resolve([]);
    });
    mount({ item: VERSE });
    await settle();
    await settle();
    expect(value('In this plan')).toBe('not yet');
    expect(btn('Add to plan').disabled).toBe(false);
  });

  it('with a plan open that already holds it, "yes"', async () => {
    setSession({ planId: 3 });
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_plans') return Promise.resolve([{ id: 3, title: 'Sunday' }]);
      if (cmd === 'plan_items') return Promise.resolve([{ id: 1, label: 'John 3:16' }]);
      return Promise.resolve([]);
    });
    mount({ item: VERSE });
    await settle();
    await settle();
    expect(value('In this plan')).toBe('yes');
  });
});

describe('add to plan goes through the plan, which is the §10 rule', () => {
  it('appends a cue and re-reads the plan rather than assuming it worked', async () => {
    setSession({ planId: 3 });
    const seen = [];
    invoke.mockImplementation((cmd, args) => {
      seen.push(cmd);
      if (cmd === 'list_plans') return Promise.resolve([{ id: 3, title: 'Sunday' }]);
      if (cmd === 'plan_items') return Promise.resolve([]);
      if (cmd === 'add_plan_item') return Promise.resolve({ ...args });
      return Promise.resolve([]);
    });
    mount({ item: VERSE });
    await settle();
    await settle();

    btn('Add to plan').click();
    await settle();
    await settle();

    expect(seen).toContain('add_plan_item');
    // The re-read is the point: "yes" must come from the plan, not from the fact
    // that a button was pressed.
    expect(seen.filter((c) => c === 'plan_items').length).toBeGreaterThan(1);
    expect(host.querySelector('.li-msg').textContent).toMatch(/Sunday/);
  });

  it('a FAILED add says so in the failure colour and claims nothing', async () => {
    setSession({ planId: 3 });
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_plans') return Promise.resolve([{ id: 3, title: 'Sunday' }]);
      if (cmd === 'plan_items') return Promise.resolve([]);
      if (cmd === 'add_plan_item') return Promise.reject('database is locked');
      return Promise.resolve([]);
    });
    mount({ item: VERSE });
    await settle();
    await settle();

    btn('Add to plan').click();
    await settle();
    await settle();

    const err = host.querySelector('.li-err');
    expect(err).toBeTruthy();
    expect(err.getAttribute('role')).toBe('alert');
    expect(host.querySelector('.li-msg')).toBe(null);
  });
});
