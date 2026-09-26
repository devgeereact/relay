// ONE CHAPTER PICKER, TWO MOUNTS (RG-216).
//
// The operator's words: *"I want the Library to have a chapter picker as on the
// live workspace"*. Live's rail opens a book into a fixed grid of numbered
// chips; the Library offered a `<select>` of "Chapter 1 … Chapter 150" on the
// pane head — the same job, a different shape, and the wrong shape for the job.
// Reading 119 out of 150 in a dropdown means scrolling a list; the grid's own
// comment records why it stopped being a wrapped row of chips, and every word of
// that reasoning applies to the Library.
//
// **The picker is a component, and that is the whole point of the change.** A
// copy of the grid in `Browse.svelte` would be a second set of chips that agrees
// with Live today and drifts next year, which is the failure this repository has
// recorded four times under a different name. So the rule — a fixed grid, tabular
// figures, the one you opened marked in steel and never in amber — lives in one
// file and both surfaces mount it.
//
// Watched to fail: the mounted cases return no `.cp-chip` at all before the
// component exists, and the Library case finds the `<select>` still in the head.
import { describe, it, expect, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const read = (p) => readFileSync(join(process.cwd(), p), 'utf8');
const BIBLE = 'src/lib/views/library/Browse.svelte';
const RAIL = 'src/lib/LiveRail.svelte';

function mount(Component, props = {}) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = new Component({ target: host, props });
  return { host, app, done: () => (app.$destroy(), host.remove()) };
}

describe('the picker itself', () => {
  it('draws one chip per chapter, each named for a reader who cannot see the grid', async () => {
    const ChapterPicker = (await import('./ui/ChapterPicker.svelte')).default;
    const { host, done } = mount(ChapterPicker, { book: 'John', count: 21 });
    await tick();
    const chips = [...host.querySelectorAll('.cp-chip')];
    expect(chips).toHaveLength(21);
    expect(chips[0].getAttribute('aria-label')).toBe('John chapter 1');
    expect(chips[20].textContent.trim()).toBe('21');
    done();
  });

  it('marks the one you are on, and marks nothing when you are on none', async () => {
    const ChapterPicker = (await import('./ui/ChapterPicker.svelte')).default;
    const on = mount(ChapterPicker, { book: 'John', count: 21, current: 3 });
    await tick();
    expect(on.host.querySelectorAll('[aria-current="true"]')).toHaveLength(1);
    expect(on.host.querySelector('[aria-current="true"]').textContent.trim()).toBe('3');
    on.done();

    const off = mount(ChapterPicker, { book: 'John', count: 21 });
    await tick();
    expect(off.host.querySelectorAll('[aria-current="true"]')).toHaveLength(0);
    off.done();
  });

  it('hands the NUMBER back, not an event to be re-read out of the DOM', async () => {
    const ChapterPicker = (await import('./ui/ChapterPicker.svelte')).default;
    const picked = [];
    const { host, done } = mount(ChapterPicker, {
      book: 'John',
      count: 21,
      onPick: (n) => picked.push(n),
    });
    await tick();
    host.querySelectorAll('.cp-chip')[2].click();
    expect(picked).toEqual([3]);
    done();
  });

  it('a disabled picker offers nothing pressable', async () => {
    const ChapterPicker = (await import('./ui/ChapterPicker.svelte')).default;
    const picked = [];
    const { host, done } = mount(ChapterPicker, {
      book: 'John',
      count: 21,
      disabled: true,
      onPick: (n) => picked.push(n),
    });
    await tick();
    expect([...host.querySelectorAll('.cp-chip')].every((c) => c.disabled)).toBe(true);
    done();
  });
});

describe('and both surfaces mount the same one', () => {
  it('the Library opens a book into the grid', async () => {
    const Browse = (await import('./views/library/Browse.svelte')).default;
    invoke.mockResolvedValue([]);
    const { host, done } = mount(Browse, {
      books: [
        { book: 'Genesis', chapters: 50 },
        { book: 'John', chapters: 21 },
      ],
    });
    await tick();
    // Nothing until a book is chosen: a grid of 1189 chips is not a picker.
    expect(host.querySelectorAll('.cp-chip')).toHaveLength(0);
    [...host.querySelectorAll('.br-book')].find((b) => b.textContent.includes('John')).click();
    await tick();
    await tick();
    expect(host.querySelectorAll('.cp-chip'), 'the Library still has no chapter grid').toHaveLength(21);
    done();
  });

  it('…and the dropdown it replaces is gone, not sitting beside it', () => {
    // Two controls for one piece of state is the argument that deleted the book
    // select from this same head, and it has not changed.
    expect(codeOnly(read(BIBLE)), 'the Chapter select survived').not.toMatch(/aria-label="Chapter"/);
  });

  it('neither surface has its own copy of the grid', () => {
    for (const f of [BIBLE, RAIL]) {
      expect(codeOnly(read(f)), `${f} mounts no shared picker`).toMatch(/ChapterPicker/);
    }
    // The rail's own chip markup is gone rather than left beside the component,
    // which is what "one picker" means. The CSS class is the tell.
    expect(codeOnly(read(RAIL)), 'the rail kept its own chips').not.toMatch(/class="lr-chip"/);
  });
});
