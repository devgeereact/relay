// WHAT ONE PRESS ON A CARD DOES, AND WHERE.
//
// `docs/REBRAND.md` §2 and §10 put the line here: a single click sends to
// Program on LIVE, the run surface, and CUES on the LIBRARY, the build surface.
// `VerseDeck` is shared by both sides of that line — five Library panes render
// it, and Live reaches them — so the behaviour is a PROP, not a rewrite, and its
// default is what every caller did before the prop existed.
//
// This file exists because the two claims below are the kind that are only ever
// wrong in front of a congregation, and because a prop with a default is exactly
// the shape that rots silently: a pane that forgets to pass `press="select"`
// looks identical and fires.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Test the bug, not the fix (CLAUDE.md). Each assertion below was watched to go
// RED with the change reverted:
//
//   · "a Library press reaches no fire path" — reverted by deleting
//     `press="select"` from `Browse.svelte`: `onFire` is called, `onSelect` is
//     not, and the test fails on both halves.
//   · "the default press still fires" — reverted by changing `VerseDeck`'s
//     `export let press = 'fire'` to `'select'`: the default-props case fails
//     while the Library cases stay green, which is the failure that says the
//     blast radius went the wrong way.
//   · "every Library pane opts in" — reverted by removing the prop from any one
//     of the five panes: the test names that file.

import { describe, it, expect, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const VERSE = {
  reference: 'John 3:16',
  label: 'John 3:16',
  text: 'For God so loved the world',
  slideNo: 1,
};

function mount(Component, props) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = new Component({ target: host, props });
  return { host, app, done: () => (app.$destroy(), host.remove()) };
}

const card = (host) => host.querySelector('.vd-shot');

describe('§10 · a Library press CUES; it does not go to air', () => {
  it('one press on a card selects it and reaches no fire path', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const onSelect = vi.fn();
    const { host, done } = mount(VerseDeck, {
      items: [VERSE],
      layout: 'grid',
      press: 'select',
      onFire,
      onSelect,
    });
    await tick();

    card(host).click();
    await tick();

    // The whole claim, in two lines: the inspector got the item, and nothing
    // that can reach a screen was touched.
    expect(onSelect).toHaveBeenCalledOnce();
    expect(onSelect.mock.calls[0][0].reference).toBe('John 3:16');
    expect(onFire).not.toHaveBeenCalled();
    done();
  });

  it('a DOUBLE press opens the item — and still fires nothing', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const onOpen = vi.fn();
    const { host, done } = mount(VerseDeck, {
      items: [VERSE],
      layout: 'grid',
      press: 'select',
      onFire,
      onOpen,
    });
    await tick();

    card(host).dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await tick();

    expect(onOpen).toHaveBeenCalledOnce();
    expect(onFire).not.toHaveBeenCalled();
    done();
  });

  it('ENTER on a list row obeys the same rule as the press beside it', async () => {
    // A keyboard operator who has learned that clicking a Library card selects it
    // must not find that Enter on the same card puts it in front of people.
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const onSelect = vi.fn();
    const { host, done } = mount(VerseDeck, {
      items: [VERSE],
      layout: 'list',
      press: 'select',
      onFire,
      onSelect,
    });
    await tick();

    host
      .querySelector('.vd-row')
      .dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await tick();

    expect(onSelect).toHaveBeenCalledOnce();
    expect(onFire).not.toHaveBeenCalled();
    done();
  });

  it('the hover legend says what the press will do, on both settings', async () => {
    // "Go live →" over a press that selects is rule 35 in miniature: a line that
    // reads the same whether or not the thing behind it is what you think.
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const sel = mount(VerseDeck, { items: [VERSE], layout: 'grid', press: 'select' });
    await tick();
    expect(sel.host.querySelector('.vd-go').textContent).toMatch(/open/i);
    expect(sel.host.querySelector('.vd-go').textContent).not.toMatch(/go live/i);
    sel.done();

    const fire = mount(VerseDeck, { items: [VERSE], layout: 'grid' });
    await tick();
    expect(fire.host.querySelector('.vd-go').textContent).toMatch(/go live/i);
    fire.done();
  });
});

describe('§10 · the DEFAULT is unchanged, so no other surface moved', () => {
  it('a deck given no `press` still fires on one click', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const onSelect = vi.fn();
    const { host, done } = mount(VerseDeck, { items: [VERSE], layout: 'grid', onFire, onSelect });
    await tick();

    card(host).click();
    await tick();

    // This is the blast-radius assertion. If it ever fails, the Library's change
    // has leaked onto every other caller of this deck.
    expect(onFire).toHaveBeenCalledOnce();
    expect(onSelect).not.toHaveBeenCalled();
    done();
  });

  it('a LIST row given no `press` still fires on one click', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const onFire = vi.fn();
    const { host, done } = mount(VerseDeck, { items: [VERSE], layout: 'list', onFire });
    await tick();

    host.querySelector('.vd-row').click();
    await tick();

    expect(onFire).toHaveBeenCalledOnce();
    done();
  });
});

describe('the bulk tick box is deliberate, not a ghost', () => {
  // DECISIONS §76 removed three selection tick boxes that led nowhere. This one
  // does not: it feeds "Queue N selected" in the head of both scripture panes.
  // But a permanent grey square in the corner of every card is indistinguishable
  // from one of those at a glance — and now that a single press SELECTS into the
  // inspector, a second differently-shaped "select" on the same card is two
  // meanings for one word. So it is quiet until it is wanted.
  const read = () =>
    readFileSync(join(process.cwd(), 'src/lib/views/library/VerseDeck.svelte'), 'utf8');

  it('stays in the DOM, so it keeps its tab order and its accessible name', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, { items: [VERSE], layout: 'grid', press: 'select' });
    await tick();
    const box = host.querySelector('.vd-check input');
    expect(box).toBeTruthy();
    expect(box.getAttribute('aria-label')).toMatch(/John 3:16/);
    // `opacity`, never `display:none` — a control removed from the a11y tree to
    // tidy a grid is a control a keyboard operator cannot reach.
    expect(read()).not.toMatch(/\.vd-check\s*\{[^}]*display:\s*none/);
    done();
  });

  it('is revealed by hover, by focus, and by a selection already under way', () => {
    const css = read();
    expect(css).toMatch(/\.vd-check\s*\{[^}]*opacity:\s*0/);
    for (const trigger of [
      /\.vd-card:hover \.vd-check/,
      /\.vd-check:focus-within/,
      /\.vd-check\.armed/,
      /\.vd-check:has\(input:checked\)/,
    ]) {
      expect(css, `no rule reveals it for ${trigger}`).toMatch(trigger);
    }
  });

  it('and once ONE is ticked, every card shows its box — a mode looks like one', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, app, done } = mount(VerseDeck, {
      items: [VERSE, { ...VERSE, reference: 'John 3:17', label: 'John 3:17', slideNo: 2 }],
      layout: 'grid',
      press: 'select',
      checked: new Set(),
    });
    await tick();
    expect([...host.querySelectorAll('.vd-check.armed')]).toHaveLength(0);

    app.$set({ checked: new Set(['John 3:16']) });
    await tick();
    // BOTH, not just the ticked one: counting a selection means seeing the boxes
    // that are not filled as well as the ones that are.
    expect([...host.querySelectorAll('.vd-check.armed')]).toHaveLength(2);
    done();
  });

  it('the panes that show it have a bulk action behind it, and the rest turn it off', () => {
    const owns = (p) =>
      readFileSync(join(process.cwd(), p), 'utf8');
    // Scripture and Browse offer "Queue N selected".
    for (const p of [
      'src/lib/views/library/Browse.svelte',
      'src/lib/views/library/Scripture.svelte',
    ]) {
      expect(owns(p), `${p} shows a tick box with nothing behind it`).toMatch(/queueChecked/);
    }
    // The other three pass `select: false` — DECISIONS §76's three dead boxes.
    for (const p of [
      'src/lib/views/library/LyricsPane.svelte',
      'src/lib/views/library/MediaLibrary.svelte',
      'src/lib/views/library/Announcements.svelte',
    ]) {
      expect(owns(p), `${p} shows a tick box with no bulk action`).toMatch(/select: false/);
    }
  });
});

describe('§10 · every Library pane opts in, and the deck says which', () => {
  // A SOURCE assertion, because the failure it guards against is a sixth pane
  // added next year that renders this deck and forgets the prop. That pane would
  // look identical and fire on a browse click, and no mounted test would see it
  // unless somebody remembered to write one.
  const read = (p) => readFileSync(join(process.cwd(), p), 'utf8');
  const PANES = [
    'src/lib/views/library/Browse.svelte',
    'src/lib/views/library/Scripture.svelte',
    'src/lib/views/library/LyricsPane.svelte',
    'src/lib/views/library/MediaLibrary.svelte',
    'src/lib/views/library/Announcements.svelte',
  ];

  it('the scanner can still see the thing it is looking for', () => {
    // A scanner that quietly narrows passes everything (CLAUDE.md, ipc.test.js,
    // twice). Prove the match is real before trusting five absences of a failure.
    expect(/press="select"/.test('<VerseDeck press="select" />')).toBe(true);
    expect(/press="select"/.test('<VerseDeck />')).toBe(false);
  });

  it('all five Library panes pass press="select"', () => {
    const missing = PANES.filter((p) => !/press="select"/.test(read(p)));
    expect(missing, 'a Library pane whose cards still fire on one click').toEqual([]);
  });

  it('and every one of them renders the deck, so the list is not stale', () => {
    const notRendering = PANES.filter((p) => !/<VerseDeck\b/.test(read(p)));
    expect(notRendering, 'a pane in this list that no longer renders VerseDeck').toEqual([]);
  });
});
