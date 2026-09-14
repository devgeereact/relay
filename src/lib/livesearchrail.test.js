// THE SEARCH RAIL — one click does the whole job, and every hit says why.
//
// `docs/REBRAND.md` §9 asks two things of the Live rail that nothing held until
// this file:
//
//   · **One click does the whole job** — the verse goes to the programme and its
//     chapter loads into the grid. Double click opens the chapter and puts
//     nothing on a screen, through the SAME `pressArbiter` the slide grid uses,
//     so a double can never also fire (that beat is tested in `slidegrid.test.js`).
//   · **Each hit says why it matched** — DECISIONS §72 recorded this as not
//     built precisely because it changes the shape three surfaces read.
//
// And the rule that bounds all of it (CLAUDE.md rule 10): nothing here may
// auto-fire. A press is an operator action, start to finish, and a MOUNT — the
// search arriving, the list redrawing — must reach no screen at all. The Rust
// half of that boundary is `search.rs` and `e2e::r9_*`.
//
// COLOUR LAW (rule 18, DECISIONS §21): a guess is cyan. Never amber, which means
// ON AIR, and never amethyst, which means rehearsal.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { tick } from 'svelte';

const read = (p) => readFileSync(new URL(p, import.meta.url), 'utf8');

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const LiveRail = (await import('./LiveRail.svelte')).default;
const { PRESS_MS } = await import('./slidegrid.js');

// What the backend now answers with: a verse row PLUS why it is in the list.
const HIT_TYPED = {
  id: 1,
  book: 'Psalms',
  chapter: 23,
  verse: 1,
  reference: 'Psalms 23:1',
  text: 'The LORD is my shepherd; I shall not want.',
  method: 'reference',
  guess: false,
  why: 'the reference you typed — “ps 23 1”',
  matched: [],
};
const HIT_GUESSED = {
  id: 2,
  book: 'Romans',
  chapter: 8,
  verse: 1,
  reference: 'Romans 8:1',
  text: 'There is therefore now no condemnation…',
  method: 'paraphrase',
  guess: true,
  why: 'close in meaning, not in words — a guess',
  matched: [],
};

let host;
let app;
let staged;
let sent;

function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new LiveRail({
    target: host,
    props: {
      onChapter: (book, chapter) => staged.push([book, chapter]),
      onVerse: (book, chapter, verse, reference) => sent.push(reference ?? `${book} ${chapter}:${verse}`),
      ...props,
    },
  });
  return host;
}

/**
 * Let the debounce, the dynamic bridge import and Svelte's scheduler all finish.
 *
 * `capture.js` reaches the backend through a dynamic `import()`, which resolves a
 * turn later than the scheduler; the rail also debounces the query by 220ms. A
 * test that only ticks sees an empty list and reads like the search is dead.
 */
async function settle(ms = 300) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const hitRows = () => [...host.querySelectorAll('.lr-hit')];

/** Type a query and wait for the results to land. */
async function search(text) {
  const box = host.querySelector('.lr-q');
  box.value = text;
  box.dispatchEvent(new Event('input'));
  await settle();
}

beforeEach(() => {
  invoke.mockReset();
  staged = [];
  sent = [];
  // `onMount` asks for the book list; the search asks for hits.
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'search_scripture') return [HIT_TYPED, HIT_GUESSED];
    return [];
  });
  vi.useRealTimers();
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

describe('the Live search rail', () => {
  it('says WHY each hit matched, in the backend’s own words', async () => {
    mount();
    await search('ps 23 1');

    const rows = hitRows();
    expect(rows.length).toBe(2);
    // Not a sentence this component composed — `search.rs` owns it, so the
    // Library, the Planner and the preacher's remote cannot disagree with it.
    expect(rows[0].textContent).toContain('the reference you typed');
    expect(rows[1].textContent).toContain('a guess');
  });

  it('marks a guess cyan and a typed reference not', async () => {
    mount();
    await search('ps 23 1');

    const whys = [...host.querySelectorAll('.lr-why')];
    expect(whys.length).toBe(2);
    expect(whys[0].classList.contains('guess')).toBe(false);
    expect(whys[1].classList.contains('guess')).toBe(true);
  });

  it('never quotes a percentage — a cosine is not a probability', async () => {
    mount();
    await search('no condemnation');
    for (const w of host.querySelectorAll('.lr-why')) {
      expect(w.textContent).not.toContain('%');
    }
  });

  it('sends the verse to the programme on ONE click, and opens its chapter', async () => {
    mount();
    await search('ps 23 1');

    hitRows()[0].dispatchEvent(new MouseEvent('click', { bubbles: true }));
    // The chapter is staged straight away; the fire waits out the 190ms beat.
    await new Promise((r) => setTimeout(r, PRESS_MS + 40));
    await tick();

    expect(sent).toEqual(['Psalms 23:1']);
  });

  it('a DOUBLE click opens the chapter and sends nothing', async () => {
    mount();
    await search('ps 23 1');

    const row = hitRows()[0];
    row.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    row.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    await new Promise((r) => setTimeout(r, PRESS_MS + 40));
    await tick();

    expect(staged).toEqual([['Psalms', 23]]);
    expect(sent).toEqual([]);
  });

  it('searching reaches no screen until an operator presses something', async () => {
    // RULE 10, on this surface. Mounting, typing and the list redrawing are not
    // operator choices, and none of them may put anything anywhere.
    mount();
    await search('ps 23 1');
    await search('the lord is my shepherd');

    expect(sent).toEqual([]);
    expect(staged).toEqual([]);
    // …and the only backend call a search makes is the search itself.
    const commands = new Set(invoke.mock.calls.map((c) => c[0]));
    expect(commands.has('manual_fire')).toBe(false);
    expect(commands.has('fire_content')).toBe(false);
  });

  it('browsing a chapter still only stages — there is no one verse to send', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
      return [];
    });
    mount();
    await settle(0);

    host.querySelector('.lr-row').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await tick();
    host.querySelector('.lr-chip').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await tick();

    expect(staged).toEqual([['Psalms', 1]]);
    expect(sent).toEqual([]);
  });

  it('a backend with no “why” renders the hit and invents no reason', async () => {
    // The field is additive (`#[serde(flatten)]` over the row), so an older
    // backend is a missing sentence, not a broken rail. It must say nothing
    // rather than make something up.
    invoke.mockImplementation(async (cmd) =>
      cmd === 'search_scripture'
        ? [{ id: 9, book: 'John', chapter: 3, verse: 16, reference: 'John 3:16', text: 'For God…' }]
        : [],
    );
    mount();
    await search('john 3 16');

    expect(hitRows().length).toBe(1);
    expect(host.querySelector('.lr-why')).toBe(null);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE OTHER HALF OF §9, WHICH ONLY `Live.svelte` CAN KEEP.
//
// The rail hands out a verse; `Live` is what stages the chapter and fires. It
// cannot be mounted meaningfully in this suite — `r2livepath.test.js` (R2-F)
// states why and this file does not pretend otherwise — so these read the source.
//
// A SOURCE TEST IS A WEAKER TEST and is labelled as one. It cannot prove the
// press works; it can prove the two things that would silently break the
// guarantee if someone edited them: a second fire path, and a fire that leaves
// the grid on the previous chapter when it fails.
describe('Live wires the rail without inventing a fire path (source)', () => {
  const live = read('./views/Live.svelte');
  const body = live.slice(live.indexOf('async function fireSearchHit'));
  const fn = body.slice(0, body.indexOf('\n  }\n') + 4);

  it('takes a search hit through the same manualFire the grid uses', () => {
    expect(live).toContain('onVerse={fireSearchHit}');
    expect(fn).toContain('await manualFire(');
    // Never `fireContent`, never a hand-built OutputContent — CLAUDE.md's
    // "never build an OutputContent by hand", and rule 36's one choke point.
    expect(fn).not.toContain('fireContent');
    expect(fn).not.toContain('invoke(');
  });

  it('stages the chapter BEFORE the fire, so a failure still leaves the passage up', () => {
    expect(fn.indexOf('stageChapter(')).toBeGreaterThan(-1);
    expect(fn.indexOf('stageChapter(')).toBeLessThan(fn.indexOf('await manualFire('));
  });

  it('reports its own outcome rather than assuming one (rule 15)', () => {
    // A `catch` that humanises the error, not a bare one: a fire that failed
    // must never leave "now live" on the screen over a wall that did not change.
    expect(fn).toContain('catch');
    expect(fn).toContain('humanError(e)');
  });
});
