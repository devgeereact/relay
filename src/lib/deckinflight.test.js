// A CARD THAT IS ALREADY ON ITS WAY TO A SCREEN DOES NOT ANSWER A SECOND PRESS.
//
// `VerseDeck` is the ONE deck component and five Library panes render it —
// Announcements, Scripture, Browse, LyricsPane, MediaLibrary. Every one of them
// already sets `busyRef` before it awaits the fire and clears it afterwards, and
// the grid card already WORE that fact as a "Sending…" badge.
//
// None of the three press paths consulted it. The grid button, the list row (a
// `role="button"` div, which cannot be `disabled` at all) and the kebab's "Take to
// screen" all answered the second press, so an operator pressing twice on a slow
// fire sent the same verse twice: two broadcasts, and two `manual_fire` rows for a
// router that calibrates itself from that column (CLAUDE.md rule 14).
//
// The guard is at the choke point — `VerseDeck.fire()` — for the reason rule 36
// gives: a check added at three doors is a check that will be missing from the
// fourth. `disabled` on the two real buttons is the visible half.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Test the bug, not the fix. Each case below was watched to go RED by reverting
// the guard — restoring `function primary(v) { … else onFire(v); }`, the two
// `disabled={$safeMode}` expressions and the kebab's `onFire(v)`:
//
//   · "the grid card" — the button is enabled and `onFire` is called: both halves fail.
//   · "the list row" — `onFire` is called through the div's own handler, which is
//     the path no `disabled` attribute could ever have covered.
//   · "the kebab" — the menu item is enabled and `onFire` is called.
//   · "a DIFFERENT card still fires" — stays GREEN with the guard reverted. It is
//     here to catch the opposite mistake: a deck-wide freeze during one fire would
//     be a worse bug than the one being fixed, and nothing else would have said so.
//   · "nothing in flight" — stays GREEN both ways, for the same reason: it pins
//     that the guard cannot become a permanent block.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const A = { reference: 'John 3:16', label: 'John 3:16', text: 'For God so loved', slideNo: 1 };
const B = { reference: 'Romans 8:28', label: 'Romans 8:28', text: 'All things work', slideNo: 2 };

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
});

async function mount(props) {
  const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new VerseDeck({
    target: host,
    props: { items: [A, B], layout: 'grid', ...props },
  });
  await tick();
  return host;
}

/** The one control on a card that puts it on a screen, per layout. */
const gridShot = (el, i = 0) => el.querySelectorAll('.vd-shot')[i];
const listArrow = (el, i = 0) =>
  [...el.querySelectorAll('.vd-ic')].filter((b) => b.textContent.trim() === '→')[i];
const listRow = (el, i = 0) => el.querySelectorAll('.vd-row')[i];

describe('a fire that is already in flight does not answer a second press', () => {
  it('the grid card is disabled and reaches no fire path', async () => {
    const onFire = vi.fn();
    const el = await mount({ layout: 'grid', busyRef: 'John 3:16', onFire });

    const shot = gridShot(el, 0);
    expect(shot.disabled).toBe(true);
    // Disabled is the VISIBLE half. The label must say which of the two reasons
    // this is — safe mode and "already sending" are different situations and an
    // operator acts differently on each (rule 35).
    expect(shot.getAttribute('aria-label')).toBe('Sending John 3:16 to the screens');

    shot.click();
    await tick();
    expect(onFire).not.toHaveBeenCalled();
  });

  it('the list row reaches no fire path — the path no `disabled` could cover', async () => {
    const onFire = vi.fn();
    const el = await mount({ layout: 'list', busyRef: 'John 3:16', onFire });

    // The arrow is a real button, so it carries the attribute…
    expect(listArrow(el, 0).disabled).toBe(true);
    // …but the ROW is a `role="button"` div. Clicking it and pressing Enter on it
    // both run `primary()` directly, and that is what the guard has to cover.
    listRow(el, 0).click();
    listRow(el, 0).dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await tick();
    expect(onFire).not.toHaveBeenCalled();
  });

  it('the kebab’s Take to screen is disabled and says why', async () => {
    const onFire = vi.fn();
    const el = await mount({ layout: 'grid', busyRef: 'John 3:16', onFire });

    // Open the first card's menu.
    const kebab = el.querySelector('.vd-kebab') ?? el.querySelector('[aria-haspopup]');
    expect(kebab).toBeTruthy();
    kebab.click();
    await tick();

    const take = [...el.querySelectorAll('.vd-mi')].find((b) =>
      /Take to screen|Sending/.test(b.textContent),
    );
    expect(take).toBeTruthy();
    expect(take.disabled).toBe(true);
    expect(take.textContent.trim()).toBe('Sending…');

    take.click();
    await tick();
    expect(onFire).not.toHaveBeenCalled();
  });

  it('a DIFFERENT card still fires — one verse in flight is not a frozen deck', async () => {
    const onFire = vi.fn();
    const el = await mount({ layout: 'grid', busyRef: 'John 3:16', onFire });

    const other = gridShot(el, 1);
    expect(other.disabled).toBe(false);
    other.click();
    await tick();
    expect(onFire).toHaveBeenCalledTimes(1);
    expect(onFire.mock.calls[0][0].reference).toBe('Romans 8:28');
  });

  it('with nothing in flight the press still fires, in both layouts', async () => {
    for (const layout of ['grid', 'list']) {
      const onFire = vi.fn();
      const el = await mount({ layout, busyRef: '', onFire });
      const ctl = layout === 'grid' ? gridShot(el, 0) : listArrow(el, 0);
      expect(ctl.disabled).toBe(false);
      ctl.click();
      await tick();
      expect(onFire).toHaveBeenCalledTimes(1);
      app.$destroy();
      host.remove();
    }
  });
});
