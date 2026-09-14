// GO LIVE — THE ONE CONTROL IN THE LIBRARY THAT REACHES A CONGREGATION.
//
// `liveoutputrail.test.js` pins what happens when a take SUCCEEDS and when it
// FAILS. This file pins the two states that sit between those: the take that has
// not answered yet, and the wiring that decides whether the failure case is real
// in the shipping app at all.
//
// ── 1 · WHILE IT IS IN FLIGHT ────────────────────────────────────────────────
//
// `onQueueChange(rest)` runs AFTER the await — deliberately, so that a fire which
// failed leaves the verse in Up Next. That is also what made the second press
// dangerous: until the first resolves, `queue` still has the same item at its
// head, so pressing twice takes the SAME verse twice and neither press advances.
// Two broadcasts, and two `manual_fire` rows for a router that calibrates itself
// from that column (CLAUDE.md rule 14).
//
// Gating it is allowed BECAUSE it is not a panic control. Clear screens and
// Blackout are never gated and are not on this rail (rule 15, DECISIONS §20).
//
// ── 2 · WHETHER THE FAILURE CASE EXISTS OUTSIDE THE TEST ─────────────────────
//
// `liveoutputrail.test.js` supplies a REJECTING `onFireQueued` and asserts the
// item survives. The shipping app supplied `Library.svelte::fireQueued`, which
// wrapped both commands in its own `try/catch` and resolved either way — so the
// rail's `catch` could never run, `onQueueChange(rest)` always ran, and a verse
// that never reached a screen was dropped out of Up Next as though it had. The
// operator's next press then sends the SECOND item, which is precisely what that
// test exists to prevent. It passed the whole time.
//
// A guarantee is only kept on the doors you checked. This is the door.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
//   · "a second press" / "says so on its face" — reverted by deleting `taking`
//     from LiveOutputRail (the `if (taking) return`, the two assignments and the
//     `disabled`/label expressions). Both go RED.
//   · "a take that answers still clears the guard" — stays GREEN either way. It is
//     here to catch the opposite mistake: a flag that is never cleared is a Go Live
//     button that stops working mid-service, which is worse than a double press.
//   · "the shipping handler does not swallow" — reverted by restoring the
//     `try { … } catch (e) { errMsg = humanError(e); }` around `fireQueued`'s two
//     awaits. It goes RED.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const cap = await import('./stores/capture.js');

/**
 * Let the view's mount actually finish.
 *
 * MACROTASKS, not microtasks. `capture.js` reaches the bridge through a lazy
 * `await import('@tauri-apps/api/core')`, and a module import does not resolve on a
 * microtask drain however many times you await `Promise.resolve()` — so a loop of
 * those leaves every wrapper reading `undefined.invoke` and the pane renders as
 * though the engine were missing. Same shape as `screencards.test.js`.
 */
async function settle(n = 12) {
  for (let i = 0; i < n; i += 1) {
    await new Promise((r) => setTimeout(r, 0));
    await tick();
  }
}

const A = { reference: 'John 3:16', text: 'For God so loved the world' };
const B = { reference: 'Romans 8:28', text: 'All things work together' };

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
});

let host;
let app;

async function mountRail(props) {
  const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new LiveOutputRail({ target: host, props });
  await tick();
  return host;
}

const goLive = (el) =>
  [...el.querySelectorAll('button')].find((b) => /Go Live|Sending/.test(b.textContent));

describe('Go Live cannot be pressed twice while the first take is in flight', () => {
  it('a second press reaches no fire path', async () => {
    let calls = 0;
    // A take that has been asked for and has not answered — the whole subject.
    const el = await mountRail({
      queue: [A, B],
      onQueueChange: () => {},
      onFireQueued: () => {
        calls += 1;
        return new Promise(() => {});
      },
    });

    goLive(el).click();
    await tick();
    expect(calls).toBe(1);

    goLive(el).click();
    goLive(el).click();
    await tick();
    expect(calls).toBe(1);
  });

  it('says so on its face — disabled, and the label is not still an offer', async () => {
    const el = await mountRail({
      queue: [A],
      onQueueChange: () => {},
      onFireQueued: () => new Promise(() => {}),
    });

    expect(goLive(el).disabled).toBe(false);
    goLive(el).click();
    await tick();

    expect(goLive(el).disabled).toBe(true);
    // Not "Go Live — John 3:16". A control that still reads as an offer while it
    // is mid-flight is the reason the operator presses it again (rule 35).
    expect(goLive(el).textContent.trim()).toBe('Sending…');
  });

  it('a take that ANSWERS clears the guard — the button works again', async () => {
    let queue = [A, B];
    const el = await mountRail({
      queue,
      onQueueChange: (q) => {
        queue = q;
        app.$set({ queue });
      },
      onFireQueued: () => Promise.resolve(),
    });

    goLive(el).click();
    await tick();
    await tick();
    expect(goLive(el).disabled).toBe(false);
    expect(queue.map((q) => q.reference)).toEqual(['Romans 8:28']);
  });
});

describe('the handler the shipping app actually passes lets a failure through', () => {
  it('Library.svelte::fireQueued does not swallow its own rejection', () => {
    // A SOURCE assertion, and it is honest about being one: the behavioural half
    // already exists in `liveoutputrail.test.js`, and what it cannot see is which
    // handler the app hands the rail. Driving that from a mounted `Library.svelte`
    // would mean populating its private queue through six panes of UI; the fact
    // being pinned is one line long and this reads it directly.
    const src = readFileSync(join(process.cwd(), 'src/lib/views/Library.svelte'), 'utf8');
    const body = src.slice(src.indexOf('async function fireQueued'));
    const fn = body.slice(0, body.indexOf('\n  }') + 4);

    expect(fn, 'fireQueued must exist and be found by this scanner').toMatch(/manualFire/);
    expect(fn, 'fireQueued must let the rail see the failure — see the header').not.toMatch(
      /catch/,
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE SAME QUESTION ON THE OTHER SURFACE THAT CREATES SOMETHING.
//
// `Channels.add()` cleared `newName` only AFTER the await, and Enter in the name
// box calls it as well as the button — so a held Enter, or a double click on a
// slow write, added the same screen twice. Two output channels with one name is
// not a cosmetic mess: every surface that picks a screen by name (the chrome
// lamps, the Live status pane, Copy URL) then has two rows it cannot tell apart,
// and one of them carries a template assignment nobody made on purpose.
//
// Watched to fail by deleting `adding` from `Channels.svelte` — `add_channel` is
// then called three times and the button still reads "Add".
// ─────────────────────────────────────────────────────────────────────────────
describe('Outputs → Add Screen cannot be pressed twice', () => {
  it('a second press writes no second screen', async () => {
    let adds = 0;
    invoke.mockImplementation((cmd) => {
      if (cmd === 'add_channel') {
        adds += 1;
        return new Promise(() => {}); // asked, not yet answered
      }
      return Promise.resolve([]);
    });

    // Every control on this pane is `disabled={!$capture.available}` — a screen
    // cannot be added with no engine behind it — so the subject only exists once
    // the bridge is attached.
    cap.capture.update((c) => ({ ...c, available: true }));

    const Channels = (await import('./views/Channels.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Channels({ target: host, props: {} });
    await settle();

    const press = (label) => {
      const b = [...host.querySelectorAll('button')].find((x) =>
        x.textContent.replace(/\s+/g, ' ').trim().startsWith(label),
      );
      if (!b) throw new Error(`no control labelled "${label}"`);
      b.click();
    };

    press('＋ Add Screen');
    await settle();

    const name = host.querySelector('input[placeholder="New screen name"]');
    expect(name).toBeTruthy();
    name.value = 'Lobby TV';
    name.dispatchEvent(new Event('input'));
    await settle();

    press('Add');
    await settle();
    expect(adds).toBe(1);

    press('Add');
    press('Add');
    await settle();
    expect(adds).toBe(1);

    // And it says so rather than still reading as an offer (rule 35).
    const add = [...host.querySelectorAll('button')].find((x) =>
      /^(Add|Adding…)$/.test(x.textContent.trim()),
    );
    expect(add.textContent.trim()).toBe('Adding…');
    expect(add.disabled).toBe(true);
  });
});
