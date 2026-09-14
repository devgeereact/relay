// QUICK TOOLS — the three things that change during a service, plus the one that
// interrupts it.
//
// `docs/REBRAND.md` §2 names the card's contents: the countdown, the **name
// band**, and the **word to the preacher**, with `Load whole plan` in its header.
// Relay had the countdown and a single `To preacher` row. The lower thirds and
// the `stage_alert` frame kind already existed (phases 5 and 6, DECISIONS §75 and
// §68) with no operator surface at all, which is the same defect as a command
// with no rendered control — built, shipped and unreachable.
//
// The emergency announcement is here too. It was in Live's inspector column,
// where §2 puts the AI's claims alone; it paints over live scripture on EVERY
// screen at once, so being reachable only from the tab you happen to be on was
// the argument for moving it rather than against.
//
// WHAT THESE TESTS ARE FOR, in order of how badly each would hurt:
//
//   1. nothing an operator types for the PREACHER may reach a congregation
//      channel. The guarantee is `channels.rs`'s, and this holds the door on
//      this side: the stage row's press reaches `send_stage_alert` and no fire.
//   2. the announcement is armed in two steps. A stray Enter in a text field
//      must not be able to interrupt a reading in front of a room.
//   3. the name band is an ordinary manual fire through an EXISTING path, with
//      the operator's chosen band as the cue's own template — not a new kind, not
//      a new renderer, and never automatic.
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

describe('the word to the preacher', () => {
  it('reaches the stage alert and no fire path at all', async () => {
    mount();
    await settle();
    const box = host.querySelector('[aria-label="Word to the preacher — stage monitor only"]');
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

describe('the emergency announcement', () => {
  const type = async (text) => {
    const box = host.querySelector('[aria-label="Emergency announcement"]');
    box.value = text;
    box.dispatchEvent(new Event('input'));
    await settle();
  };

  it('takes TWO presses — one stray Enter cannot interrupt a reading', async () => {
    mount();
    await settle();
    await type('Fire alarm — please leave by the side doors');

    const go = () => [...host.querySelectorAll('button')].find((b) => b.className.includes('ann-go'));
    go().click();
    await settle();
    expect(called('push_announcement')).toHaveLength(0);
    expect(go().textContent.trim()).toBe('Confirm?');

    go().click();
    await settle();
    expect(called('push_announcement')).toHaveLength(1);
    expect(called('push_announcement')[0][1]).toEqual({
      message: 'Fire alarm — please leave by the side doors',
    });
  });

  it('Enter alone arms it and does not send it', async () => {
    mount();
    await settle();
    await type('Doctor needed at the back');
    const box = host.querySelector('[aria-label="Emergency announcement"]');
    box.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await settle();
    expect(called('push_announcement')).toHaveLength(0);
  });

  it('a failure is reported — an operator must never believe the room was warned', async () => {
    mount();
    await settle();
    await type('Fire alarm');
    invoke.mockRejectedValue({ kind: 'internal', message: 'the output lock is poisoned' });
    const go = () => [...host.querySelectorAll('button')].find((b) => b.className.includes('ann-go'));
    go().click();
    await settle();
    go().click();
    await settle();
    expect(host.querySelector('[role="alert"]')).not.toBeNull();
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

describe('the card is the four things §2 names, in one place', () => {
  it('holds the countdown, the name band, the word to the preacher and the announcement', () => {
    const card = src.slice(src.indexOf('<span class="dk">Quick tools</span>'));
    const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
    expect(body).toContain('Countdown');
    expect(body).toContain('Name band');
    expect(body).toContain('To preacher');
    expect(body).toContain('Announce');
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
    const from = src.indexOf('<div class="tmr">');
    expect(from).toBeGreaterThan(-1);
    const block = src.slice(from, src.indexOf('<!-- ── THE NAME BAND', from));
    expect(block).toContain('Countdown');
    expect(block).toContain('cdfields');
    expect(block).toContain('cdstate');
    expect(block).toContain('cdtrans');
    // The seam falls BETWEEN the two tools, which is the whole reason for it.
    expect(block).not.toContain('Name band');
    const css = src.slice(src.indexOf('  .tmr {'), src.indexOf('  .tmr {') + 340);
    expect(css).toMatch(/border: 1px solid/);
  });

  // THE FIGURE IS NOT AMBER, WHATEVER THE PROTOTYPE DOES. Amber in this room means
  // a congregation is looking at something (rule 18), and this is a READOUT of a
  // countdown rather than the countdown.
  it('the block spends none of the colour law', () => {
    const css = src.slice(src.indexOf('  .tmr {'), src.indexOf('  .cdstate {'));
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
    expect([...host.querySelectorAll('.audrow .dcap')].map((e) => e.textContent.trim()))
      .toEqual(['Sens', 'armed']);
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
    expect(well).toContain('>INPUT<');
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
  it('Clear screens is still the first control in the card', () => {
    const ctl = src.slice(src.indexOf('<div class="r-ctl">'));
    const order = [...ctl.slice(0, ctl.indexOf('</div>')).matchAll(/>\s*\{?[^<>{]*?(Clear screens|Blackout|Rehearse|End service)/g)]
      .map((m) => m[1]);
    expect(order[0]).toBe('Clear screens');
    expect(order).toContain('End service');
    expect(order.indexOf('Clear screens')).toBeLessThan(order.indexOf('End service'));
  });

  // …and the reason is written down where the next person to compare the two
  // will read it, rather than being rediscovered.
  it('and the card says why it differs from the prototype', () => {
    expect(src).toMatch(/THE ORDER IS NOT THE PROTOTYPE'S, deliberately/);
  });
});
