// LIBRARY'S SLIDE ORDINAL IS PRINTED THE WAY LIVE PRINTS ITS OWN.
//
// Neither surface shows a VERSE number. Both show a position in a deck, and
// Library's is decoupled from the verse number on purpose — `Browse.svelte`
// renumbers the deck by index because a verse number "drifts the moment
// anything is inserted, filtered or sorted, and then two slides on screen wear
// the same number". That decision is untouched here.
//
// What was inconsistent was only the TYPESETTING. Live's stage-guide list prints
// `String(c.n).padStart(2, '0')` (`Live.svelte`, `.sg-n`), as do the Planner's
// slide index (`ServicePlanner.svelte`, `.sp-slideidx`) and the import review
// (`ImportReview.svelte`, `.ir-idx`) — three surfaces, one house style, all in
// `r-mono`. Library's two print sites emitted a bare `{v.slideNo}`, so the same
// operator reading the same kind of number saw `1` on one desk and `01` on the
// next, and a mono column of 1/2/…/10 does not align while a column of 01/02/…/10
// does. The operator's instruction was explicit: match the existing Live
// rendering rather than invent a second style.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Test the bug, not the fix. Every case below was watched to go RED by reverting
// `VerseDeck.svelte`'s two spans to `{v.slideNo}`:
//
//   · "the grid card footer" / "the list row" — each prints `01`, and the
//     assertion is on the exact string, so the un-padded `1` fails it.
//   · "two digits are left alone" — stays GREEN both ways. It is here to catch
//     the opposite mistake: a pad that truncated or re-padded a number already
//     two digits wide would be a worse bug, and nothing else would have said so.
//   · "Live is the source of the style" — reads both components off disk and
//     asserts they agree. It is what makes this a claim about CONSISTENCY rather
//     than a claim about the literal string `01`; if Live ever restyles, this
//     fails and sends the next person to both files instead of one.

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const read = (rel) => readFileSync(fileURLToPath(new URL(rel, import.meta.url)), 'utf8');

const ONE = { reference: 'John 3:16', label: 'John 3:16', text: 'For God so loved', slideNo: 1 };
const TEN = { reference: 'John 3:25', label: 'John 3:25', text: 'A tenth slide', slideNo: 10 };

let host;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
});

async function mount(props) {
  const VerseDeck = (await import('./views/library/VerseDeck.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  new VerseDeck({ target: host, props: { items: [ONE, TEN], ...props } });
  await tick();
  return host;
}

/** Every ordinal the deck printed, in document order. */
const ordinals = (el) => [...el.querySelectorAll('.vd-n')].map((n) => n.textContent.trim());

describe('the Library deck prints its slide ordinal the way Live does', () => {
  it('pads a single digit on the grid card footer', async () => {
    const el = await mount({ layout: 'grid' });
    expect(ordinals(el)[0]).toBe('01');
  });

  it('pads a single digit on the list row', async () => {
    const el = await mount({ layout: 'list' });
    expect(ordinals(el)[0]).toBe('01');
  });

  it('leaves a two-digit ordinal alone on both layouts', async () => {
    for (const layout of ['grid', 'list']) {
      const el = await mount({ layout });
      expect(ordinals(el)).toContain('10');
      host.remove();
    }
  });

  it('is the same expression Live uses, so the two cannot drift apart', () => {
    // Live's stage-guide ordinal. If this stops matching, Live restyled and
    // Library has to follow — which is the whole point of pinning it here.
    const live = read('./views/Live.svelte');
    expect(live).toContain("String(c.n).padStart(2, '0')");

    const deck = read('./views/library/VerseDeck.svelte');
    const padded = deck.match(/String\(v\.slideNo\)\.padStart\(2, '0'\)/g) || [];
    expect(padded).toHaveLength(2); // the grid footer and the list row

    // And no print site was left behind un-padded.
    expect(deck).not.toMatch(/>\{v\.slideNo\}</);
  });
});
