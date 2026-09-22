// THE DOCK AS THE APPROVED CANVAS DRAWS IT (RG-258).
//
// The operator: *"I want you to rework all this section keeping its relevance
// but a better way to have all of it clean format"*, then *"Do what's exactly on
// the artifacts"*. Two cards change; the other two were already what the drawing
// shows once RG-254 and RG-257 landed.
//
// **Quick tools does ONE JOB AT A TIME.** It carried the Name band and the Stage
// Message stacked, each getting half a 178px card: two labels, three inputs,
// five buttons and an optional preview in the space one of them needs. The
// drawing puts a segmented picker in the head — the slot the card already uses
// for a picker — and gives whichever job is chosen the whole body. Nothing is
// removed and nothing moves workspace; the card stops asking an operator to read
// two instruments to find one.
//
// **Controls goes back to the drawing's three rows.** `End service` and
// `Rehearse` share the top row, `Blackout` takes the second, `Clear screens` the
// third and widest. Rule 15 is untouched and asserted here as well as in
// `panic.test.js`: the card never scrolls, and the panic control is last, full
// width, and never behind an overflow edge.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Dock from './Dock.svelte';

const DOCK = readFileSync(resolve('src/lib/Dock.svelte'), 'utf8');

let app;
let host;

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host });
  await tick();
  await tick();
  return host;
}

const text = () => host.textContent ?? '';
const button = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === label);

describe('Quick tools does one job at a time', () => {
  it('opens on the Stage Message, which is the one that changes mid-sermon', async () => {
    await mount();
    expect(host.querySelector('.qpick'), 'no picker in the head').toBeTruthy();
    expect(text(), 'the Stage Message is not the one showing').toContain('Stage Message');
    expect(text(), 'both jobs are showing at once').not.toContain('Name band');
  });

  it('and the picker swaps which one has the card', async () => {
    await mount();
    button('Name')?.click();
    await tick();
    expect(text()).toContain('Name band');
    expect(text(), 'the message stayed behind').not.toContain('Stage Message');

    button('Stage')?.click();
    await tick();
    expect(text()).toContain('Stage Message');
  });

  it('the picker says which one is chosen, for somebody who cannot see it', async () => {
    await mount();
    const chosen = [...host.querySelectorAll('.qpick button')].filter(
      (b) => b.getAttribute('aria-pressed') === 'true',
    );
    // EXACTLY ONE. Two pressed is a picker that has lost track of itself, and
    // none is a picker that never answered.
    expect(chosen).toHaveLength(1);
    expect(chosen[0].textContent.trim()).toBe('Stage');
  });

  it('nothing was dropped on the way — both jobs are still reachable', async () => {
    // THE REWORK IS A LAYOUT, NOT A CULL. The operator asked to keep the
    // relevance; a tidier card that quietly lost the Name band would be the
    // worse outcome, and this is the case that would catch it.
    await mount();
    expect(text()).toContain('Send to stage');
    expect(text()).toContain('Alert');
    button('Name')?.click();
    await tick();
    // THE BLOCK, not its primary button: with no lower third in the library the
    // Name band correctly renders its empty state instead, and requiring
    // `To programme` here would be asserting the fixture rather than the card.
    expect(text()).toContain('Name band');
    expect(
      text().includes('To programme') || text().includes('No lower third yet'),
      'the Name band rendered neither its controls nor its empty state',
    ).toBe(true);
  });
});

describe('Controls is the drawing’s three rows', () => {
  it('End service and Rehearse share the top row', () => {
    // Neither is `wide`, so the two-column grid puts them side by side — which
    // is what gives the two rows below their full width.
    const endsvc = DOCK.match(/class="r-cbtn endsvc[^"]*"/)?.[0] ?? '';
    expect(endsvc, 'End service still spans the card').not.toContain('wide');
  });

  it('Blackout takes a row of its own', () => {
    expect(DOCK).toMatch(/class="r-cbtn black wide"/);
  });

  it('and Clear screens is last, full width, and never behind an edge', async () => {
    // RULE 15, asserted here as well as in `panic.test.js` because this card was
    // rearranged. A panic control that can be scrolled out of reach is one that
    // will be, exactly once, on a Sunday.
    expect(DOCK).toMatch(/class="r-cbtn danger wide"/);
    const ctl = DOCK.slice(DOCK.indexOf('<div class="r-ctl">'), DOCK.indexOf('</div>\n    </div>\n  </div>\n</section>'));
    const order = [...ctl.matchAll(/class="r-cbtn ([a-z]+)/g)].map((m) => m[1]);
    expect(order, `the order is ${order.join(' · ')}`).toEqual(['endsvc', 'rehearse', 'black', 'danger']);

    await mount();
    const card = host.querySelector('.ctlbody');
    expect(card, 'no controls card').toBeTruthy();
    expect(getComputedStyle(card).overflow, 'the panic card can scroll').not.toBe('auto');
  });
});
