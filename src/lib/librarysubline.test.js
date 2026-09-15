// THE CARD'S SECOND LINE — REBRAND §10, the prototype's `.cell .sub2`.
//
// The prototype's Library cell is thumb → title → a quieter line under it. Relay's
// card was thumb → footer(number · key · reference · kebab) and stopped there, so a
// chapter of scripture rendered as twelve cards that differ only in a verse number.
// The thumbnail does carry the words, but it carries them through the operator's
// own template, fitted into a 268px × 151px box — a long verse is shrunk there
// until it is a grey smudge (the floor rule 37 exists for is about exactly this
// shrink), and a picture or a document carries no words at all.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Test the bug, not the fix (CLAUDE.md). Each assertion below was watched to go
// RED with the change reverted:
//
//   · "a card carries a second line" — reverted by deleting the `{#if sub}` block
//     from `VerseDeck.svelte`: `.vd-sub` is not in the DOM and the test fails.
//   · "only the FIRST line" — reverted by returning `s` unchanged from `subLine`:
//     the announcement's three lines arrive as one run-on sentence, because a
//     `white-space: nowrap` ellipsis renders a newline as a space, and the
//     assertion that the second paragraph is absent fails.
//   · "no sub, no line" — reverted by rendering the span unconditionally: an
//     empty `.vd-sub` appears under every card and the test fails.
//   · "every pane that has words to add supplies them" — reverted by removing
//     `sub:` from any one of the four panes: the test names that file.

import { describe, it, expect, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

function mount(Component, props) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = new Component({ target: host, props });
  return { host, done: () => (app.$destroy(), host.remove()) };
}

const sub = (host) => host.querySelector('.vd-sub');

describe('§10 · the Library card says what it holds, not only what it is called', () => {
  it('a card carries a second line under its reference', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [
        {
          reference: 'John 3:16',
          label: 'John 3:16',
          text: 'For God so loved the world',
          sub: 'For God so loved the world',
          slideNo: 1,
        },
      ],
      layout: 'grid',
      press: 'select',
    });
    await tick();

    expect(sub(host)).not.toBeNull();
    expect(sub(host).textContent.trim()).toBe('For God so loved the world');
    // And it is UNDER the reference, in the same column — not beside it, where it
    // would ellipse against the kebab instead of against the card's own edge.
    const where = host.querySelector('.vd-where');
    expect(where).not.toBeNull();
    expect(where.querySelector('.vd-ref')).not.toBeNull();
    expect(where.querySelector('.vd-sub')).not.toBeNull();
    done();
  });

  it('shows only the FIRST line of a body that has several', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [
        {
          reference: 'Car park',
          label: 'Car park',
          text: 'The north gate is closed.\n\nUse the lane by the hall.',
          sub: 'The north gate is closed.\n\nUse the lane by the hall.',
          slideNo: 1,
        },
      ],
      layout: 'grid',
      press: 'select',
    });
    await tick();

    // A CSS ellipsis on a nowrap box renders a newline as a SPACE, so without the
    // split the two paragraphs read as one sentence that was never written.
    expect(sub(host).textContent.trim()).toBe('The north gate is closed.');
    expect(sub(host).textContent).not.toContain('Use the lane');
    done();
  });

  it('draws no line at all when the pane has nothing to add', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [{ reference: 'logo.png', label: 'logo.png', text: '', slideNo: 1 }],
      layout: 'grid',
      press: 'select',
    });
    await tick();

    // An empty second line is a strip of blank space under every card claiming
    // there is more to know.
    expect(sub(host)).toBeNull();
    done();
  });

  it('and a sub of only whitespace is nothing to add', async () => {
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [{ reference: 'Untitled', label: 'Untitled', text: '', sub: '  \n \n ', slideNo: 1 }],
      layout: 'grid',
      press: 'select',
    });
    await tick();

    expect(sub(host)).toBeNull();
    done();
  });

  it('every Library pane with words beyond the title supplies them', () => {
    // Source-level, because these are four separate `deck` builders and the bug
    // this guards is one of them being added or rewritten without the field —
    // the exact shape CLAUDE.md's "enumerate every caller" note describes.
    const panes = ['Browse', 'Scripture', 'Announcements', 'LyricsPane', 'MediaLibrary'];
    for (const p of panes) {
      const src = readFileSync(join('src/lib/views/library', `${p}.svelte`), 'utf8');
      expect(src, `${p}.svelte must give its cards a second line`).toMatch(/^\s*sub:/m);
    }
  });

  // ── MEDIA · what the line says when there is no caption column ────────────
  //
  // REBRAND §10 asks for a caption stored apart from the item's name and that is
  // NOT built — `filename` is already the operator's own words, so a caption
  // would be a second operator-authored string beside the one that exists, and
  // §10's own sentence for media is "the slide *is* the picture", so none of it
  // reaches a congregation. The line is facts instead, and the facts must not be
  // reformatted into a lie.
  //
  // Each watched to go RED as noted.
  it('a media card names the kind and the date it arrived', async () => {
    const { mediaSub } = await import('./views/library/collections.js');
    expect(mediaSub({ kind: 'image', created_at: '2026-03-04' })).toBe('Image · added 2026-03-04');
    expect(mediaSub({ kind: 'video', created_at: '2026-03-04' })).toBe('Video · added 2026-03-04');
    expect(mediaSub({ kind: 'document', created_at: '2026-03-04' })).toBe(
      'Document · added 2026-03-04',
    );
  });

  it('says only the kind when a row has no date at all', async () => {
    // `created_at` defaults to '' on a row written before the column carried one.
    // The line must then be the kind alone — never the word "added" with nothing
    // after it, which is a field that failed to load wearing the look of a fact.
    // Reverted by dropping the `date ?` branch: both cases render "Image · added ".
    const { mediaSub } = await import('./views/library/collections.js');
    expect(mediaSub({ kind: 'image', created_at: '' })).toBe('Image');
    expect(mediaSub({ kind: 'image' })).toBe('Image');
    expect(mediaSub({ kind: 'image', created_at: '   ' })).toBe('Image');
    expect(mediaSub({})).toBe('File');
  });

  it('a media ROW is not a bare filename either', async () => {
    // List layout. The row already carried the item's words, so it needs no line
    // of its own — except where there are none: a media item has `text: ''`
    // because a picture is its own content, and the row was a filename over a
    // blank half-row. Reverted by deleting the `{:else if subLine(v.sub)}` branch.
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [
        {
          reference: 'Baptism title card',
          label: 'Baptism title card',
          text: '',
          sub: 'Image · added 2026-03-04',
          slideNo: 1,
        },
      ],
      layout: 'list',
      press: 'select',
    });
    await tick();

    const row = host.querySelector('.vd-rtext');
    expect(row, 'a media row must say something beyond its name').not.toBeNull();
    expect(row.textContent.trim()).toBe('Image · added 2026-03-04');
    done();
  });

  it('and a row that HAS words still shows the words, not the sub', async () => {
    // The fallback must not become a replacement: an announcement's `sub` is the
    // first line only, and a row that had the whole body may not quietly lose it.
    const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
    const { host, done } = mount(VerseDeck, {
      items: [
        {
          reference: 'Car park',
          label: 'Car park',
          text: 'The north gate is closed.\nUse the lane by the hall.',
          sub: 'The north gate is closed.\nUse the lane by the hall.',
          slideNo: 1,
        },
      ],
      layout: 'list',
      press: 'select',
    });
    await tick();

    expect(host.querySelector('.vd-rtext').textContent).toContain('Use the lane by the hall.');
    done();
  });

  it('is dated EXACTLY as stored, never through a locale', () => {
    // An ISO date with no time is parsed as UTC midnight and rendered in local
    // time, so west of Greenwich every item in the library would be dated the day
    // BEFORE it was added — a wrong date on the one field an operator uses to tell
    // two near-identical title cards apart. Reverted by wrapping `date` in
    // `new Date(date).toLocaleDateString()`.
    //
    // The pattern looks for a CALL — `.toLocaleDateString(` — not the bare word.
    // CLAUDE.md records an entitlement test that passed on a broken file because
    // it grepped a comment; this file's own comment above names the method, and a
    // word-level scanner would fail on the prose that explains the rule.
    const src = readFileSync(join('src/lib/views/library', 'collections.js'), 'utf8');
    expect(src).not.toMatch(/\.toLocale[A-Za-z]*\s*\(/);
    // Guards the guard: the pattern must actually recognise the call it forbids.
    expect('new Date(d).toLocaleDateString()').toMatch(/\.toLocale[A-Za-z]*\s*\(/);
  });
});
