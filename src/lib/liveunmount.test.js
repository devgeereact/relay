// THE RUN SURFACE THAT WOULD NOT LET GO.
//
// `Live.svelte`'s `onMount` is `async`, and it registered the transport keys and
// subscribed to the `live` store AFTER five awaited backend round trips. Svelte
// runs `onDestroy` the moment the component goes away; it does not wait for an
// async `onMount` to finish. So a workspace switch inside that window left the
// teardown with nothing to tear down — `unregisterKeys`, `unsubLive` and
// `unsubNav` were all still `undefined` when `onDestroy` ran, and were assigned a
// moment later, by a view that no longer exists.
//
// What that costs, in the room:
//
//   · `registerContext` is ONE global slot, last writer wins. A dead Live that
//     registers after its own teardown owns `→`, `←` and `Space` on every other
//     workspace. The operator is on Templates; they press the key they press more
//     than any other; a plan slide goes to the congregation's screen from a view
//     they cannot see.
//   · the leaked `live` subscription outlives the view and fires `setStageNext`
//     into the preacher's monitor on every clear, once per leaked mount.
//   · the async tail finishes its session restore and writes `liveCue` — the
//     playhead — after the operator has left. CLAUDE.md: wiping the position
//     would make the next `→` restart the plan at cue 1.
//
// This is the rebrand's own acceptance clause read strictly: "the programme is
// content, not an index into a grid", and switching workspace must not disturb
// either it or the playhead.
//
//   npx vitest run src/lib/liveunmount.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { activeActions } = await import('./shortcuts.js');
const Live = (await import('./views/Live.svelte')).default;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));

/** Every backend read resolves, but only after `ms` — the window a fast
 *  workspace switch lands in. */
function slowBackend(ms) {
  invoke.mockImplementation(
    (cmd) =>
      new Promise((res) => {
        setTimeout(() => {
          if (cmd === 'list_plans' || cmd === 'list_output_channels' || cmd === 'list_templates')
            res([]);
          else if (cmd === 'rehearsal') res(false);
          else res(null);
        }, ms);
      }),
  );
}

let host;
beforeEach(() => {
  invoke.mockReset();
  cap.live.set(null);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  host = document.createElement('div');
  document.body.appendChild(host);
});
afterEach(() => {
  host.remove();
});

describe('a workspace switch during Live’s mount', () => {
  it('leaves no transport keys behind — a destroyed view may not own → and Space', async () => {
    slowBackend(30);
    const app = new Live({ target: host, props: {} });
    // The operator changes their mind before the first read comes back.
    app.$destroy();
    // …and the mount finishes anyway.
    await settle(120);
    expect(get(activeActions)).toEqual([]);
  });

  it('leaves no live-store subscription behind — the preacher’s monitor is written once', async () => {
    slowBackend(30);
    const app = new Live({ target: host, props: {} });
    app.$destroy();
    await settle(120);

    invoke.mockReset();
    invoke.mockResolvedValue(null);
    // Something went on the wall, then came off it. The leaked subscriber's
    // whole job is to fire `set_stage_next` on that transition.
    cap.live.set({ reference: 'John 3:16', text: 'For God so loved' });
    cap.live.set(null);
    await settle(20);
    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('set_stage_next');
  });

  it('does not write the playhead after the operator has left', async () => {
    // A session that names a plan, so the restore path is the one that runs.
    const sess = await import('./session.js');
    sess.setSession({ planId: 1, liveCueId: 9, liveSlide: 1, liveOnAir: true });
    invoke.mockImplementation(
      (cmd) =>
        new Promise((res) => {
          setTimeout(() => {
            if (cmd === 'list_plans') res([{ id: 1, title: 'Sunday', plan_date: '2026-09-14', cue_count: 1 }]);
            else if (cmd === 'plan_items') res([]);
            else if (cmd === 'list_output_channels' || cmd === 'list_templates') res([]);
            else if (cmd === 'rehearsal') res(false);
            else res(null);
          }, 30);
        }),
    );
    // The operator is somewhere else, and the playhead is theirs.
    cap.liveCue.set({ cueId: 42, slide: 3, onAir: false });
    const app = new Live({ target: host, props: {} });
    app.$destroy();
    await settle(200);
    expect(get(cap.liveCue)).toEqual({ cueId: 42, slide: 3, onAir: false });
  });
});
