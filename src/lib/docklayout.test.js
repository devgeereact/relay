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
import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Dock from './Dock.svelte';
import { templates } from './stores/capture.js';

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

  // THE PICKER MOVED TO THE HEAD (RG-270, operator instruction 2026-09-23):
  // *"I want you to move Stage | Name to the same bar as QUICK TOOLS where Load
  // whole plan was before"*. `Load whole plan` vacated that slot at RG-261, and
  // the body is for whichever job is chosen, which is the room the request is
  // about. The case asserts BOTH directions — in the head AND not in the body —
  // because a picker duplicated into the head would satisfy the first alone.
  it('and the picker sits in the card’s head, in the slot Load whole plan left', async () => {
    await mount();
    const card = [...host.querySelectorAll('.dpanel')].find(
      (p) => p.querySelector('.dk')?.textContent.trim() === 'Quick tools',
    );
    expect(card, 'no Quick tools card').toBeTruthy();
    expect(card.querySelector('.dhead .qpick'), 'the picker is not in the head').toBeTruthy();
    expect(card.querySelector('.dbody .qpick'), 'the picker is still in the body').toBeNull();
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

// ── QUICK TOOLS FITS IN ITS CARD (RG-277) ───────────────────────────────────
//
// The operator, from a screenshot of the running app: *"clean up QUICK TOOLS and
// make everything fit in the card... both the stage and the name"*. The Name
// band ran its button row about 17px BELOW the card's bottom edge at every desk
// width, because `.ltpick` carried an `auto` flex basis against `.r-select`'s
// own `width: 100%` and so could never share a line with the label beside it:
// `.qhead` wrapped, and a 44px head went where a 26px one was drawn.
//
// **jsdom does not lay anything out, and this test says so rather than
// pretending.** `clientHeight` and `scrollHeight` are 0 on every element here,
// so the measurement the defect was found with is not available - the first
// case below asserts that emptiness, so nobody later reads these numbers as
// rendered ones and nobody writes a `scrollHeight <= clientHeight` check that
// passes because both are zero.
//
// So the card is PRICED instead. The rows come from the real mounted DOM, one
// tab at a time, and each row's height comes from the stylesheet that actually
// declares it - `src/app.css` for the shared controls, `Dock.svelte`'s own
// `<style>` for the paddings and gaps. A row the pricer does not recognise is a
// THROWN ERROR and never a zero, which is the whole anti-vacuity guard: adding
// a fourth row to the Stage Message fails this test rather than slipping past
// it. The arithmetic agrees with Chromium, which was driven over the same
// markup at 1024, 1280 and 1440 while this was written: Stage 84 of 136, Name
// 126 of 136.
describe('Quick tools fits inside a 178px card, on both tabs', () => {
  const APP = readFileSync(resolve('src/app.css'), 'utf8');

  // A band is a template whose SHAPE is a lower third; `templateKind` derives
  // it. Without one the Name band renders its (smaller) empty state, and this
  // describe would price the easy half of the card - so one is seeded and the
  // case below asserts the real fields arrived.
  const BAND = {
    id: 31,
    name: 'Name — Classic',
    layout: {
      layers: [
        { id: 'b', type: 'band', members: ['n', 'r'] },
        { id: 'n', type: 'text', bind: 'verse', name: 'Name' },
        { id: 'r', type: 'text', bind: 'reference', name: 'Role' },
      ],
    },
    style: {},
  };
  beforeEach(() => templates.set([BAND]));
  afterEach(() => templates.set([]));

  /** One declaration, from whichever rule with this selector declares it.
      A selector can appear more than once - `.dhead` is written twice, the
      first only to say that it wraps - so this scans them all and throws if
      none of them carries the property rather than reading the first. */
  const decl = (css, sel, prop) => {
    const want = new RegExp('(?:^|[^-\\w])' + prop + ':\\s*([^;}]+)');
    // Both spellings: this file writes `.sel {` and `src/app.css` writes `.sel{`.
    const open = new RegExp(sel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\s*\\{', 'g');
    let seen = false;
    for (const hit of css.matchAll(open)) {
      seen = true;
      const at = hit.index;
      const m = css.slice(at, css.indexOf('}', at)).match(want);
      if (m) return m[1].trim();
    }
    throw new Error(seen ? `${sel} declares no ${prop}` : `no rule for ${sel}`);
  };
  const px = (v) => parseFloat(v);
  /** The first length in a shorthand, and the pair for a `a b` padding. */
  const pad = (v) => {
    const parts = v.split(/\s+/).map(parseFloat);
    return parts.length === 1 ? [parts[0], parts[0]] : [parts[0], parts[0]];
  };

  // The shared controls own their boxes (B1), so their heights are read from
  // `src/app.css` and a change there moves this budget rather than breaking it
  // silently.
  const INPUT_H = px(decl(APP, '.r-input, .r-select', 'height'));
  const BTN_SM_H = px(decl(APP, '.r-btn.sm', 'height'));
  // `.r-lbl` declares no line-height, so this is the rendered line box for
  // `--v-fs-cap` at the browser default - measured at 14px in Chromium over
  // this exact markup. It is the LARGER of the two candidates (the token's own
  // `--v-lh-cap` is 13px), so the budget stays the conservative one.
  const LBL_H = 14;
  const CAP_H = 14;

  const STYLE = DOCK.slice(DOCK.indexOf('<style>'));
  const DOCK_H = px(decl(STYLE, '.dock', 'height'));
  // The head: `min-height` and padding, plus its 1px bottom rule. The picker
  // inside it is 22px (`.qp`'s 18px inside `.qpick`'s 1px padding and 1px
  // border), which is taller than the 20px the min-height leaves, so the head
  // is the picker's height and not the declared minimum.
  const HEAD_PAD = pad(decl(STYLE, '.dhead', 'padding'))[0];
  const PICKER_H = px(decl(STYLE, '.qp', 'min-height')) + 2 + 2;
  const HEAD_H = Math.max(px(decl(STYLE, '.dhead', 'min-height')), PICKER_H + HEAD_PAD * 2) + 1;
  const BODY_PAD = pad(decl(STYLE, '.tools', 'padding'))[0];
  const BLOCK_PAD = pad(decl(STYLE, '.qblock', 'padding'))[0];
  const BLOCK_GAP = px(decl(STYLE, '.qblock', 'gap'));
  const TOOLS_GAP = px(decl(STYLE, '.tools', 'gap'));
  const BUDGET = DOCK_H - HEAD_H - BODY_PAD * 2;
  const LTPICK = STYLE.slice(STYLE.indexOf('  .ltpick {'), STYLE.indexOf('}', STYLE.indexOf('  .ltpick {')));
  const HEAD_IS_ONE_ROW = /flex:\s*1 1 0/.test(LTPICK);

  /** What one row in a `.qblock` costs, or a thrown error if it is unknown. */
  function priceRow(el) {
    const cls = el.classList;
    if (cls.contains('qhead')) {
      const kids = [...el.children].map(priceRow);
      // ONE LINE ONLY WHEN THE BAND PICKER HAS A ZERO BASIS. `.r-select` carries
      // `width: 100%`, so an `auto` basis resolves to the whole head and
      // `.qhead`'s own `flex-wrap` puts the select on a line of its own. The
      // pricer models that rather than assuming the good case, which is what
      // makes reverting `.ltpick` fail the two budget cases below and not only
      // the one that names it.
      const wraps = !!el.querySelector('.ltpick') && !HEAD_IS_ONE_ROW;
      if (!wraps) return Math.max(...kids, 0);
      return kids.reduce((a, b) => a + b, 0) + px(decl(STYLE, '.qhead', 'row-gap'));
    }
    if (cls.contains('r-lbl') || cls.contains('qbadge')) return LBL_H;
    if (cls.contains('qspring')) return 0;
    if (cls.contains('r-input') || cls.contains('r-select')) return INPUT_H;
    if (cls.contains('qbtns')) return BTN_SM_H;
    if (cls.contains('qcap')) return CAP_H;
    throw new Error(
      `nothing prices <${el.tagName.toLowerCase()} class="${el.className}"> - ` +
        'a new row in Quick tools must be priced here, not assumed free',
    );
  }

  /** Everything in the block ABOVE the preview, which is what has to fit. */
  function priceBlock(block) {
    const rows = [...block.children].filter((el) => !el.classList.contains('ltprev'));
    const upTo = rows.findIndex((el) => el.classList.contains('ltprev'));
    const kept = upTo < 0 ? rows : rows.slice(0, upTo);
    const sum = kept.reduce((a, el) => a + priceRow(el), 0);
    // `+ 2` is the block's own 1px hairline, top and bottom. The trailing
    // margin is priced too rather than assumed gone - it was 6px of a 137px
    // body, and a pricer that ignored it would report a card that fits over a
    // card that does not.
    const margin = /margin-bottom:\s*([\d.]+)px/.exec(
      STYLE.slice(STYLE.indexOf('  .qblock {'), STYLE.indexOf('}', STYLE.indexOf('  .qblock {'))),
    );
    return (
      sum +
      BLOCK_GAP * Math.max(0, kept.length - 1) +
      BLOCK_PAD * 2 +
      2 +
      (margin ? parseFloat(margin[1]) : 0)
    );
  }

  async function show(tab) {
    [...host.querySelectorAll('.qpick button')].find((b) => b.textContent.trim() === tab)?.click();
    await tick();
    return host.querySelector('.dbody.tools .qblock');
  }

  it('cannot be measured in jsdom, which is why it is priced', async () => {
    await mount();
    const body = host.querySelector('.dbody.tools');
    expect(body, 'no Quick tools body at all').toBeTruthy();
    // If this ever stops being 0, jsdom grew a layout engine and this whole
    // describe should be rewritten to measure rather than to price.
    expect(body.clientHeight, 'jsdom laid something out').toBe(0);
    expect(body.scrollHeight, 'jsdom laid something out').toBe(0);
  });

  it('prices the Stage Message inside the card, with room to spare', async () => {
    await mount();
    const block = await show('Stage');
    expect(host.textContent).toContain('Stage Message');
    const h = priceBlock(block);
    expect(h, `the Stage Message costs ${h}px of a ${BUDGET}px body`).toBeLessThanOrEqual(BUDGET);
  });

  it('prices the Name band inside the card, preview closed', async () => {
    await mount();
    const block = await show('Name');
    expect(host.textContent).toContain('Name band');
    // The empty state renders instead when no lower third is seeded, and that
    // is a smaller block - so the case would pass having checked the easy one.
    // It is asserted rather than assumed.
    expect(block.querySelector('[aria-label="Name for the lower third"]'), 'the Name band rendered its empty state, so this priced nothing')
      .toBeTruthy();
    const h = priceBlock(block);
    expect(h, `the Name band costs ${h}px of a ${BUDGET}px body`).toBeLessThanOrEqual(BUDGET);
  });

  it('the Name band head is ONE row, which is the 18px the fix bought', async () => {
    // `.r-select` carries `width: 100%`, so an `auto` basis resolves to the
    // whole head and the select can never sit beside its label. This is the
    // declaration that stops that, and reverting it puts the 44px head back.
    expect(HEAD_IS_ONE_ROW, '.ltpick went back to an auto basis and the head will wrap again').toBe(true);

    await mount();
    const head = (await show('Name')).querySelector('.qhead');
    expect(head.querySelector('.r-lbl').textContent.trim()).toBe('Name band');
    expect(head.querySelector('.ltpick'), 'the band picker left the head').toBeTruthy();
  });

  it('the block carries no margin under it, now one job renders at a time', async () => {
    // `margin-bottom: 6px` spaced the block from a sibling that stopped being
    // rendered at RG-258, and `.tools`' own gap is what separates siblings if a
    // second block ever comes back.
    const rule = STYLE.slice(STYLE.indexOf('  .qblock {'), STYLE.indexOf('}', STYLE.indexOf('  .qblock {')));
    expect(rule, 'the trailing margin is back').not.toMatch(/margin-bottom/);
    expect(TOOLS_GAP, '.tools lost the gap that would separate two blocks').toBeGreaterThan(0);
  });

  it('everything that can overflow sits BELOW the action row', async () => {
    // The 16:9 preview genuinely cannot fit: it is 135px at its cap against
    // about 10px of headroom. So the rule is not that it fits, it is where it
    // is - after the buttons, so opening it scrolls the preview into view and
    // never pushes `To programme` out of reach. The error line is a sibling
    // below the block for the same reason.
    await mount();
    let block = await show('Name');
    // THE PREVIEW IS OPENED FOR REAL. It is behind `{#if ltPreview && ltReady}`
    // and starts closed, so a case that only looked at the default render would
    // find no `.ltprev` at all and pass having checked nothing - which is the
    // shape of vacuity this file has been bitten by before.
    const name = block.querySelector('[aria-label="Name for the lower third"]');
    name.value = 'Pastor Ade';
    name.dispatchEvent(new Event('input'));
    await tick();
    const open = [...block.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Preview');
    expect(open, 'no Preview control').toBeTruthy();
    expect(open.disabled, 'Preview is still disabled, so nothing opened').toBe(false);
    open.click();
    await tick();
    block = host.querySelector('.dbody.tools .qblock');
    const kids = [...block.children];
    const btns = kids.findIndex((el) => el.classList.contains('qbtns'));
    expect(btns, 'no button row in the Name band').toBeGreaterThanOrEqual(0);
    for (const later of ['ltprev', 'qcap']) {
      const at = kids.findIndex((el) => el.classList.contains(later));
      expect(at, `.${later} did not render, so its position was never checked`).toBeGreaterThanOrEqual(0);
      expect(at, `.${later} is above the action row`).toBeGreaterThan(btns);
    }
    // And the preview is capped by WIDTH, so the cap cannot squash a 16:9 box.
    const prev = STYLE.slice(STYLE.indexOf('  .ltprev {'), STYLE.indexOf('}', STYLE.indexOf('  .ltprev {')));
    expect(prev).toMatch(/max-width:/);
    expect(prev, 'a max-height against aspect-ratio squashes the preview').not.toMatch(/max-height:/);
  });
});
