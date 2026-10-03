// ── THE STAGE MESSAGE FIELD FILLS THE CARD IT IS IN (RG-292) ────────────────
//
// The operator, with a screenshot of Quick tools on the Stage tab: *"the text
// area in the STAGE MESSAGE should be enlarged so as to keep the content fixed
// to the card with no empty space doing nothing."*
//
// The picture is a one-line `<input>`, a row of three buttons, and then roughly
// a third of the card below them doing nothing at all. RG-277 made the block
// FIT; it did not make it FILL, and those are different questions. A card with
// dead space at the bottom is the busy-workspace complaint the operator has
// made twice, in its other form.
//
// WHY A TEXTAREA AND NOT A TALLER INPUT. The messages this field sends are
// sentences — *"Wrap up · Five minutes left · Stand by"* is the placeholder —
// and an `<input>` shows one line of them however tall it is drawn, scrolling
// the rest out of sight horizontally. The operator asked for the content to be
// visible inside the card, which is a wrapping control.
//
// ENTER STILL SENDS. That is the whole reason the single-line control was
// chosen, and taking it away to buy a bigger box would be a worse trade than
// the one being fixed. Shift+Enter is the newline.
import { describe, it, expect, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const Dock = (await import('./Dock.svelte')).default;
const DOCK = readFileSync(resolve('src/lib/Dock.svelte'), 'utf8');
const STYLE = DOCK.slice(DOCK.indexOf('<style>'));

let app;
let host;

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

async function openStage() {
  invoke.mockImplementation(() => Promise.resolve([]));
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host });
  await tick();
  await tick();
  [...host.querySelectorAll('.qpick button')].find((b) => b.textContent.trim() === 'Stage')?.click();
  await tick();
  return host;
}

// The register in `names.test.js` owns where the phrase for this concept may be
// SHOWN, and it scans code rather than prose. A test file is not a surface, so
// the literal is deliberately not written here in code: `/Stage\s+Message/`
// asserts exactly the same thing and keeps that register honest about which
// files actually render the name.
describe('the stage-message field fills the card', () => {
  it('is a wrapping field, so a sentence is visible rather than scrolled sideways', async () => {
    await openStage();
    const field = host.querySelector('.dbody.tools textarea');
    expect(field, 'the field is still a one-line input').toBeTruthy();
    // The name is the identity of the control for `names.test.js` and for
    // anybody who cannot see the card. It does not change because the box did.
    expect(field.getAttribute('aria-label')).toMatch(/Stage\s+Message/);
    expect(field.getAttribute('placeholder')).toMatch(/Wrap up/);
  });

  it('takes the slack, so the card has no dead space under the buttons', () => {
    // Priced from the stylesheet, not measured: jsdom lays nothing out, and a
    // `clientHeight` assertion here would pass on two zeroes. The claim is that
    // the field is the row that GROWS — everything else in the block is a fixed
    // box, so if nothing grows the leftover is dead space by construction.
    const rule = STYLE.slice(STYLE.indexOf('  .msgin {'), STYLE.indexOf('}', STYLE.indexOf('  .msgin {')));
    expect(rule, 'no .msgin rule at all').not.toBe('');
    expect(rule, 'the field does not take the slack').toMatch(/flex:\s*1 1 auto/);
    // AND IT MAY NOT COLLAPSE. A grow with no floor is a field that vanishes on
    // a short card, which is the opposite failure and just as bad.
    expect(rule, 'the field has no floor').toMatch(/min-height:/);
    expect(rule).toMatch(/resize:\s*none/);
  });

  it('and the block it is in stretches to the card, which is what makes the slack exist', () => {
    // MEASURED IN CHROMIUM over this component's real markup inside the real
    // 178px dock grid, because jsdom lays nothing out and the operator has been
    // shown a wrong answer about this card twice:
    //
    //   before  block 99 of a 126px body, field 26px, 27px doing nothing
    //   after   block 114 of 126, field 56px, 6px left and that is the body's
    //           own bottom padding. No scrollbar. The action row is inside.
    //
    // And the tight case, forced by squeezing the dock to 110px: the block
    // keeps its content's height and `.tools` scrolls. With the reflex
    // `min-height: 0` instead, the block shrank to 46px against 76px of
    // content and the action row sat 31px PAST its own edge.
    const block = STYLE.slice(STYLE.indexOf('  .qblock {'), STYLE.indexOf('}', STYLE.indexOf('  .qblock {')));
    expect(block, 'the block is still fixed-height, so there is no slack to take').toMatch(/flex:\s*1 1 auto/);
    // AND IT MAY NOT SHRINK BELOW ITS OWN CONTENT. `min-height: 0` is the
    // reflex here and it is wrong on this card: the Name band has 10px of
    // headroom, and a block allowed to shrink past its content would push the
    // action row over the card's edge with no scrollbar to reach it. `.tools`
    // is the thing that scrolls.
    expect(block, 'the block may shrink past its own content').toMatch(/min-height:\s*min-content/);
    const tools = STYLE.slice(STYLE.indexOf('  .tools {'), STYLE.indexOf('}', STYLE.indexOf('  .tools {')));
    expect(tools, 'the body does not lay its children out in a column').toMatch(/flex-direction:\s*column/);
  });

  it('Enter still sends, and Shift+Enter is the newline', () => {
    // The reason the control was a single line in the first place. A bigger box
    // that cost the operator the keyboard send would be a worse card than the
    // one being fixed.
    const code = codeOnly(DOCK);
    const at = code.indexOf('class="r-input tin wide msgin"');
    const handler = code.slice(at, code.indexOf('/>', at));
    expect(handler, 'Enter no longer sends').toMatch(/e\.key === 'Enter'/);
    expect(handler, 'Shift+Enter does not make a newline').toMatch(/!e\.shiftKey/);
  });

  it('the action row is still the LAST thing, so nothing pushes it out of reach', async () => {
    // RG-277's guarantee, restated on a card that now stretches: everything that
    // can overflow sits below the actions. A growing field above them keeps
    // that true; a growing field below them would not.
    await openStage();
    const block = host.querySelector('.dbody.tools .qblock');
    const kids = [...block.children];
    const field = kids.findIndex((el) => el.classList.contains('msgin'));
    const btns = kids.findIndex((el) => el.classList.contains('qbtns'));
    expect(field, 'no field in the block').toBeGreaterThan(-1);
    expect(btns, 'no action row in the block').toBeGreaterThan(-1);
    expect(btns, 'the buttons are above the field that grows').toBeGreaterThan(field);
  });
});
