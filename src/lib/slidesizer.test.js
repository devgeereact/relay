// W4 · A SLIDE CAN BE READ BEFORE IT IS PICKED.
//
// The grid was fixed at `minmax(158px, 1fr)` — and 158px is the width
// `runsurface.test.js` names in its own describe title as the size at which a
// LIVE cell and a CUED cell must be unmistakable. That is a claim about the
// border, not about the words: an operator choosing between two lyric slides is
// reading the lyrics, and there was no way to make them bigger.
//
// Four things are asserted here, and the third is the one a careless
// implementation gets wrong:
//
//   · the stepper steps, and stops at each end rather than going quiet;
//   · the choice survives a remount, through the session;
//   · IT MOVES THE GRID TRACK, NEVER `.sg-thumb` — the thumb carries
//     `container-type: inline-size` and is the container query every `cqw` in a
//     cell resolves against, so resizing it would re-scale the type inside each
//     cell instead of making the cell bigger;
//   · a console that has never touched it renders exactly the old 158px grid.
//
// Each was watched to fail with its change reverted; the reverts are named in the
// PR body.
//
//   npx vitest run src/lib/slidesizer.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { session, setSession, clearSession } = await import('./session.js');

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

const VERSES = [
  { verse: 1, reference: 'Psalms 23:1', text: 'The LORD is my shepherd; I shall not want.' },
  { verse: 2, reference: 'Psalms 23:2', text: 'He maketh me to lie down in green pastures.' },
];

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve([]);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'chapter_verses':
        return Promise.resolve(VERSES);
      case 'list_books':
        return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      case 'list_plans':
        return Promise.resolve([]);
      case 'rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      default:
        return Promise.resolve(null);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true, stt: { ...c.stt, loaded: true } }));
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
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

async function mountLive() {
  const Live = (await import('./views/Live.svelte')).default;
  app = new Live({ target: host, props: {} });
  await until(() => host.querySelector('.view-size'), 'the view controls');
}

const smaller = () => host.querySelectorAll('.view-size button')[0];
const bigger = () => host.querySelectorAll('.view-size button')[1];
const readout = () => host.querySelector('.view-szval').textContent.trim();
const track = () => host.querySelector('.sg-body').style.getPropertyValue('--sg-min');
const stored = () => {
  let s;
  session.subscribe((v) => (s = v))();
  return s.liveSlideSize;
};

describe('the slide sizer', () => {
  itMounted('is two real buttons with accessible names, not two glyphs', async () => {
    await mountLive();
    // `aria-label` because the face of each is a bare − and +, which a screen
    // reader announces as punctuation or as nothing at all. The two unnamed
    // controls this product once had were exactly this shape.
    expect(smaller().tagName).toBe('BUTTON');
    expect(bigger().tagName).toBe('BUTTON');
    expect(smaller().getAttribute('aria-label')).toBe('Smaller slide cells');
    expect(bigger().getAttribute('aria-label')).toBe('Bigger slide cells');
  });

  itMounted('steps up and down, and stops at each end rather than going quiet', async () => {
    await mountLive();
    // A console nobody has touched starts at the smallest step, so the LOWER end
    // is already reached and says so.
    expect(smaller().disabled).toBe(true);
    expect(bigger().disabled).toBe(false);

    const seen = [readout()];
    for (let i = 0; i < 6; i += 1) {
      bigger().click();
      await tick();
      seen.push(readout());
    }
    // Four steps, and pressing past the top changes nothing and disables the
    // control rather than silently doing nothing.
    expect([...new Set(seen)]).toEqual(['S', 'M', 'L', 'XL']);
    expect(bigger().disabled).toBe(true);
    expect(smaller().disabled).toBe(false);

    for (let i = 0; i < 6; i += 1) {
      smaller().click();
      await tick();
    }
    expect(readout()).toBe('S');
    expect(smaller().disabled).toBe(true);
  });

  itMounted('moves the GRID TRACK and never the thumb', async () => {
    await mountLive();
    // THE WHOLE POINT. `.sg-thumb` is `container-type: inline-size` and every
    // `cqw` inside a cell resolves against it, so a sizer that set a width there
    // would re-scale the type inside each cell rather than enlarge the cell.
    const before = host.querySelector('.sg-thumb')?.getAttribute('style') ?? '';
    expect(track()).toBe('158px');

    bigger().click();
    await tick();
    expect(track()).toBe('210px');
    expect(
      host.querySelector('.sg-thumb')?.getAttribute('style') ?? '',
      'the container-query root must not be touched by the sizer',
    ).toBe(before);
  });

  itMounted('the chosen size survives a remount, through the session', async () => {
    await mountLive();
    bigger().click();
    bigger().click();
    await tick();
    expect(readout()).toBe('L');
    expect(stored()).toBe(2);

    // The operator leaves Live and comes back — a real remount, and the session
    // is the only thing that crosses it.
    app.$destroy();
    host.innerHTML = '';
    await mountLive();
    expect(readout()).toBe('L');
    expect(track()).toBe('280px');
  });

  itMounted('a size nobody has chosen is the old 158px grid, exactly', async () => {
    await mountLive();
    expect(stored()).toBe(0);
    expect(track()).toBe('158px');
    expect(readout()).toBe('S');
  });

  itMounted('a stored size that makes no sense lands on a real step', async () => {
    // The session is a file on a disk: an older build, a hand edit, a half-written
    // payload. A cell track of `NaNpx` or `999px` is a grid an operator cannot use
    // and cannot obviously fix.
    for (const [saved, expectedTrack] of [
      [99, '370px'],
      [-4, '158px'],
      ['big', '158px'],
      [1.7, '210px'],
      [null, '158px'],
    ]) {
      clearSession();
      setSession({ liveSlideSize: saved });
      app?.$destroy();
      host.innerHTML = '';
      await mountLive();
      expect(track(), `stored ${JSON.stringify(saved)}`).toBe(expectedTrack);
    }
  });
});

describe('the wiring, read off the source', () => {
  const src = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  it('the grid rule reads the property and keeps its 158px fallback', () => {
    expect(src).toMatch(
      /\.sgrid\{display:grid; grid-template-columns:repeat\(auto-fill,minmax\(var\(--sg-min,158px\),1fr\)\);/,
    );
  });

  it('`<div class="sgrid">` is untouched — three tests locate it by spelling', () => {
    // `slidegridwiring.test.js` matches this element literally, so the property
    // rides on the pane BODY instead. Breaking it here breaks that file by
    // spelling rather than by meaning, which is the failure mode hardest to read.
    expect(src).toMatch(/<div class="sgrid">/);
    expect(src).toMatch(/<div class="pane-body sg-body" style="--sg-min:\{slideSize\.px\}px">/);
  });

  it('nothing sets a width on the container-query root', () => {
    const at = src.indexOf('\n  .sg-thumb{');
    const thumb = src.slice(at, at + 400);
    expect(thumb).toMatch(/container-type:inline-size/);
    expect(thumb).not.toMatch(/--sg-min/);
    expect(thumb).not.toMatch(/\bwidth:/);
  });
});
