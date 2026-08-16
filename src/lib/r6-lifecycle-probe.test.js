// THE COMPONENT-TEST APPARATUS IS REAL. This file is what keeps it that way.
//
// ── What this was, and why it is now inverted ────────────────────────────────
//
// Written 2026-08-14 as instrument calibration for the QA audit, this file
// originally asserted the OPPOSITE of everything below, and its closing comment
// read: *"If this ever reads ['mount','afterUpdate','destroy'], the config was
// fixed and every layer-B finding in QA-2026-08-14.md can be re-graded up."*
//
// It does now. Rather than delete a file whose whole subject is an assumption
// nobody had stated, it is turned around: the same three probes, asserting the
// repaired behaviour, so the defect cannot come back silently.
//
// ── The defect it guards ─────────────────────────────────────────────────────
//
// Svelte 4's package exports map `.` to `src/runtime/ssr.js` under every
// resolution condition except `browser`, and that file defines `onMount`,
// `beforeUpdate` and `afterUpdate` as literal empty functions. `environment:
// 'jsdom'` does not imply the `browser` condition. So a component was compiled
// for the DOM and handed the SSR stubs: it mounted, it rendered, and it silently
// skipped every load-on-mount path in the application. Nothing threw, so every
// such test passed by doing nothing.
//
// The asymmetry is what hid it for so long — `svelte/internal` has only a
// `default` condition, so rendering, `onDestroy`, `tick`, `setContext` and
// `createEventDispatcher` were all REAL. Teardown assertions worked. Nobody
// suspected the other half.
//
// The repair is one line in `vitest.config.js`: `resolve: { conditions:
// ['browser'] }`. Delete it and all three tests here fail immediately, which is
// the entire point of keeping them.
//
//   Found independently by three agents: R3-00, R2-G, and R6's P1-11.
import { describe, it, expect } from 'vitest';
import * as SvelteRuntime from 'svelte';

describe('the svelte runtime under test is the DOM one, not the SSR stubs', () => {
  it('resolves mount hooks with real bodies', () => {
    // The tell: an empty function stringifies with an empty body. These must not.
    const body = (fn) => String(fn).replace(/\s+/g, ' ');
    expect(body(SvelteRuntime.onMount)).not.toMatch(/\{\s*\}$/);
    expect(body(SvelteRuntime.beforeUpdate)).not.toMatch(/\{\s*\}$/);
    expect(body(SvelteRuntime.afterUpdate)).not.toMatch(/\{\s*\}$/);
  });

  it('and onDestroy and tick are still real — the asymmetry is gone, not reversed', () => {
    // Worth pinning both directions. The original bug was invisible precisely
    // because half the lifecycle worked, so "the other half now works too" is the
    // claim, not "something changed".
    const body = (fn) => String(fn).replace(/\s+/g, ' ');
    expect(body(SvelteRuntime.onDestroy)).not.toMatch(/\{\s*\}$/);
    expect(typeof SvelteRuntime.tick).toBe('function');
  });

  it('empirically: a mounted component runs its full lifecycle, in order', async () => {
    // The smallest possible component, built through the same plugin pipeline the
    // suite uses — the real path, not a simulation.
    const { default: Probe } = await import('./__r6probe.svelte');
    const seen = [];
    const target = document.createElement('div');
    document.body.appendChild(target);
    const c = new Probe({ target, props: { seen } });
    await SvelteRuntime.tick();
    c.$destroy();
    target.remove();

    // Before the fix this read ['destroy'] — the component mounted and rendered
    // and its onMount never ran. `afterUpdate` matters on its own account:
    // architecture rule 1 names it as the safe alternative to calling `tick()` in
    // a reactive block, and no test in this repo could observe it.
    expect(seen).toEqual(['mount', 'afterUpdate', 'destroy']);
  });
});
