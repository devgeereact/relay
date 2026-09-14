// ONE ROW OF CHROME, AND NOTHING LOST ON THE WAY (REBRAND §10).
//
// The Library stacked four full-width bands above the first item an operator
// could see: collection chips, a Bible/Saved row, a filter bar of five controls,
// then the pane head. The prototype has two. Collapsing a toolbar is the easiest
// change in this repository to get wrong, because the failure is silent: a
// control does not break, it just stops being on the screen, and no instrument
// Relay has can see the difference between "deliberately removed" and "lost".
//
// So this file is about the SECOND half of that sentence. It asserts where each
// control went, not that the row is short — a screenshot can say the latter and
// nothing can say the former.
//
// The one control that was genuinely DELETED is the book `<select>`: the Books
// rail beside the grid is the book picker, and the select was a second control
// over the same piece of state. Its absence is asserted too, so a well-meaning
// restoration has to argue with a test rather than with a comment.

import { describe, it, expect, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const read = (p) => readFileSync(join(process.cwd(), p), 'utf8');
const SHELL = 'src/lib/views/Library.svelte';
const BIBLE = 'src/lib/views/library/Browse.svelte';

/** Markup only: this repository's files explain their own rules in prose, and a
    grep a comment can trip is a grep that will trip on the next one. */
const markup = (p) =>
  read(p)
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/(^|\s)\/\/[^\n]*/g, ' ');

function mount(Component, props = {}) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = new Component({ target: host, props });
  return { host, app, done: () => (app.$destroy(), host.remove()) };
}

describe('the shell carries ONE chrome row', () => {
  it('the filter bar is gone from the shell', () => {
    // It was `<div class="lib-filters">` holding translation, search, book,
    // chapter, verse and Favourites.
    expect(markup(SHELL)).not.toMatch(/class="lib-filters"/);
  });

  it('the collection chips and the views inside them are on one line', () => {
    // `Collections.svelte` rendered the views on a row beneath the chips. A
    // choice between two panes of one collection is a refinement of the chip
    // beside it, not a tier of navigation.
    const bar = markup('src/lib/views/library/Collections.svelte');
    expect(bar).toMatch(/\.cr\s*\{[^}]*display:\s*flex[^}]*align-items:\s*center/);
    expect(bar).not.toMatch(/\.cr\s*\{[^}]*flex-direction:\s*column/);
  });

  it('the ONE search box is still in the shell, and there is still only one', () => {
    // "The search is one box." Five panes with their own would be five places to
    // look and five behaviours to learn.
    const searches = [...markup(SHELL).matchAll(/type="search"/g)];
    expect(searches).toHaveLength(1);
  });
});

describe('every control the row carried still exists, on a surface that owns it', () => {
  const bible = markup(BIBLE);

  it('translation, chapter, verse, Favourites and Sort are in the Bible pane', () => {
    for (const label of ['Translation', 'Chapter', 'Verse', 'Sort']) {
      expect(bible, `${label} left the shell and did not arrive`).toMatch(
        new RegExp(`aria-label="${label}"`),
      );
    }
    expect(bible).toMatch(/aria-pressed=\{favouritesOnly\}/);
  });

  it('and they are reachable while a bulk selection is active', () => {
    // The bulk actions replace the LEGEND, not the navigation. Putting them in
    // the `{:else}` of the controls would take the Bible's chapter picker away
    // from an operator who had ticked a verse.
    const head = bible.slice(bible.indexOf('br-mainhead'), bible.indexOf('br-body'));
    const guard = head.indexOf('{#if checked.size}');
    const close = head.indexOf('{/if}', guard);
    const inside = head.slice(guard, close);
    for (const label of ['Translation', 'Chapter', 'Verse', 'Sort']) {
      expect(inside, `${label} is hidden while verses are ticked`).not.toContain(
        `aria-label="${label}"`,
      );
    }
  });

  it('Import and New Item stay in the shell, where they act on the whole library', () => {
    expect(markup(SHELL)).toMatch(/New Item/);
    expect(markup(SHELL)).toMatch(/\{importing \? 'Importing…' : 'Import'\}/);
  });
});

describe('the book select was deleted, not moved', () => {
  it('neither the shell nor the Bible pane offers one', () => {
    for (const f of [SHELL, BIBLE]) {
      expect(markup(f), `${f} still has a Book select`).not.toMatch(/aria-label="Book"/);
    }
  });

  it('because the rail IS the book picker', async () => {
    const Browse = (await import('./views/library/Browse.svelte')).default;
    invoke.mockResolvedValue([]);
    const { host, done } = mount(Browse, {
      books: [
        { book: 'Genesis', chapters: 50 },
        { book: 'John', chapters: 21 },
      ],
    });
    await tick();
    const names = [...host.querySelectorAll('.br-book .nm')].map((n) => n.textContent);
    expect(names).toEqual(['Genesis', 'John']);
    done();
  });

  it('and the rail no longer carries a footer button that named a different one', () => {
    // "Browse All Books" set `book` to the FIRST book, directly beneath an "All
    // Books" control that really did clear the filter. Whichever one an operator
    // believed, one of them was lying.
    expect(markup(BIBLE)).not.toMatch(/Browse All Books/);
    // …and the control it was mistaken for is still there.
    expect(markup(BIBLE)).toMatch(/class="br-all/);
  });
});

describe('the slide grid says how many, and what a press does', () => {
  it('the Bible pane prints the item count and the press legend together', () => {
    expect(markup(BIBLE)).toMatch(/single click cues/);
    expect(markup(BIBLE)).toMatch(/double click opens/);
    expect(markup(BIBLE)).toMatch(/item\{numbered\.length === 1 \? '' : 's'\}/);
  });

  it('and the Bible opens in the GRID, like the other four panes', () => {
    // It was the only pane that opened as a numbered text list. A card is the
    // verse drawn through the same renderer that paints the projector; a row is
    // a description of one.
    for (const p of [
      BIBLE,
      'src/lib/views/library/Scripture.svelte',
      'src/lib/views/library/LyricsPane.svelte',
      'src/lib/views/library/MediaLibrary.svelte',
      'src/lib/views/library/Announcements.svelte',
    ]) {
      expect(read(p), `${p} does not open in the grid`).toMatch(/let layout = 'grid'/);
    }
  });
});
