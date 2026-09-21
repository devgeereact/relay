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
    host.querySelector('.cp-chip').dispatchEvent(new MouseEvent('click', { bubbles: true }));
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

// ─────────────────────────────────────────────────────────────────────────────
// WAVE 3 · BOTH COLLECTIONS, AND ONE BOX
//
// `docs/REBRAND.md` §2 gives the run surface a rail that offers Bible AND Songs;
// Relay's offered scripture only, so half of what a service is made of could not
// be reached without leaving the surface the service is run from. §9 asks for
// ONE search box, which is why the inspector column's second scripture field —
// the manual box, the floor under the AI — is the `Fire` beside this one rather
// than a panel of its own.
//
// The boundary is unchanged and is what these tests are mostly about: the rail
// STAGES. A song press puts slides in the grid and touches no screen; only the
// explicit `Fire`, on an explicit press, sends anything.
// ─────────────────────────────────────────────────────────────────────────────
describe('the rail offers both collections', () => {
  const SONGS = [
    { id: 7, title: 'Blessed Assurance', author: 'Crosby', section_count: 4 },
    { id: 8, title: 'It Is Well', author: 'Spafford', section_count: 5 },
  ];

  function mountWithSongs(props = {}) {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'search_scripture') return [HIT_TYPED, HIT_GUESSED];
      if (cmd === 'list_songs' || cmd === 'search_songs') return SONGS;
      return [];
    });
    return mount(props);
  }

  const tabs = () => [...host.querySelectorAll('.lr-seg button')];

  it('has a Bible | Songs switch, and Bible is what it opens on', async () => {
    mountWithSongs();
    await settle();
    expect(tabs().map((b) => b.textContent.trim())).toEqual(['Bible', 'Songs']);
    expect(tabs()[0].getAttribute('aria-pressed')).toBe('true');
    expect(tabs()[1].getAttribute('aria-pressed')).toBe('false');
  });

  it('the Songs half lists songs and a press only STAGES — no screen is touched', async () => {
    const opened = [];
    mountWithSongs({ onSong: (id, title) => opened.push([id, title]) });
    await settle();
    tabs()[1].click();
    await settle();

    const rows = [...host.querySelectorAll('.lr-row')];
    expect(rows.map((r) => r.querySelector('.lr-n').textContent)).toEqual([
      'Blessed Assurance',
      'It Is Well',
    ]);

    invoke.mockClear();
    rows[0].click();
    await settle();
    expect(opened).toEqual([[7, 'Blessed Assurance']]);
    // THE WHOLE POINT. Nothing that puts content on a screen was called.
    const called = invoke.mock.calls.map((c) => c[0]);
    expect(called).not.toContain('fire_content');
    expect(called).not.toContain('manual_fire');
    expect(called).not.toContain('fire_media');
  });

  it('switching halves clears the query, so one collection never reports the other miss', async () => {
    mountWithSongs();
    await settle();
    await search('ps 23 1');
    expect(hitRows()).toHaveLength(2);
    tabs()[1].click();
    await settle();
    expect(host.querySelector('.lr-q').value).toBe('');
    expect(hitRows()).toHaveLength(0);
  });

  it('Fire sends the reference exactly as typed — ranges included', async () => {
    const fired = [];
    mountWithSongs({ onReference: (t) => fired.push(t) });
    await settle();
    await search('John 3:16-18');
    const fire = host.querySelector('.lr-fire');
    expect(fire.disabled).toBe(false);
    fire.click();
    // Not `John 3:16`, which is all the corpus search can offer for that query.
    expect(fired).toEqual(['John 3:16-18']);
  });

  // NOT OFFERED MEANS NOT THERE (L2). This test said "not offered" and asserted
  // `disabled`, which is a different claim — and the rail really did draw a grey
  // Fire beside every empty box and every phrase, for the whole life of the
  // surface. The prototype's rail has no such button; a control that spends its
  // life refusing teaches an operator to stop reading the column it sits in.
  it('Fire is not offered for a phrase — a fire nobody could satisfy is not a control', async () => {
    const fired = [];
    mountWithSongs({ onReference: (t) => fired.push(t) });
    await settle();
    await search('seek ye first the kingdom');
    expect(host.querySelector('.lr-fire')).toBeNull();
    const box = host.querySelector('.lr-q');
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(fired).toEqual([]);
  });

  it('nor for an empty box — the rail opens with no Fire in it at all', async () => {
    mountWithSongs();
    await settle();
    expect(host.querySelector('.lr-fire')).toBeNull();
  });

  // …AND THE ENGINE IS THE OTHER QUESTION. A reference the operator HAS typed is
  // exactly when the manual fire must still be visible: it is the floor under the
  // AI (§9), and a floor that disappears when the engine drops reads as a floor
  // that was never built. Greyed, with the reason in `title` — not removed.
  it('a real reference with no engine keeps the button, disabled and explained', async () => {
    mountWithSongs({ disabled: true });
    await settle();
    await search('ps 23 1');
    const fire = host.querySelector('.lr-fire');
    expect(fire).not.toBeNull();
    expect(fire.disabled).toBe(true);
    expect(fire.getAttribute('title')).toMatch(/engine is not attached/);
  });

  it('the Songs half has no Fire at all — a song section has no reference to resolve', async () => {
    mountWithSongs();
    await settle();
    tabs()[1].click();
    await settle();
    expect(host.querySelector('.lr-fire')).toBeNull();
  });

  it('exposes focus(), so the console search shortcut still reaches a box', async () => {
    mountWithSongs();
    await settle();
    expect(typeof app.focus).toBe('function');
    app.focus();
    expect(document.activeElement).toBe(host.querySelector('.lr-q'));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// L5 · THE CHAPTER PICKER
//
// The operator's report was a screenshot: fifty numbered chips, 1 to 50, in
// seven ragged rows, every chip a different width because the numbers are one
// and two digits. Underneath it were four more costs on the same path —
// chapters inline in the book list, chapters opening below the fold, a picker
// drawn for a book with one chapter, and no memory of where you had been.
//
// NOTHING HERE MOVES WHAT A PRESS MEANS. Browsing stages and reaches no screen;
// rule 10's neighbourhood is untouched, and the tests above still hold it.
// ─────────────────────────────────────────────────────────────────────────────
describe('the chapter picker', () => {
  const BOOKS = [
    { book: 'Genesis', chapters: 50 },
    { book: 'Psalms', chapters: 150 },
    // One of the five one-chapter books.
    { book: 'Jude', chapters: 1 },
  ];

  function mountBooks(props = {}) {
    invoke.mockImplementation(async (cmd) => (cmd === 'list_books' ? BOOKS : []));
    return mount(props);
  }

  const bookRow = (name) =>
    [...host.querySelectorAll('.lr-row')].find((r) => r.querySelector('.lr-n')?.textContent === name);
  const chips = () => [...host.querySelectorAll('.cp-chip')];

  it('lays the chapters on a FIXED GRID of equal cells, not a ragged wrap (source)', () => {
    // A SOURCE TEST, labelled as one: jsdom does no layout, so it cannot measure
    // two boxes and compare them. What it CAN hold is the pair of properties
    // that made them ragged — a wrap, and a chip sized by its own label — and it
    // was watched to fail with either of them restored.
    // The grid moved into `ui/ChapterPicker.svelte` (RG-216) so the Library
    // could mount the same picker rather than grow a copy. The rules did not
    // move: every property below is the one that made these ragged.
    const css = read('./ui/ChapterPicker.svelte');
    const block = css.slice(css.indexOf('.cp-chips {'), css.indexOf('.cp-chip {'));
    expect(block).toContain('display: grid');
    expect(block).toContain('repeat(auto-fill, minmax(');
    expect(block).not.toContain('flex-wrap');

    const chip = css.slice(css.indexOf('.cp-chip {'), css.indexOf('.cp-chip:hover'));
    // Every cell filled by its grid track — NOT `min-width`, which is what let
    // `50` draw a wider box than `1`.
    expect(chip).toContain('width: 100%');
    expect(chip).not.toContain('min-width');
    // Figures that do not shift inside cells that are finally the same size.
    expect(chip).toContain('tabular-nums');
  });

  it('the chapter grid says what a press does, where the press happens', async () => {
    mountBooks();
    await settle(0);
    bookRow('Genesis').click();
    await tick();

    const cap = host.querySelector('.cp-cap');
    expect(cap).not.toBeNull();
    // Browsing opens a chapter in the grid. It is NOT the search half's
    // sentence, which is true of a hit and false of a chapter: §9 makes a single
    // press on a search HIT send a verse to the programme. One legend over both
    // meanings would read the same whether or not a congregation is looking at
    // something, which is rule 35 on a caption.
    expect(cap.textContent).toMatch(/no screen changes/i);
    expect(cap.textContent).not.toMatch(/send/i);
  });

  it('every chapter says, on itself, that it reaches no screen', async () => {
    mountBooks();
    await settle(0);
    bookRow('Genesis').click();
    await tick();
    expect(chips()[0].getAttribute('title')).toMatch(/nothing reaches a screen/i);
  });

  it('a ONE-CHAPTER book opens on one press — a picker with one choice asks nothing', async () => {
    mountBooks();
    await settle(0);

    invoke.mockClear();
    bookRow('Jude').click();
    await settle(0);

    expect(staged).toEqual([['Jude', 1]]);
    // Still only staging. The shortcut is a press saved, not a rule relaxed.
    expect(sent).toEqual([]);
    const called = invoke.mock.calls.map((c) => c[0]);
    expect(called).not.toContain('manual_fire');
    expect(called).not.toContain('fire_content');
    // …and no picker was drawn for it.
    expect(chips()).toHaveLength(0);
  });

  it('marks the chapter it opened, so coming back does not start again at 1', async () => {
    mountBooks();
    await settle(0);
    bookRow('Psalms').click();
    await settle(0);

    chips()[118].click(); // Psalms 119
    await tick();
    expect(staged).toEqual([['Psalms', 119]]);

    // Close the book and open it again — the operator's place is still there.
    bookRow('Psalms').click();
    await tick();
    bookRow('Psalms').click();
    await settle(0);

    const marked = chips().filter((c) => c.getAttribute('aria-current') === 'true');
    expect(marked).toHaveLength(1);
    expect(marked[0].textContent.trim()).toBe('119');
  });

  it('the mark is the rail’s own action and never wears a colour that claims a screen', () => {
    const css = read('./ui/ChapterPicker.svelte');
    const at = css.indexOf(".cp-chip[aria-current='true'] {");
    expect(at).toBeGreaterThan(-1);
    const rule = css.slice(at, css.indexOf('}', at));
    // Steel = the thing being worked on. Amber means ON AIR, amethyst means
    // rehearsal, cyan means the AI guessed — none of the three is true of a
    // chapter an operator opened into the grid.
    expect(rule).toContain('--v-sel-soft');
    expect(rule).not.toContain('--v-amber');
    expect(rule).not.toContain('--v-cyan');
    expect(rule).not.toContain('--v-amethyst');
  });

  it('brings a freshly opened book into view rather than leaving it below the fold', async () => {
    // jsdom does not implement `scrollIntoView`, so the component calls it
    // optionally and this test supplies it.
    const seen = [];
    Element.prototype.scrollIntoView = function stub(opts) {
      seen.push([this.className, opts]);
    };
    try {
      mountBooks();
      await settle(0);
      bookRow('Psalms').click();
      await settle(0);
      expect(seen.some(([cls]) => String(cls).includes('lr-row'))).toBe(true);
    } finally {
      delete Element.prototype.scrollIntoView;
    }
  });

  it('the Fire button is the shared control, not one more hand-rolled shape', async () => {
    invoke.mockImplementation(async (cmd) =>
      cmd === 'search_scripture' ? [HIT_TYPED] : cmd === 'list_books' ? BOOKS : [],
    );
    mount();
    await search('ps 23 1');
    expect(host.querySelector('.lr-fire').classList.contains('r-btn')).toBe(true);
  });
});
