// THE LIBRARY'S OUTPUT RAIL — the queue, and the one press that fires it.
//
// ── How this file found its subject, THREE times ─────────────────────────────
//
// It was first written against `PreviewProgram.svelte`, which reads like the
// switcher this product's safety model describes: two panes, LEFT what is coming,
// RIGHT what they can see. Fourteen tests passed. `scripts/qa-inventory.mjs` then
// reported that nothing imported that component, and it was deleted.
//
// So the tests moved to `LiveOutputRail.svelte` — and grew a second set about a
// `preview` prop and a "Take to screen" button. The audit then found that
// `stage()` in `Library.svelte` had zero callers, so `preview` was permanently
// null and that half could not render either. Seventeen green tests over a state
// the app could not reach, in a file whose opening comment was about exactly that
// mistake. The preview half went (audit P1-2, 2026-08-15).
//
// THE THIRD TIME is this rewrite, 2026-09-14, and it is a different shape of the
// same lesson: eleven green tests over panels that should not have been in this
// workspace at all. `docs/REBRAND.md` §10 asks the Library's right column to be an
// INSPECTOR for the selected item; what stood there was a second run surface —
// a programme monitor, a HEARD panel, a transcript and five run controls — every
// one of which already had an owner on `Live.svelte` or in the dock row. Tests do
// not make a duplicate correct; they make it harder to remove.
//
// WHERE THE OLD CLAIMS WENT, so none of them is merely dropped:
//
//   the monitor's amber/amethyst/blackout law → `Live.svelte` owns the programme
//       pane, pinned by `r2livepath.test.js` and `e2e.rs`; the law itself is
//       `colourlaw.test.js` and DECISIONS §22.
//   the panic tiles' "never report a success you did not achieve" → `Dock.svelte`
//       owns Clear and Blackout app-wide, pinned by `panic.test.js` (which is
//       where CLAUDE.md rule 15 names the instrument).
//   RG-63, a suggestion whose verse does not exist → `Live.svelte`'s primary card
//       and its also-pending rows, pinned by `detect.test.js`. The Library no
//       longer renders an Approve at all, which is one fewer door to keep that
//       guarantee on — and the last test below holds it AS AN ABSENCE, so the
//       duplicate cannot come back unnoticed.
//
// What is left here is the thing this pane uniquely owns and nothing else in
// Relay renders: UP NEXT, and Go Live.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
const { capture } = await import('./stores/capture.js');
const { setSafeMode } = await import('./boot/boot.js');

const A = { reference: 'John 3:16', text: 'For God so loved the world' };
const B = { reference: 'Romans 8:28', text: 'And we know that all things work together' };

let host;
let app;

function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new LiveOutputRail({ target: host, props: { queue: [], ...props } });
  return host;
}

const btn = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.includes(text));
const rows = () => [...host.querySelectorAll('.lo-q')];

/**
 * Let a clicked handler finish.
 *
 * `tick()` alone is not enough: `capture.js` reaches the backend through a dynamic
 * `import('@tauri-apps/api/core')`, which resolves a turn later than Svelte's
 * scheduler. A test that only ticks sees zero calls and reads like the button is dead.
 */
async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  setSafeMode(false);
  capture.update((s) => ({ ...s, available: true }));
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

describe('up next is the queue, and nothing else renders it', () => {
  it('an empty queue says what fills it rather than reading as broken', () => {
    mount();
    expect(host.textContent).toMatch(/Nothing queued/i);
    expect(host.textContent).toMatch(/Cue in Live/i);
    expect(rows()).toHaveLength(0);
  });

  it('lists the queue in order and names the next item on the button', () => {
    mount({ queue: [A, B] });
    expect(rows().map((r) => r.querySelector('b').textContent)).toEqual([
      'John 3:16',
      'Romans 8:28',
    ]);
    expect(btn('Go Live').textContent).toContain('John 3:16');
    // The NEXT item is marked, and it is marked in steel — it is the thing you
    // are working on, not the thing a congregation is looking at. Amber may only
    // ever mean the latter (CLAUDE.md rule 18).
    expect(rows()[0].className).toContain('next');
    expect(rows()[0].className).not.toContain('air');
  });

  it('reordering and removing go through the pure queue helpers', async () => {
    let queue = [A, B];
    mount({ queue, onQueueChange: (q) => (queue = q) });
    rows()[1].querySelector('[aria-label="Move up"]').click();
    expect(queue.map((q) => q.reference)).toEqual(['Romans 8:28', 'John 3:16']);

    app.$set({ queue });
    await tick();
    rows()[0].querySelector('[aria-label="Remove"]').click();
    expect(queue.map((q) => q.reference)).toEqual(['John 3:16']);
  });
});

describe('go live is the one control here that can reach a screen', () => {
  it('fires the top of the queue and drops it, in that order', async () => {
    const fired = [];
    let queue = [A, B];
    mount({
      queue,
      onQueueChange: (q) => (queue = q),
      onFireQueued: (item) => {
        fired.push(item.reference);
        return Promise.resolve();
      },
    });
    btn('Go Live').click();
    await settle();
    expect(fired).toEqual(['John 3:16']);
    expect(queue.map((q) => q.reference)).toEqual(['Romans 8:28']);
  });

  it('a FIRE THAT FAILED leaves the item in the queue and says why', async () => {
    // The half that matters: a take that did not reach a screen must not look
    // like one that did. If the item were dropped anyway the operator would
    // press Go Live again and send the SECOND verse.
    let queue = [A, B];
    mount({
      queue,
      onQueueChange: (q) => (queue = q),
      onFireQueued: () => Promise.reject('no output channel'),
    });
    btn('Go Live').click();
    await settle();
    await settle();
    expect(queue.map((q) => q.reference)).toEqual(['John 3:16', 'Romans 8:28']);
    const err = host.querySelector('.lo-err');
    expect(err).toBeTruthy();
    expect(err.getAttribute('role')).toBe('alert');
    expect(host.querySelector('.lo-msg')).toBe(null);
  });

  it('is disabled in safe mode, and disabled with nothing queued', async () => {
    mount({ queue: [A] });
    expect(btn('Go Live').disabled).toBe(false);
    setSafeMode(true);
    await tick();
    expect(btn('Go Live').disabled).toBe(true);
    setSafeMode(false);
    app.$set({ queue: [] });
    await tick();
    expect(btn('Go Live').disabled).toBe(true);
  });
});

describe('the duplicated run surface is gone, and stays gone', () => {
  // Resolved from the working directory, the way `surface.test.js` does it: this
  // file mounts a component, so `import.meta.url` is the jsdom document's URL and
  // not a `file:` one.
  const code = () =>
    readFileSync(join(process.cwd(), 'src/lib/views/library/LiveOutputRail.svelte'), 'utf8')
      .replace(/<!--[\s\S]*?-->/g, ' ')
      .replace(/(^|\s)\/\/[^\n]*/g, ' ');

  it('renders no programme monitor, no HEARD panel and no transcript', () => {
    mount({ queue: [A] });
    const text = host.textContent;
    for (const gone of ['Live Output', 'Heard', 'Transcript', 'Program']) {
      expect(text, `${gone} belongs to Live and the dock row, not to the Library`).not.toMatch(
        new RegExp(gone, 'i'),
      );
    }
  });

  it('imports none of the run-surface wrappers it used to own', () => {
    const src = code();
    // A SOURCE assertion on purpose. What this guards is a boundary, and the way
    // a boundary breaks is somebody re-adding one helpful control: every name
    // below now has exactly one owner, and a second copy is how the two come to
    // disagree. `confirmDetection` is the sharpest — this rail carried its own
    // RG-63 absent-verse guard beside Live's.
    for (const wrapper of [
      'confirmDetection',
      'dismissDetection',
      'setRehearsal',
      'clearScreens',
      'blackScreen',
      'startCountdown',
      'startCapture',
      'stopCapture',
      'listOutputChannels',
    ]) {
      expect(src, `${wrapper} has an owner outside the Library`).not.toMatch(
        new RegExp(`\\b${wrapper}\\b`),
      );
    }
  });

  it('offers no Approve — so RG-63 has one fewer door to be kept on', () => {
    mount({ queue: [A] });
    expect(btn('Approve')).toBeUndefined();
    expect(btn('Nothing to send')).toBeUndefined();
  });
});
