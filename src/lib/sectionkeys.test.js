// W2 · SECTION KEYS — the letter that puts a section of a song on the screens.
//
// `docs/REBRAND.md` §10. Four things this file exists to hold, all of them from
// the brief's acceptance clause:
//
//   1. a section key fires that section, and fires NOTHING while a field has
//      focus;
//   2. a panic key is never shadowed — `Escape` still clears and `b` still
//      blacks out, whatever a song's sections are called;
//   3. two sections cannot share a key;
//   4. the label the key fires goes to the RECORD and not to the glass — the
//      caller says what it fired and `fire_content` decides what is shown
//      (DECISIONS §73, CLAUDE.md rule 36).
//
//   npx vitest run src/lib/sectionkeys.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { tick } from 'svelte';
import * as svelteRuntime from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { assignKeys, resolveKeystroke, keyIndex, kindOf, explicitKey, RESERVED } = await import(
  './sectionkeys.js'
);
const { parseLyrics, toText } = await import('./reflow.js');
const { installShortcuts, registerContext, SHORTCUTS } = await import('./shortcuts.js');
const { live, screenBlack, rehearsing, templates, capture, readErrors } = await import(
  './stores/capture.js'
);
const { setSafeMode } = await import('./boot/boot.js');

// Same self-detecting gate as `surface.test.js`: if `resolve.conditions` is ever
// tidied out of `vitest.config.js`, `onMount` is the SSR stub and a mounted test
// passes by doing nothing. These skip loudly instead.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const sec = (label, tag) => ({ label, tag: tag ?? '', lyrics: 'words' });

// ─────────────────────────────────────────────────────────────────────────────
// THE RESERVATION — read out of `shortcuts.js`, not restated
// ─────────────────────────────────────────────────────────────────────────────
describe('reserved letters', () => {
  it('reserves every single-character global key, by reading the real table', () => {
    // Not a hard-coded list: the point is that a key bound in SHORTCUTS tomorrow
    // is reserved tomorrow, with no edit to sectionkeys.js.
    const singles = SHORTCUTS.flatMap((s) => s.keys)
      .filter((k) => k.length === 1)
      .map((k) => k.toLowerCase());
    expect(singles.length).toBeGreaterThan(0);
    for (const k of singles) expect(RESERVED.has(k)).toBe(true);
  });

  it('reserves `b`, which is BLACKOUT — so no section may ever be given it', () => {
    expect(RESERVED.has('b')).toBe(true);
    const keys = assignKeys([sec('Verse 1'), sec('Bridge')]).map((k) => k.key);
    expect(keys).not.toContain('b');
  });

  it('derives the bridge’s letter from its own name rather than inventing one', () => {
    const [, bridge] = assignKeys([sec('Verse 1'), sec('Bridge')]);
    expect(bridge.key).toBe('r'); // b · r · i · d · g · e — `b` is blackout
    expect(bridge.why).toBe('fallback');
    expect(bridge.problem).toMatch(/blackout/i);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// ASSIGNMENT — "`v c b t i o`, numbered only when a kind repeats, per song"
// ─────────────────────────────────────────────────────────────────────────────
describe('assignKeys', () => {
  it('gives the six kinds their letters, unnumbered when each appears once', () => {
    const keys = assignKeys([
      sec('Intro'),
      sec('Verse 1'),
      sec('Chorus'),
      sec('Bridge'),
      sec('Tag'),
      sec('Outro'),
    ]).map((k) => k.key);
    expect(keys).toEqual(['i', 'v', 'c', 'r', 't', 'o']);
  });

  it('numbers a kind only when it repeats', () => {
    const one = assignKeys([sec('Verse 1'), sec('Chorus')]).map((k) => k.key);
    expect(one).toEqual(['v', 'c']);

    const two = assignKeys([sec('Verse 1'), sec('Chorus'), sec('Verse 2')]).map((k) => k.key);
    expect(two).toEqual(['v1', 'c', 'v2']);
  });

  it('numbers in section order, not in label order', () => {
    // "Verse 3" first is still v1, because the key is where to press, not what
    // the section is called.
    const keys = assignKeys([sec('Verse 3'), sec('Verse 1')]).map((k) => k.key);
    expect(keys).toEqual(['v1', 'v2']);
  });

  it('gives a section nobody named a letter from its own name', () => {
    const [hook] = assignKeys([sec('Hook')]);
    expect(hook).toMatchObject({ key: 'h', why: 'name' });
  });

  it('moves a second claimant off a letter already taken, and says so', () => {
    const [, interlude] = assignKeys([sec('Intro'), sec('Interlude')]);
    expect(interlude.key).toBe('n'); // i taken → n
    expect(interlude.problem).toMatch(/already taken/i);
  });

  it('is per song — the same label in two songs gets the same letter', () => {
    expect(assignKeys([sec('Chorus')])[0].key).toBe('c');
    expect(assignKeys([sec('Verse 1'), sec('Chorus')])[1].key).toBe('c');
  });

  it('survives junk without throwing', () => {
    expect(assignKeys(null)).toEqual([]);
    expect(assignKeys([{}])[0].key).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// "TWO SECTIONS CANNOT SHARE A KEY"
// ─────────────────────────────────────────────────────────────────────────────
describe('two sections cannot share a key', () => {
  it('never hands the same key to two sections, however they are named', () => {
    const keys = assignKeys([
      sec('Verse 1'),
      sec('Verse 2'),
      sec('Chorus'),
      sec('Chorus 2'),
      sec('Vamp'),
    ])
      .map((k) => k.key)
      .filter(Boolean);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('refuses the SECOND of two explicit requests for one letter, and says which', () => {
    // Numbering two verses apart is not sharing. Two sections both ASKING for
    // `g` is, and Relay does not guess which one was meant.
    const [first, second] = assignKeys([sec('Bridge', 'G'), sec('Hook', 'G')]);
    expect(first).toMatchObject({ key: 'g', why: 'explicit' });
    expect(second.key).toBeNull();
    expect(second.problem).toMatch(/both ask/i);
  });

  it('refuses an explicit request for a Relay key and names what it does', () => {
    const [hook] = assignKeys([sec('Hook', 'B')]);
    expect(hook.key).toBeNull();
    expect(hook.problem).toMatch(/blackout/i);
  });

  it('treats `[Bridge:b]` as no request at all, because it is not one', () => {
    // `b` is what a bridge derives, so the tag carries no information the label
    // did not already carry. It falls through to the ordinary rule and lands on
    // `r`, with the reason printed — rather than being refused for asking for
    // something nobody can prove it asked for.
    expect(assignKeys([sec('Bridge', 'B')])[0]).toMatchObject({ key: 'r', why: 'fallback' });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE EXPLICIT KEY, AND WHERE IT LIVES
// ─────────────────────────────────────────────────────────────────────────────
describe('explicitKey', () => {
  it('reads a request out of `tag`, which is the field that survives a save', () => {
    expect(explicitKey({ tag: 'G', label: 'Bridge' })).toBe('g');
  });

  it('treats a tag that is merely what the label derives as no request at all', () => {
    expect(explicitKey({ tag: 'C', label: 'Chorus' })).toBe('');
    expect(explicitKey({ tag: 'V1', label: 'Verse 1' })).toBe('');
    // A ProPresenter import's `PC2` is not a key shape and must not be read as one.
    expect(explicitKey({ tag: 'PC2', label: 'Pre-Chorus 2' })).toBe('');
  });
});

describe('kindOf', () => {
  it('knows the six and nothing else', () => {
    for (const [label, letter] of [
      ['Verse 2', 'v'],
      ['Chorus', 'c'],
      ['Bridge', 'b'],
      ['Tag', 't'],
      ['Intro', 'i'],
      ['Outro', 'o'],
    ]) {
      expect(kindOf(label).letter).toBe(letter);
    }
    expect(kindOf('Pre-Chorus')).toBeNull();
    expect(kindOf('Interlude')).toBeNull();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE HALF-TYPED KEY
// ─────────────────────────────────────────────────────────────────────────────
describe('resolveKeystroke', () => {
  const keys = new Set(['v1', 'v2', 'c', 'r']);

  it('holds a prefix so `v2` can be typed', () => {
    const a = resolveKeystroke('', 'v', keys);
    expect(a).toEqual({ buffer: 'v', fire: null });
    expect(resolveKeystroke(a.buffer, '2', keys)).toEqual({ buffer: '', fire: 'v2' });
  });

  it('fires at once when the letter is the whole key', () => {
    expect(resolveKeystroke('', 'c', keys)).toEqual({ buffer: '', fire: 'c' });
  });

  it('does not let a half-typed key swallow the key that follows it', () => {
    // `v` is held; `c` continues nothing, so it is tried again as a fresh start
    // and fires the chorus rather than being eaten.
    expect(resolveKeystroke('v', 'c', keys)).toEqual({ buffer: '', fire: 'c' });
  });

  it('fires nothing, and holds nothing, for a letter no section owns', () => {
    expect(resolveKeystroke('', 'z', keys)).toEqual({ buffer: '', fire: null });
    expect(resolveKeystroke('v', 'z', keys)).toEqual({ buffer: '', fire: null });
  });

  it('ignores anything that is not a single letter or digit', () => {
    for (const k of ['Escape', 'ArrowRight', ' ', '', null, undefined]) {
      expect(resolveKeystroke('v', k, keys)).toEqual({ buffer: '', fire: null });
    }
  });

  it('takes a plain array as well as a Set', () => {
    expect(resolveKeystroke('', 'c', ['c'])).toEqual({ buffer: '', fire: 'c' });
  });
});

describe('keyIndex', () => {
  it('maps each key to the section it fires', () => {
    const map = keyIndex([sec('Verse 1'), sec('Chorus'), sec('Verse 2')]);
    expect([...map.entries()]).toEqual([
      ['v1', 0],
      ['c', 1],
      ['v2', 2],
    ]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// REFLOW — `[Chorus:c]`, and the round trip
// ─────────────────────────────────────────────────────────────────────────────
describe('reflow · the keyed header', () => {
  it('reads `[Bridge:g]` as a label and a key', () => {
    const [s] = parseLyrics('[Bridge:g]\nline one\nline two');
    expect(s).toMatchObject({ label: 'Bridge', tag: 'G', lyrics: 'line one\nline two' });
    expect(assignKeys([s])[0]).toMatchObject({ key: 'g', why: 'explicit' });
  });

  it('leaves `[Bridge x2]` alone — the marker is a colon and ONE letter', () => {
    expect(parseLyrics('[Bridge x2]\nw')[0].label).toBe('Bridge x2');
    expect(parseLyrics('[Intro: soft]\nw')[0].label).toBe('Intro: soft');
  });

  it('round-trips a TWO-LINE SLIDE through text → slides → text, unchanged', () => {
    // The brief's acceptance clause, verbatim.
    const src = '[Chorus]\nway maker\nmiracle worker';
    expect(toText(parseLyrics(src))).toBe(src);

    const keyed = '[Bridge:g]\nline one\nline two';
    expect(toText(parseLyrics(keyed))).toBe(keyed);
  });

  it('round-trips a whole song, and is idempotent on a second pass', () => {
    const src = '[Verse 1]\na\nb\n\n[Chorus]\nc\nd\n\n[Bridge:g]\ne\nf';
    const once = toText(parseLyrics(src));
    expect(once).toBe(src);
    expect(toText(parseLyrics(once))).toBe(once);
  });

  it('does not write back a key that is merely what the label derives', () => {
    // `[Chorus:c]` asks for the letter a chorus gets anyway, so the editor is
    // not left carrying a marker that says nothing.
    expect(toText(parseLyrics('[Chorus:c]\nsing'))).toBe('[Chorus]\nsing');
  });

  it('leaves an imported tag that is not a key shape alone', () => {
    // `PC2` from a ProPresenter import must not be emitted as `:pc2`, which the
    // parser could not read back.
    expect(toText([{ tag: 'PC2', label: 'Pre-Chorus 2', lyrics: 'w' }])).toBe(
      '[Pre-Chorus 2]\nw',
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE KEY ON A REAL SURFACE
//
// A pure module nothing renders is not covered, however green its tests
// (CLAUDE.md, Testing). These mount the real `LyricsPane`, install the real
// global key handler, and press real keys.
// ─────────────────────────────────────────────────────────────────────────────
const SONG = {
  id: 4,
  title: 'Great Are You Lord',
  author: null,
  sections: [
    { tag: 'V1', label: 'Verse 1', lyrics: 'you give life' },
    { tag: 'C', label: 'Chorus', lyrics: 'it is your breath' },
    { tag: 'V2', label: 'Verse 2', lyrics: 'you restore' },
  ],
};

/** What `get_song` answers with. A test may swap it before mounting. */
let songFixture = SONG;

let host;
let app;
let teardown;

function mountInto(Component, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Component({ target: host, props });
  return host;
}

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

// 150 rather than a handful: this file runs alongside seventy others and the
// machine CI uses is roughly 13x slower than this one. A flaky test is worse
// than a missing one — it trains whoever sees it red to run it again rather
// than read it (`surface.test.js` says the same thing, for the same reason).
async function until(predicate, what, tries = 150) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(
    `timed out waiting for: ${what}\n---- what the DOM said ----\n${host?.textContent?.slice(0, 600)}`,
  );
}

function press(key, target = document.body) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  target.dispatchEvent(e);
  return e;
}

/**
 * Mount the pane, get a song open, and wait for its deck.
 *
 * It clicks the song in the rail rather than relying on the pane opening the
 * first one for itself, and it clicks "Try again" if the first read reported a
 * failure. Both are what an operator does, and the second is load-bearing for a
 * reason worth writing down: **vitest's mocked dynamic import is not safe under
 * concurrency**. `LyricsPane`'s `onMount` fires three reads through
 * `Promise.all`, each of which does `await import('@tauri-apps/api/core')`, and
 * one of the three reliably receives `undefined` for the module namespace. That
 * is a property of the harness, not of Relay — `readErrors` catches it, the pane
 * says so, and the retry the pane already offers clears it.
 */
async function openSong(el) {
  await until(() => {
    if (el.querySelectorAll('.vd-key').length) return true;
    const retry = [...el.querySelectorAll('button')].find((b) => /Try again/.test(b.textContent));
    if (retry) retry.click();
    else el.querySelector('.ly-song')?.click();
    return false;
  }, 'the deck to render its keys');
}

const fired = () => invoke.mock.calls.filter((c) => c[0] === 'fire_content');

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_songs') {
      return Promise.resolve([{ id: 4, title: SONG.title, author: null, section_count: 3 }]);
    }
    if (cmd === 'get_song') return Promise.resolve(songFixture);
    if (cmd === 'get_content_templates') return Promise.resolve({});
    return Promise.resolve([]);
  });
  live.set(null);
  screenBlack.set(false);
  rehearsing.set(false);
  templates.set([]);
  readErrors.set({});
  registerContext({});
  setSafeMode(false);
  songFixture = SONG;
  capture.update((s) => ({ ...s, available: true }));
});

afterEach(() => {
  teardown?.();
  teardown = null;
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
  registerContext({});
});

describe('a section key on the real surface', () => {
  itMounted('prints the key on the slide it fires, and only on that slide', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    await openSong(el);

    const caps = [...el.querySelectorAll('.vd-key')].map((k) => k.textContent.trim());
    expect(caps).toEqual(['v1', 'c', 'v2']);
  });

  itMounted('prints the key ONCE for a section that reflowed into several slides', async () => {
    // A section that is too long for one slide still has ONE key, and it sends
    // the section. Printing it on all three parts would be three claims about
    // one keystroke.
    songFixture = {
      id: 9,
      title: 'Long One',
      author: null,
      sections: [
        { tag: 'V1', label: 'Verse 1', lyrics: 'a\nb\nc\nd\ne\nf' },
        { tag: 'C', label: 'Chorus', lyrics: 'sing' },
      ],
    };
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    await openSong(el);

    // Four lines a slide, so verse 1 is two slides; three cards, two keys.
    expect(el.querySelectorAll('.vd-card').length).toBe(3);
    expect([...el.querySelectorAll('.vd-key')].map((k) => k.textContent.trim())).toEqual(['v', 'c']);
  });

  itMounted('fires that section — and passes the REAL label, not a blank one', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    teardown = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
    await openSong(el);

    press('c');
    await until(() => fired().length, 'the chorus to reach fire_content');

    const [, args] = fired()[0];
    expect(args.kind).toBe('song');
    expect(args.text).toBe('it is your breath');
    // DECISIONS §73 / r2livepath R2-H: the caller says what it fired; the
    // backend decides what is shown. A blank label here would cost the service
    // record the name of what was on the screens.
    expect(args.label).toBe('Great Are You Lord · Chorus');
  });

  itMounted('holds a half-typed key so `v2` can be typed', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    teardown = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
    await openSong(el);

    press('v');
    await settle();
    expect(fired(), 'a bare `v` names two sections — it must fire neither').toEqual([]);

    press('2');
    await until(() => fired().length, 'verse 2 to reach fire_content');
    expect(fired()[0][1].text).toBe('you restore');
  });

  itMounted('fires NOTHING while a field has focus', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    teardown = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
    await openSong(el);

    // Open the lyric editor and type into it: `c` is a section key AND a letter
    // in half the words of a song.
    const editBtn = [...el.querySelectorAll('button')].find((b) =>
      /Edit lyrics/.test(b.textContent),
    );
    editBtn.click();
    await until(() => el.querySelector('#ly-text'), 'the lyric editor to open');

    press('c', el.querySelector('#ly-text'));
    press('v', el.querySelector('#ly-text'));
    press('2', el.querySelector('#ly-text'));
    await settle();
    expect(fired()).toEqual([]);

    // …and the same key OUTSIDE the field still fires, so this test cannot pass
    // by the section keys simply not working.
    press('c');
    await until(() => fired().length, 'the same key outside the field to fire');
  });
});

describe('a section key never shadows a panic key', () => {
  itMounted('`b` blacks out; it does not fire a section', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    let blacked = 0;
    teardown = installShortcuts({
      clearScreens: () => {},
      blackScreen: () => {
        blacked += 1;
      },
    });
    await openSong(el);

    press('b');
    await settle();
    expect(blacked).toBe(1);
    expect(fired()).toEqual([]);

    // …and a key that is NOT a panic key still fires, so this cannot pass by the
    // section keys simply not working.
    press('c');
    await until(() => fired().length, 'an ordinary section key to still fire');
  });

  itMounted('`Escape` clears the screens; it does not fire a section', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    let cleared = 0;
    teardown = installShortcuts({
      clearScreens: () => {
        cleared += 1;
      },
      blackScreen: () => {},
    });
    await openSong(el);

    press('Escape');
    await settle();
    expect(cleared).toBe(1);
    expect(fired()).toEqual([]);

    press('c');
    await until(() => fired().length, 'an ordinary section key to still fire');
  });

  it('the dispatcher is a no-op, and eats no keystroke, with nothing registered', () => {
    registerContext({});
    teardown = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
    const e = press('z');
    expect(e.defaultPrevented).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// NO "ADD ALL TO LIVE" — REBRAND §10, and the dead tick box that was the
// nearest thing the Library had to one
//
// "A song joins a service through the plan." A control that puts a whole song on
// the screens in one action is the opposite of the product's one safety idea,
// which is that an operator chooses each slide.
//
// The tick box on every deck card says "Select for a bulk action". Scripture and
// Browse have one — "Queue N selected", which STAGES verses on the rail and
// fires none of them. Songs, announcements and media had the tick box and no
// action at all: it tinted the card and led nowhere. It is off on those three
// now, and songs deliberately gain no bulk action to replace it.
// ─────────────────────────────────────────────────────────────────────────────
describe('a song joins a service through the plan', () => {
  // Comments are stripped before scanning. A file that EXPLAINS why it has no
  // "add all" must not read as though it has one — and a scanner that cannot
  // tell a control from a sentence about a control will be weakened until it
  // cannot tell anything.
  const read = (p) =>
    readFileSync(new URL(p, import.meta.url), 'utf8')
      .replace(/<!--[\s\S]*?-->/g, ' ')
      .replace(/\/\*[\s\S]*?\*\//g, ' ')
      .replace(/(^|\s)\/\/[^\n]*/g, ' ');
  const LIBRARY = [
    './views/Library.svelte',
    './views/library/LyricsPane.svelte',
    './views/library/Announcements.svelte',
    './views/library/MediaLibrary.svelte',
    './views/library/Scripture.svelte',
    './views/library/Browse.svelte',
    './views/library/VerseDeck.svelte',
    './views/library/LiveOutputRail.svelte',
    // Added 2026-09-14 with the §10 rebuild. The inspector is where the two
    // things you can do to a selected item now live, so it is exactly the
    // surface a future "add the whole song" button would land on — and the rule
    // it would break ("a song joins a service through the plan") is the sentence
    // printed at the bottom of that pane.
    './views/library/Inspector.svelte',
  ];

  it('the scanner can still see a control — it is not blind to its own subject', () => {
    // A scanner that quietly narrows passes everything (CLAUDE.md, ipc.test.js,
    // twice). Prove it still matches the thing it is looking for.
    expect(/\b(add|fire|send|take|queue)\s+all\b/i.test('<button>Add all to Live</button>')).toBe(
      true,
    );
    expect(read('./views/library/VerseDeck.svelte')).not.toMatch(/REBRAND §10/);
  });

  it('offers no "add all" / "fire all" / "send all" control anywhere in the Library', () => {
    const offenders = LIBRARY.filter((f) =>
      /\b(add|fire|send|take|queue)\s+all\b/i.test(read(f)),
    );
    expect(offenders, 'a whole song reaching the screens in one action').toEqual([]);
  });

  it('fires exactly one slide per action — no library pane loops a fire', () => {
    // The shape to catch is a fire inside an iteration. `queueChecked` in
    // Scripture and Browse loops, and stages on the RAIL, which is not a fire.
    const offenders = [];
    for (const f of LIBRARY) {
      const src = read(f);
      for (const m of src.matchAll(/\.(forEach|map|filter)\([^\n]*\n?[^\n]*/g)) {
        if (/\b(fireContent|manualFire|fireMedia)\(/.test(m[0])) offenders.push(f);
      }
    }
    expect(offenders).toEqual([]);
  });

  itMounted('the song deck has no selection tick box, because nothing consumes one', async () => {
    const LyricsPane = (await import('./views/library/LyricsPane.svelte')).default;
    const el = mountInto(LyricsPane);
    await openSong(el);
    expect(el.querySelectorAll('.vd-check').length).toBe(0);
  });
});
