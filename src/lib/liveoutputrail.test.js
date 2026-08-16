// THE RUN COLUMN'S MONITOR TELLS THE TRUTH ABOUT THE WALL.
//
// `LiveOutputRail.svelte` is the run column: the monitor that answers the only
// question that matters mid-service — *what are they looking at right now* — plus
// the panic tiles. It is the most dangerous component in the app and, until this
// file, no test referenced it.
//
// ── How this file found its subject, twice ───────────────────────────────────
//
// It was first written against `PreviewProgram.svelte`, which reads like the
// switcher this product's safety model describes: two panes, LEFT what is coming,
// RIGHT what they can see. Fourteen tests passed. `scripts/qa-inventory.mjs` then
// reported that nothing imported that component, and it was deleted.
//
// So the tests moved here — and grew a second set about a `preview` prop and a
// "Take to screen" button. The audit then found that `stage()` in `Library.svelte`
// had zero callers, so `preview` was permanently null and that half could not
// render either. Seventeen green tests over a state the app could not reach, in a
// file whose opening comment was about exactly that mistake.
//
// The preview half is now GONE (audit P1-2, 2026-08-15). Going live from the
// Library fires the top of the QUEUE — a staging area that holds N items instead of
// one, and that actually exists. `Live.svelte` owns Preview ≠ Programme for the
// plan path, where it is implemented and reachable.
//
// What remains here is the law that was always true of this component:
//
//   AMBER MEANS LIVE. It appears when, and only when, a congregation is genuinely
//   looking. Not while blacked out. Not in rehearsal — that is amethyst.
//
//   CLAUDE.md (frontend shape, §15, §18) · DECISIONS §22

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
const { live, screenBlack, rehearsing, panicError, capture } = await import('./stores/capture.js');
const { setSafeMode } = await import('./boot/boot.js');

const ON_WALL = { reference: 'John 3:16', text: 'For God so loved the world' };

let host;
let app;

function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new LiveOutputRail({ target: host, props: { queue: [], ...props } });
  return host;
}

const monitor = () => host.querySelector('.lo-top');
const badge = () => host.querySelector('.r-badge');
const headline = () => host.querySelector('.lo-top .r-lbl').textContent.replace(/\s+/g, ' ').trim();
const tileNamed = (text) =>
  [...host.querySelectorAll('.lo-tile')].find((b) => b.textContent.includes(text));

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
  // `onMount` polls `list_output_channels`; an array keeps the monitor picker quiet.
  invoke.mockResolvedValue([]);
  live.set(null);
  screenBlack.set(false);
  rehearsing.set(false);
  panicError.set(null);
  setSafeMode(false);
  capture.update((s) => ({ ...s, available: true }));
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

describe('the monitor says what is actually on the wall', () => {
  it('says so in words when the screens are clear', () => {
    mount();
    expect(monitor().textContent).toMatch(/Nothing is on the screens/i);
    // A blank monitor reads as "the feed died"; a sentence reads as "nothing is up".
    expect(badge().textContent.trim()).toBe('Clear');
    expect(badge().className).toMatch(/grey/);
  });

  it('shows the live verse, in amber, when a congregation is looking', async () => {
    mount();
    live.set(ON_WALL);
    await tick();

    expect(monitor().textContent).toMatch(/God so loved/);
    expect(badge().textContent.trim()).toBe('Live');
    expect(badge().className).toMatch(/amber/);
    expect(headline()).toMatch(/Program$/);
  });

  it('goes amethyst in rehearsal, never amber', async () => {
    // Amethyst is the whole point: same content, same controls, and a colour that
    // says out loud that nobody is looking.
    mount();
    rehearsing.set(true);
    live.set(ON_WALL);
    await tick();

    expect(badge().textContent.trim()).toBe('Rehearsal');
    expect(badge().className).toMatch(/amethyst/);
    expect(badge().className).not.toMatch(/amber/);
    expect(headline()).toMatch(/Rehearsal$/);
  });

  it('drops the amber during a blackout, though the content is still armed', async () => {
    // `live` survives a blackout — the content is loaded, the screens are just
    // dark. Amber must follow the CONGREGATION, not the state variable.
    mount();
    live.set(ON_WALL);
    screenBlack.set(true);
    await tick();

    expect(monitor().textContent).toMatch(/Blacked out/i);
    expect(badge().className).not.toMatch(/amber/);
    expect(badge().textContent.trim()).toBe('Clear');
  });
});


describe('the panic tiles tell the truth', () => {
  it('Clear Screens reports success only when it succeeded', async () => {
    mount();
    live.set(ON_WALL);
    await tick();

    tileNamed('Clear Screens').click();
    await settle();

    expect(invoke).toHaveBeenCalledWith('clear_screens');
    expect(host.querySelector('.lo-msg')?.textContent).toMatch(/Screens cleared/i);
    expect(get(panicError)).toBe(null);
  });

  it('a Clear that FAILS says nothing reassuring, and raises the banner', async () => {
    // The bug this pins, verbatim from the repo's history: the toast fired
    // unconditionally, so the operator was told the wall was clean while the verse
    // was still in front of the congregation — and then stopped looking at it.
    mount();
    live.set(ON_WALL);
    await tick();
    invoke.mockRejectedValue('emit failed');

    tileNamed('Clear Screens').click();
    await settle();

    expect(host.querySelector('.lo-msg')).toBe(null);
    expect(get(panicError)).toBeTruthy();
  });

  it('Blank Screen carries the identical contract — it is a panic control too', async () => {
    mount();
    live.set(ON_WALL);
    await tick();
    invoke.mockRejectedValue('emit failed');

    tileNamed('Blank Screen').click();
    await settle();

    expect(host.querySelector('.lo-msg')).toBe(null);
    expect(get(panicError)).toBeTruthy();
  });
});

