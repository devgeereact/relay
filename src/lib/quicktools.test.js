// QUICK TOOLS — the three things that change during a service.
//
// `docs/REBRAND.md` §2 names the card's contents: the countdown, the **name
// band**, and the **Stage Message**, with `Load whole plan` in its header.
// (§2 still carries the older wording for that third block; the name that ships
// is the one in `names.test.js`.)
// Relay had the countdown and a single `To preacher` row. The lower thirds and
// the `stage_alert` frame kind already existed (phases 5 and 6, DECISIONS §75 and
// §68) with no operator surface at all, which is the same defect as a command
// with no rendered control — built, shipped and unreachable.
//
// The emergency announcement was a FOURTH thing in this card, moved here from
// Live's inspector column. It was removed on 2026-09-14 on the operator's
// instruction (L3) — it is the one control in the card that paints over live
// scripture on every screen at once, and §2 says three. What that leaves behind
// is asserted below rather than left to be discovered.
//
// WHAT THESE TESTS ARE FOR, in order of how badly each would hurt:
//
//   1. nothing an operator types for the PREACHER may reach a congregation
//      channel. The guarantee is `channels.rs`'s, and this holds the door on
//      this side: the stage row's press reaches `send_stage_alert` and no fire.
//   2. the name band is an ordinary manual fire through an EXISTING path, with
//      the operator's chosen band as the cue's own template — not a new kind, not
//      a new renderer, and never automatic.
//   3. the three tools are ONE instrument — one card, one head, one field shape,
//      one button row — because three degrees of finish in one 200px column is
//      what the operator was actually looking at.
//
//   npx vitest run src/lib/quicktools.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { session, setSession } = await import('./session.js');
const Dock = (await import('./Dock.svelte')).default;
const src = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

// A band is a template whose SHAPE is a lower third — a `band` layer. Nothing is
// flagged; `templateKind` derives it, so a template an operator built themselves
// is offered the moment it has one.
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
const NOT_A_BAND = {
  id: 32,
  name: 'Classic Serif',
  layout: { layers: [{ id: 'v', type: 'text', bind: 'verse' }, { id: 'r', type: 'text', bind: 'reference' }] },
  style: {},
};

let host;
let app;

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host, props: {} });
  return host;
}

async function settle(ms = 10) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);
const byLabel = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === text);

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(null);
  cap.live.set(null);
  cap.stageAlert.set(null);
  cap.capture.update((s) => ({ ...s, available: true }));
  cap.templates.set([BAND, NOT_A_BAND]);
  setSession({ planId: null });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

describe('the name band', () => {
  it('offers the templates whose SHAPE is a lower third, and only those', async () => {
    mount();
    await settle();
    const pick = host.querySelector('[aria-label="Which lower third"]');
    expect(pick).not.toBeNull();
    const options = [...pick.querySelectorAll('option')].map((o) => o.textContent);
    expect(options).toEqual(['Name — Classic']);
  });

  it('says so rather than offering an empty picker when there is no band at all', async () => {
    cap.templates.set([NOT_A_BAND]);
    mount();
    await settle();
    expect(host.textContent).toContain('No lower third yet');
    expect(host.querySelector('[aria-label="Name for the lower third"]')).toBeNull();
  });

  it('sends the NAME as the words and the ROLE as the line beneath it', async () => {
    mount();
    await settle();
    const name = host.querySelector('[aria-label="Name for the lower third"]');
    const role = host.querySelector('[aria-label="Role for the lower third"]');
    name.value = 'Ade Ogunlana';
    name.dispatchEvent(new Event('input'));
    role.value = 'Guest Speaker';
    role.dispatchEvent(new Event('input'));
    await settle();

    byLabel('To programme').click();
    await settle();

    // `fire_content` puts `text` on the band's large line (bound `verse`) and
    // `label` on the small one (bound `reference`) — which is why this reads
    // backwards, and why the mapping is asserted rather than assumed.
    const [, args] = called('fire_content')[0];
    expect(args).toMatchObject({
      label: 'Guest Speaker',
      text: 'Ade Ogunlana',
      kind: 'announce',
      templateId: 31,
    });
  });

  it('cannot fire a band with no name in it', async () => {
    mount();
    await settle();
    expect(byLabel('To programme').disabled).toBe(true);
    byLabel('To programme').click();
    await settle();
    expect(called('fire_content')).toHaveLength(0);
  });

  it('Preview renders it here and reaches no screen', async () => {
    mount();
    await settle();
    const name = host.querySelector('[aria-label="Name for the lower third"]');
    name.value = 'Ade Ogunlana';
    name.dispatchEvent(new Event('input'));
    await settle();

    invoke.mockClear();
    byLabel('Preview').click();
    await settle();
    expect(host.querySelector('.ltprev')).not.toBeNull();
    expect(host.textContent).toContain('preview only — nothing is on a screen');
    // THE WHOLE POINT of the word "preview" on a run surface.
    expect(called('fire_content')).toHaveLength(0);
    expect(called('manual_fire')).toHaveLength(0);
  });
});

describe('the Stage Message', () => {
  it('reaches the stage alert and no fire path at all', async () => {
    mount();
    await settle();
    const box = host.querySelector('[aria-label="Stage Message — stage monitor only"]');
    box.value = 'Wrap up';
    box.dispatchEvent(new Event('input'));
    await settle();
    byLabel('Send to stage').click();
    await settle();

    expect(called('send_stage_alert')).toHaveLength(1);
    // Nothing an operator types for the preacher may reach a congregation
    // channel. The engine-side guarantee is `channels.rs`'s; this is the door.
    expect(called('fire_content')).toHaveLength(0);
    expect(called('manual_fire')).toHaveLength(0);
    expect(called('push_announcement')).toHaveLength(0);
  });

  it('Take down clears it, and is offered only while something is on the stage', async () => {
    mount();
    await settle();
    expect(byLabel('Take down').disabled).toBe(true);
    cap.stageAlert.set('Wrap up');
    await settle();
    expect(byLabel('Take down').disabled).toBe(false);
    byLabel('Take down').click();
    await settle();
    expect(called('send_stage_alert')).toHaveLength(1);
  });
});

// ── THE EMERGENCY ANNOUNCEMENT IS GONE, AND THIS IS THE TEST THAT SAYS SO ──
//
// Removed from Quick tools on the operator's instruction (2026-09-14, L3). Three
// tests used to sit here holding its two-step arm; they are not weakened, they
// are answered by the control not existing. What replaces them is the assertion
// that it is REMOVED rather than hidden — a `hidden` attribute or a `{#if false}`
// would have passed every one of the three tests it replaced.
//
// The wrapper's own contract (`pushAnnouncement` THROWS) is no longer held
// anywhere — the wrapper and the Rust command it called were both DELETED,
// the same precedent as the five commands deleted on 2026-08-30 (a command
// nothing calls is attack surface nobody is watching). `announce.test.js`
// keeps the reasoning in words for a wrapper that no longer exists;
// `qa-r5-groups.test.js` records the deletion itself as a decision. Neither
// still asserts against the wrapper, because there is nothing left to.
describe('the emergency announcement is not in Quick tools', () => {
  it('no control renders it, and nothing in this card can reach the command', async () => {
    mount();
    await settle();
    expect(host.querySelector('[aria-label="Emergency announcement"]')).toBeNull();
    expect(host.textContent).not.toContain('Announce');
    // Not merely hidden. A removed control leaves no state and no handler behind
    // it — a `hidden` attribute would satisfy the query above and still ship the
    // path, which is the distinction the brief asked for.
    expect(src).not.toContain('annMsg');
    expect(src).not.toContain('annArmed');
    expect(src).not.toContain('ann-go');
    // The wrapper is not imported and is called from nowhere. Asserted against
    // the SCRIPT rather than the file, because the file still names it in the
    // comment that explains its absence — and a scanner that reads the prose
    // about a rule instead of the rule is a scanner that passes everything.
    expect(instanceScript()).not.toMatch(/^\s*pushAnnouncement,/m);
    expect(instanceScript()).not.toMatch(/pushAnnouncement\s*\(/);
  });

  it('and pressing every button in Quick tools reaches no announcement', async () => {
    mount();
    await settle();
    invoke.mockClear();
    // Scoped to this card: the Controls card beside it holds the panic buttons,
    // and the audio card's switch opens a microphone.
    for (const b of host.querySelectorAll('.tools button')) if (!b.disabled) b.click();
    await settle();
    expect(called('push_announcement')).toHaveLength(0);
  });
});

describe('Load whole plan', () => {
  it('is disabled, and says why, until the Planner has handed a plan over', async () => {
    mount();
    await settle();
    const btn = byLabel('Load whole plan');
    expect(btn).not.toBeNull();
    expect(btn.disabled).toBe(true);
    expect(btn.getAttribute('title')).toContain('Run in Live');
  });

  it('re-stages the plan the Planner chose, and switches to Live', async () => {
    mount();
    await settle();
    setSession({ planId: 4 });
    await settle();
    const btn = byLabel('Load whole plan');
    expect(btn.disabled).toBe(false);
    btn.click();
    await settle();
    const s = JSON.parse(JSON.stringify(getSession()));
    expect(s.activeTab).toBe('live');
    expect(s.planId).toBe(4);
  });

  // Live's Close plan clears `session.planId`. A button that went dead the moment
  // an operator closed a plan would be useless in exactly the case it exists for:
  // putting the running order back after the preacher went off it.
  it('survives the operator closing the plan on Live', async () => {
    mount();
    await settle();
    setSession({ planId: 4 });
    await settle();
    setSession({ planId: null });
    await settle();
    expect(byLabel('Load whole plan').disabled).toBe(false);
    byLabel('Load whole plan').click();
    await settle();
    expect(getSession().planId).toBe(4);
  });

  // It only ever moves the PLAYHEAD and the grid. What a congregation is looking
  // at is `$live`, and nothing on this path touches it (docs/REBRAND.md §2).
  it('reaches no fire path', async () => {
    mount();
    await settle();
    setSession({ planId: 4 });
    await settle();
    invoke.mockClear();
    byLabel('Load whole plan').click();
    await settle();
    for (const cmd of ['fire_content', 'manual_fire', 'fire_media', 'start_countdown', 'clear_screens']) {
      expect(called(cmd), `Load whole plan reached ${cmd}`).toHaveLength(0);
    }
  });
});

function getSession() {
  let v;
  const un = session.subscribe((s) => (v = s));
  un();
  return v;
}

describe('the card is the three things §2 names, in one place', () => {
  // THREE, not four. §2 says "the three things that change during a service" and
  // the card carried a fourth — the one control in it that paints over live
  // scripture on every screen at once. It was removed on 2026-09-14 (L3) and the
  // card is now what the spec says it is.
  it('holds the countdown, the name band and the Stage Message', () => {
    const card = src.slice(src.indexOf('<span class="dk">Quick tools</span>'));
    const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
    expect(body).toContain('Countdown');
    expect(body).toContain('Name band');
    expect(body).toContain('Stage Message');
    expect(body).toContain('Load whole plan');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// L2 · MATCHED TO THE PROTOTYPE
//
// The dock was rendered beside the prototype's and compared card by card. The
// countdown is a bordered block there, with its own caption row and a format
// picker on the fields; ours was four loose rows in a scrolling column of three
// unrelated tools, with no picker at all. And the audio card's captions carried
// the letter-spacing that capitals need without ever being set as capitals.
// ─────────────────────────────────────────────────────────────────────────────

const { countdownFormat } = await import('./Dock.svelte');

/** The component's own script — `Dock.svelte` now opens with a module one. */
function instanceScript() {
  const open = src.lastIndexOf('<script>');
  return src.slice(open, src.indexOf('</script>', open));
}

describe('L2 · the countdown is one block', () => {
  it('every countdown row is inside the block and the next tool is outside it', () => {
    const from = src.indexOf('<div class="qblock tmr">');
    expect(from).toBeGreaterThan(-1);
    const block = src.slice(from, src.indexOf('<!-- ── THE NAME BAND', from));
    expect(block).toContain('Countdown');
    expect(block).toContain('cdfields');
    expect(block).toContain('cdstate');
    expect(block).toContain('cdtrans');
    // The seam falls BETWEEN the two tools, which is the whole reason for it.
    expect(block).not.toContain('Name band');
    // THE SEAM IS `.qblock`'S NOW, NOT `.tmr`'S (L3). L2 gave the countdown a
    // border of its own and that is exactly how it ended up looking like a
    // different kind of card from its two neighbours — see the next describe.
    expect(rule('.qblock')).toMatch(/border: 1px solid/);
  });

  // THE FIGURE IS NOT AMBER, WHATEVER THE PROTOTYPE DOES. Amber in this room means
  // a congregation is looking at something (rule 18), and this is a READOUT of a
  // countdown rather than the countdown.
  it('the block spends none of the colour law', () => {
    // The boundary was `  .cdstate {`, the row wrapper C2 deleted when the state
    // line moved into the head. A slice to a marker that is not there is
    // `slice(a, -1)` — the whole rest of the file, which is a scan that passes
    // for the wrong reason. Asserted, so the next removal cannot do it silently.
    const from = src.indexOf('  .tmr {');
    const to = src.indexOf('  .cdstatev {');
    expect(from, 'the countdown CSS region no longer starts at .tmr').toBeGreaterThan(-1);
    expect(to, 'the countdown CSS region no longer ends at .cdstatev').toBeGreaterThan(from);
    const css = src.slice(from, to);
    expect(css).not.toMatch(/--v-amber|--v-amethyst|--v-cyan/);
  });

  // AND THE THREE STATES STAY. `not counting` is the half of this panel that tells
  // an operator the big figure is a setting and not a screen.
  it('the state line is untouched', async () => {
    mount();
    await settle();
    expect(host.querySelector('.cdstatev').textContent.trim()).toBe('not counting');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// L3 · ONE INSTRUMENT, THREE TIMES
//
// The operator's words: "fix COUNTDOWN section to look professional and clean
// and same to NAME BAND, [the Stage Message]". All three blocks existed after
// L2 — the defect was that they were three different cards. The countdown sat on
// `--v-surf2` behind `--v-line2` at `--v-r-md` with a `.dcap` caption, a bare
// field row and a wrapping flex of buttons; the name band sat on `--v-surf`
// behind `--v-line` at `--v-r-lg` with an `.r-lbl` caption, fields indented 80px
// under a label that was in the head, and a flex button row; the word to the
// preacher had the third arrangement again.
//
// These tests hold the agreement rather than the literals. A literal-hunting
// scan would condemn the countdown's figure and the alert's red, which are the
// two things that SHOULD differ, and would then be weakened until it held
// nothing.
// ─────────────────────────────────────────────────────────────────────────────

/** One CSS rule out of the component's `<style>`, by selector. */
function rule(sel) {
  const style = src.slice(src.indexOf('<style>'));
  const at = style.indexOf('\n  ' + sel + ' {');
  if (at < 0) return '';
  return style.slice(at, style.indexOf('}', at) + 1);
}

/** The three tool blocks, as they are written in the markup. */
function toolBlocks() {
  const card = src.slice(src.indexOf('<span class="dk">Quick tools</span>'));
  const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
  return [...body.matchAll(/<div class="([^"]*\bqblock\b[^"]*)"/g)].map((m) => m[1]);
}

describe('L3 · the three tools are one card, three times', () => {
  it('every tool is a `qblock`, and none of them draws a second kind of card', () => {
    // Three blocks, each carrying the shared class. `.tmr` and `.onstage` survive
    // as MODIFIERS — one for the tool that needs a figure, one for the tool that
    // turns red — and the test says so rather than forbidding every extra class.
    const blocks = toolBlocks();
    expect(blocks).toHaveLength(3);
    for (const cls of blocks) expect(cls.split(/\s+/)).toContain('qblock');

    // The ground, the hairline, the corner, the padding and the inner gap are
    // declared ONCE. This is the assertion that fails if the countdown gets its
    // own card back: a `.tmr` (or a `.qblock.tmr`) that redeclares any of them.
    const tmr = rule('.tmr');
    for (const prop of ['background', 'border', 'border-radius', 'padding', 'gap']) {
      expect(tmr, `.tmr redeclares ${prop} — it is not the same card as its neighbours`)
        .not.toMatch(new RegExp('(^|[^-\\w])' + prop + ':'));
    }
    const shared = rule('.qblock');
    for (const prop of ['background', 'border', 'border-radius', 'padding', 'gap']) {
      expect(shared, `.qblock does not own ${prop}`).toMatch(new RegExp('(^|[^-\\w])' + prop + ':'));
    }
  });

  it('every tool head is the same head — a mono caption left, its own slot right', async () => {
    mount();
    await settle();
    const heads = [...host.querySelectorAll('.qblock .qhead')];
    expect(heads).toHaveLength(3);
    for (const h of heads) {
      // ONE caption class across all three. The countdown used `.dcap` and the
      // other two `.r-lbl`; they render the same and are not the same thing, so
      // one of them had to go and the shared one stayed.
      const lbl = h.querySelector('.r-lbl');
      expect(lbl, 'a tool head with no shared label').not.toBeNull();
      expect(h.querySelector('.qspring'), 'a head with nothing pushing its right slot over').not.toBeNull();
      // The caption is the FIRST thing in the head, in every one of them.
      expect(h.firstElementChild.classList.contains('r-lbl')).toBe(true);
    }
    expect(heads.map((h) => h.querySelector('.r-lbl').textContent.trim()))
      .toEqual(['Countdown', 'Name band', 'Stage Message']);
  });

  // ── C2 · THE COUNTDOWN BLOCK, CLEANED (operator instruction 2026-09-14) ──
  //
  //   "Fix this quick action properly and cleanly."
  //
  // Three defects, all of them layout and all of them visible only at a real
  // width — which is why the arithmetic is written into the assertions rather
  // than left to a screenshot nobody can take on this machine:
  //
  //   1. `repeat(auto-fit, minmax(50px, 1fr))` lays as many tracks as FIT, so
  //      the number of buttons per row is a function of the CARD's width. Quick
  //      tools is `1.1fr` of `1.25+1.5+1.1+1fr`; at a 1600px desk that is ~323px
  //      inside the block, which is five tracks — five buttons and an orphan.
  //   2. the state line was a row of its own between the fields and the
  //      transport, a caption for neither.
  //   3. the field row ended in a `.qspring`, so it was the one row in the three
  //      blocks that did not run to the card's edge.
  //
  // Each assertion was watched to fail with its own defect put back.
  describe('C2 · the countdown block', () => {
    it('every button in a row is the same width, whatever the card is', () => {
      // `auto-fit` is the defect itself, not a detail of it: any rule that sizes
      // tracks by available width can produce a ragged last row. What is allowed
      // is a fixed count — `grid-auto-flow: column` (one track per button) or an
      // explicit `repeat(<n>, …)`.
      for (const sel of ['.qbtns', '.cdtrans']) {
        const r = rule(sel);
        expect(r, `${sel} has no rule`).not.toBe('');
        expect(r, `${sel} still sizes its tracks by the card's width`).not.toMatch(/auto-fit|auto-fill/);
      }
      expect(rule('.qbtns'), 'the two-button rows are not one even row')
        .toMatch(/grid-auto-flow: column[\s\S]*grid-auto-columns: minmax\(0, 1fr\)/);
      // Six buttons cannot be six across in a ~323px card without clipping
      // `Resume`, so the transport is three and three — still every cell equal.
      expect(rule('.cdtrans')).toMatch(/grid-template-columns: repeat\(3, minmax\(0, 1fr\)\)/);
      expect(rule('.cdtrans'), 'the transport inherits the column flow and runs one row of six')
        .toMatch(/grid-auto-flow: row/);
    });

    it('the state line labels the figure, and is not a row between the fields and the buttons', async () => {
      mount();
      await settle();
      const head = host.querySelector('.qblock.tmr .qhead');
      expect(head.querySelector('.cdstatev'), 'the state word is not with the figure it labels')
        .not.toBeNull();
      // The figure and the word are ONE thing in the head's right-hand slot.
      const fig = host.querySelector('.qblock.tmr .cdfig');
      expect(fig, 'no figure stack').not.toBeNull();
      expect(fig.querySelector('.tfig')).not.toBeNull();
      expect(fig.querySelector('.cdstatev')).not.toBeNull();
      // …and the orphaned row is gone from the markup and from the stylesheet.
      expect(host.querySelector('.qrow.cdstate'), 'the orphan row is still rendered').toBeNull();
      expect(rule('.cdstate'), '.cdstate still has a rule of its own').toBe('');
    });

    it('the field row runs to the card edge, like every other row in the three blocks', async () => {
      // The format select takes the rest of the line — the prototype's
      // `.tset .pick.sm { flex: 1 }` — instead of sizing to `auto` with a spring
      // after it. Read off the rendered row, not off the source: the defect was
      // a spring element, and an assertion on the CSS alone would miss its twin.
      expect(rule('.cdfmt')).toMatch(/flex: 1 1 auto/);
      mount();
      await settle();
      const row = host.querySelector('.qblock.tmr .qrow');
      expect(row.querySelector('.qspring'), 'the field row still ends in a spring').toBeNull();
      expect(row.lastElementChild.classList.contains('cdfmt')).toBe(true);
    });
  });

  it('every button row is the same grid, and none of them is a wrapping flex', () => {
    const btns = rule('.qbtns');
    expect(btns).toMatch(/display: grid/);
    expect(btns).toMatch(/gap: 5px/);
    // The transport may narrow its CELL — it holds six buttons where the others
    // hold two — but it may not go back to being a different kind of row. A
    // wrapping flex of fixed-width controls is what ran the countdown figure and
    // `Take down` past this card's right edge at 1024.
    const trans = rule('.cdtrans');
    expect(trans).not.toMatch(/display: flex/);
    expect(trans).not.toMatch(/flex-wrap/);

    // And every row in the markup uses it.
    const card = src.slice(src.indexOf('<span class="dk">Quick tools</span>'));
    const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
    const rows = [...body.matchAll(/<div class="([^"]*)"[^>]*role="group"/g)].map((m) => m[1]);
    expect(rows.length).toBeGreaterThanOrEqual(2);
    for (const cls of rows) expect(cls.split(/\s+/)).toContain('qbtns');
  });

  it('the name band no longer hangs its contents off an 80px indent', async () => {
    // It was the one tool whose fields, buttons, preview and caption started a
    // third of the way across a 200px card, under a label that is in the HEAD.
    for (const sel of ['.ltsub', '.ltrow', '.ltcap']) {
      expect(rule(sel), `${sel} is still here`).toBe('');
    }
    expect(rule('.ltprev')).not.toMatch(/margin-left/);

    mount();
    await settle();
    // The two fields are stacked and full width, as the prototype's `.lt3` has
    // them — not a pair squeezed side by side into ~95px each.
    const name = host.querySelector('[aria-label="Name for the lower third"]');
    const role = host.querySelector('[aria-label="Role for the lower third"]');
    expect(name.parentElement).toBe(role.parentElement);
    expect(name.parentElement.classList.contains('qblock')).toBe(true);
    for (const f of [name, role]) expect(f.classList.contains('wide')).toBe(true);
  });

  it('`Send to stage` is a button variant this stylesheet actually defines', () => {
    // `pri` is not a class in `src/app.css`. The one button in Quick tools meant
    // to read as the primary action had been rendering as a plain `.r-btn` for as
    // long as it has existed — invisible, because a plain button is a perfectly
    // ordinary thing to look at.
    expect(src).not.toMatch(/class="r-btn sm pri"/);
    const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');
    for (const v of [...src.matchAll(/class="r-btn ([a-z ]+)"/g)].flatMap((m) => m[1].split(/\s+/))) {
      expect(css, `.r-btn.${v} is used in Dock.svelte and defined nowhere`)
        .toMatch(new RegExp('\\.r-btn\\.' + v + '[{ ,:]'));
    }
  });
});

describe('L2 · the countdown format picker', () => {
  afterEach(() => countdownFormat.set('auto'));

  // §7: "One timer, one formatter, read by the slide, the stage rail and the
  // transport so they cannot drift." The picker ASKS that formatter — it is the
  // third argument `layers.js::formatCountdown` has always taken.
  it('feeds the one formatter and adds no second one', () => {
    const script = instanceScript();
    expect(script).toMatch(/formatCountdown\([^)]*\$countdownFormat\)/);
    // No hand-rolled hours, minutes or seconds anywhere in this component.
    expect(script).not.toMatch(/Math\.floor\([^)]*3600\)/);
    expect(script).not.toMatch(/padStart\(2, '0'\)/);
  });

  it('offers exactly the three §7 names', async () => {
    mount();
    await settle();
    const pick = host.querySelector('.cdfmt');
    expect(pick).not.toBeNull();
    expect([...pick.options].map((o) => o.value)).toEqual(['auto', 'ms', 'hms']);
    expect([...pick.options].map((o) => o.textContent.trim())).toEqual(['auto', 'm:ss', 'h:mm:ss']);
    expect(pick.getAttribute('aria-label')).toBe('Countdown format');
  });

  it('changes how the same number reads', async () => {
    mount();
    await settle();
    const fig = () => host.querySelector('.tfig').textContent.trim();
    expect(fig()).toBe('5:00');
    countdownFormat.set('hms');
    await tick();
    expect(fig()).toBe('0:05:00');
    countdownFormat.set('ms');
    await tick();
    expect(fig()).toBe('5:00');
  });

  // IT CHANGES THE READOUT, AND SAYS SO. A wall's countdown is rendered from
  // `OutputContent`, which carries no format field, so a control that implied it
  // reached the screens would be claiming a reach it has not got — rule 35's
  // family, on the panel an operator watches a service from.
  it('says which figure it governs, and claims nothing about the screens', async () => {
    mount();
    await settle();
    const title = host.querySelector('.cdfmt').getAttribute('title');
    expect(title).toMatch(/this readout/i);
    expect(title).toMatch(/screens read the countdown through their own template/i);
  });

  // THE CHOICE OUTLIVES THE COMPONENT. The shell renders the dock as
  // `{#if !liveFullscreen}<Dock />{/if}`, so pressing Full screen DESTROYS it —
  // the same trap `countdown.js` records for the set duration, and the reason
  // this store is at module scope rather than a component `let`.
  it('survives the component being destroyed and rebuilt', async () => {
    mount();
    await settle();
    countdownFormat.set('hms');
    await tick();
    app.$destroy();
    host.remove();

    mount();
    await settle();
    expect(host.querySelector('.cdfmt').value).toBe('hms');
  });
});

describe('L2 · the audio card is set as the capitals it is tracked for', () => {
  it('`Sens` renders as capitals without the word being rewritten', async () => {
    const css = src.slice(src.indexOf('  .dcap {'), src.indexOf('  .dcap {') + 320);
    expect(css).toMatch(/text-transform: uppercase/);
    mount();
    await settle();
    // The WORDS stay Relay's — a screen reader hears "Sens", not "S E N S".
    // FOUR now, not two: the microphone's own row joined this card in wave 3
    // (L3) so a volunteer can change the input without leaving the run surface.
    // `Listen` / `Listening`, not `off` / `live`. Four instructions — the guide,
    // the Help screen, the latency panel and the empty History pane — say "press
    // Start listening", and this caption was the only thing near the control. It
    // read as a status rather than a button, so the first action of every service
    // was named by a word that appeared nowhere on the surface.
    expect([...host.querySelectorAll('.audrow .dcap')].map((e) => e.textContent.trim()))
      .toEqual(['Mic', 'Listen', 'Sens', 'armed']);
  });

  // THE VOICE CHIP GOES ON TELLING THE TRUTH. The prototype's reads `VOICE`; ours
  // already did when there IS voice, and what it says otherwise is the half that
  // matters — with no engine attached neither word may appear at all, because
  // `quiet` over a detached engine reads exactly like a live mic in a silent room
  // (rule 35). This was NOT changed and must not be.
  it('the chip still reports the gate, and says nothing at all with no engine', () => {
    expect(src).toMatch(/\$meter\.isVoice \? 'VOICE' : 'quiet'/);
    const head = src.slice(src.indexOf('{#if !$capture.available}'), src.indexOf('<div class="dbody audbody">'));
    expect(head).toContain('no engine');
    expect(head.indexOf('no engine')).toBeLessThan(head.indexOf("'VOICE'"));
  });

  // THE SAMPLE RATE IS NOT PRINTED, and the prototype's `INPUT · 48 kHz` is
  // deliberately not copied: the console's meter store drops `sample_rate`, so the
  // figure would be a constant reading the same on a device running at 16 kHz —
  // which is the state that silently switches the denoiser off. Rule 35 again.
  it('the waveform well names the input and claims no rate it cannot read', async () => {
    const well = src.slice(src.indexOf('<span class="wavelbl'), src.indexOf('<span class="wavelbl') + 120);
    // `INPUT · 20s`. The SPAN is printed because the trace is drawn on a time
    // axis now — a picture that does not state its own scale cannot be read.
    expect(well).toContain('INPUT · ');
    // Read the RENDERED card, not the file: the reason this rate is absent is
    // written in a comment beside the element, and the comment must not be what
    // the test is looking at.
    mount();
    await settle();
    expect(host.querySelector('.audbody').textContent).not.toMatch(/kHz/);
  });
});

describe('L2 · the Controls card keeps Relay’s order, on purpose', () => {
  // THE PROTOTYPE'S ORDER IS DERIVED FROM A BUTTON RELAY DOES NOT HAVE. It leads
  // with `Go Live`, which this repository declined (there is no honest frontend
  // state to drive it — see the note above `.dpanel.ctl`), so copying the order
  // would put `End service` under the thumb in a green button's place: the one
  // control here that is NOT about the next thirty seconds.
  //
  // Clear screens stays first and full width. Rule 15's neighbourhood: the control
  // an operator reaches for without reading is the red one.
  it('runs End service · Rehearse · Blackout · Clear screens, the prototype order', () => {
    // CHANGED 2026-09-14 on the operator's instruction. The card previously led
    // with Clear screens on the argument that the control reached for without
    // reading belongs under the thumb; the bottom edge of a card that never
    // scrolls is the same distance away, and it is where the prototype puts it.
    // Matched by class: `End service` also appears in that button's title text.
    const ctl = src.slice(src.indexOf('<div class="r-ctl">'));
    const body = ctl.slice(0, ctl.indexOf('</div>', ctl.lastIndexOf('danger wide')));
    const order = ['r-cbtn endsvc', 'rehearse', 'r-cbtn black', 'r-cbtn danger wide'];
    let at = -1;
    for (const cls of order) {
      const i = body.indexOf(cls);
      expect(i, `${cls} must be in the controls card`).toBeGreaterThan(at);
      at = i;
    }
  });

  // …and the card records that the order was DECIDED rather than inherited, and
  // what the panic rule actually requires — which is not an order at all.
  it('and the card says the order is the prototype\'s, and what rule 15 needs', () => {
    expect(src).toMatch(/THE ORDER IS THE PROTOTYPE'S, on the operator's instruction/);
    expect(src).toMatch(/never scrolls/);
  });
});
