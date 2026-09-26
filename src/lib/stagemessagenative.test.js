// THE STAGE MESSAGE ON THE OTHER DOOR — a native output window (RG-156).
//
// `stagemessage.test.js` beside this file drives the KIOSK door: a browser source
// with no Tauri bridge, reading frames off the socket. Every guarantee it holds
// was true and none of it reached the second door, because there was no second
// door: `channels::stage_alert` called `publish_kiosk` and nothing else, so a
// screen wired as a `native_window` and given the `stage` role received nothing
// at all.
//
// THE FAILURE DIRECTION IS SILENCE, which is the worse of the two. The console
// reports a Stage Message sent, the operator believes the preacher has been told,
// and one whole class of stage screen never heard it. That is rule 35 seen from
// the engine end rather than the badge end — a control that cannot detect its own
// failure, where the thing that cannot detect it is the sender.
//
// WHY THE ROLE CHECK IS THE SAME CHECK AND NOT A SECOND ONE. The kiosk hub cannot
// address one client (DECISIONS §35, not being reversed), so the filter has always
// lived on the receiving page. A Tauri emit reaches every webview, so it needs the
// identical filter — and `acceptsStageMessage(myRole)` is that filter, asked at
// both doors from one function. Two expressions would be two rules, and this
// repository's most-repeated bug is a guarantee kept on one of two doors.
//
// Watched to fail: without the `output://stage_alert` listener the first test
// paints nothing; with the listener but no role check the second and third paint
// the message on a congregation screen.
//
//   npx vitest run src/lib/stagemessagenative.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
const handlers = new Map();
const listen = vi.fn(async (name, fn) => {
  handlers.set(name, fn);
  return () => {};
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Output = (await import('../Output.svelte')).default;
const PAGE = readFileSync(resolve(process.cwd(), 'src/Output.svelte'), 'utf8');

/** A stage-shaped template: the verse, and a layer bound to Stage Message. */
const STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 40, size: 4, color: '#fff' },
      {
        id: 'msg',
        type: 'text',
        bind: 'stage_message',
        x: 5,
        y: 60,
        w: 90,
        h: 20,
        size: 5,
        color: '#ff5555',
      },
    ],
  },
  style: { background: '#000' },
};

const MESSAGE = 'Wrap up — 5 minutes';
/** A fresh install: one main screen, one stage, and congregation screens with no role. */
const SEEDED_ROLES = { 1: 'main', 2: 'stage' };

let host;
let app;

async function settle() {
  for (let round = 0; round < 3; round += 1) {
    for (let i = 0; i < 60; i += 1) await Promise.resolve();
    await tick();
  }
}

/** Mount `output.html` as a NATIVE window on `channel`, with the roles known. */
async function open(channel) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  fire('output://channel_roles', { roles: SEEDED_ROLES });
  await settle();
}

const fire = (name, payload) => handlers.get(name)?.({ payload });
const painted = () => host.textContent;

beforeEach(() => {
  invoke.mockReset();
  listen.mockClear();
  handlers.clear();
  invoke.mockImplementation((cmd, args) => {
    if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(STAGE_TPL);
    if (cmd === 'list_output_channels')
      return Promise.resolve([{ id: 1, name: 'Main screen' }, { id: 2, name: 'Stage display' }]);
    if (cmd === 'get_content_templates')
      return Promise.resolve({ scripture: null, song: null, media: null, announce: null, countdown: null });
    if (cmd === 'list_channel_looks') return Promise.resolve({});
    if (cmd === 'channel_roles') return Promise.resolve(SEEDED_ROLES);
    return Promise.resolve(null);
  });
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('a native stage window is reached by a Stage Message — RG-156', () => {
  it('the page listens on the Tauri door at all', async () => {
    await open(2);
    expect(
      [...handlers.keys()],
      'no output://stage_alert listener — a native stage screen hears nothing',
    ).toContain('output://stage_alert');
  });

  it('paints it on a native window whose role is stage', async () => {
    await open(2);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
  });

  it('paints NOTHING on a native window that is the main screen', async () => {
    // The congregation case, and the one the whole guarantee is about. `main` is
    // a real role and it is not `stage`; a filter that answered yes here would put
    // a word meant for one person on the wall in front of everybody.
    await open(1);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('a blank message takes it down again rather than leaving it on the glass', async () => {
    await open(2);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
    fire('output://stage_alert', { text: null });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('and BOTH doors ask the one predicate, rather than two expressions of it', () => {
    // The rule, not the instance. A second door that re-derived "is this a stage"
    // is two rules about one fact, and the second one is the one that drifts.
    // FOUR CALL SITES, exactly, and the regex requires the `(` so the import line
    // is not one of them: the programme-rail gate (§104), the reset when a screen
    // stops being a stage, the socket door, and the Tauri door added here.
    //
    // It is `toBe` rather than a range on purpose. This assertion was first written
    // as `>= 3` and PASSED against the broken tree — three call sites existed and
    // the missing door was the whole defect — which is a guard on a guarantee that
    // is really a guard on nothing. At four it fails before the fix and passes
    // after it, which is the only shape that means anything.
    const calls = PAGE.match(/acceptsStageMessage\(/g) ?? [];
    expect(
      calls.length,
      'a door stopped asking, or a third one appeared without the check',
    ).toBe(4);
  });
});

// ── TWO VERBS, ONE FIELD (operator, 2026-09-21; DECISIONS §116) ────────────
//
// "When a message is written in the stage message section it should show on the
// stage display fixed text only; if the operator sends to stage or alert then it
// fills the screen with a flashing warning red line."
//
// There was ONE rendering: the full-bleed flashing panel. So "wrap up in five"
// arrived as the same emergency as "stop, there is a medical incident", and an
// alarm spent on ordinary business stops being an alarm. Nothing in the product
// could put a quiet word on a preacher's screen.
//
// THE FALLBACK IS THE PART THAT NEEDED CARE, and it was found by reading the
// operator's own template rather than by reasoning. `Stage · Reading` has five
// layers — Background, Reading, Reference, Clock, Elapsed — and NO
// `stage_message` layer. A quiet send that only ever painted into that layer
// would have been swallowed on the one screen this was asked for, which is the
// silence RG-156 was about, arriving by a different door. So a quiet message
// with nowhere declared to go gets a modest fixed strip instead of nothing.
describe('a quiet word and an alarm are different things — DECISIONS §116', () => {
  const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');

  it('the renderer takes urgency as its own fact, not as the presence of words', () => {
    expect(RENDER, 'the renderer cannot tell a note from an alarm').toMatch(
      /export let stageUrgent/,
    );
  });

  it('the flashing panel is gated on urgency', () => {
    // It was `{#if stageAlert}` — any words at all. The panel is the thing that
    // must be rare, so it is the thing that must be asked for.
    expect(RENDER).toMatch(/\{#if stageAlert && stageUrgent\}/);
  });

  it('a quiet message with no layer to land in still lands, modestly', () => {
    // NEVER SWALLOWED. The operator's own stage template declares no
    // `stage_message` layer, so "renders into the layer, and otherwise nowhere"
    // would be a send that reports success and shows nothing.
    expect(RENDER, 'no quiet strip at all').toMatch(/class="lmsg"/);
    expect(RENDER, 'the strip is not conditional on the template lacking a place').toMatch(
      /\{#if stageAlert && !stageUrgent && !hasMessageLayer\}/,
    );
  });

  // REVERSED, 2026-09-23, RG-268. This test used to read "…and it does not
  // flash, which is the whole distinction" and assert that NOTHING in the strip
  // moved. That was too broad a reading of §116 and it is what left the big
  // screen grey: the distinction the phone actually draws is that *a message
  // pulses its text and an alert flashes its panel* (`Stage.svelte`), not that
  // an ordinary message is motionless. The old claim is kept below, narrowed to
  // the PANEL, which is the half that was ever load-bearing.
  it('…and the PANEL still does not flash, which is the alarm’s job alone', () => {
    const strip = /\.lmsg\s*\{[\s\S]*?\}/.exec(RENDER);
    expect(strip, 'the strip has no styling').toBeTruthy();
    expect(strip[0], 'the quiet strip’s panel animates — then it is a second alarm').not.toMatch(
      /animation:/,
    );
    // And it is still a strip, not the screen: `.lalert` is `inset: 0`.
    expect(strip[0], 'the quiet strip took the whole screen').not.toMatch(/inset:\s*0/);
  });

  it('the page carries urgency from both doors', () => {
    expect(PAGE, 'the socket door drops urgency').toMatch(/m\.urgent/);
    expect(PAGE, 'the Tauri door drops urgency').toMatch(/payload\?\.urgent/);
  });
});


// ── A MESSAGE THAT IS NOTICED, ON THE BIG SCREEN TOO (RG-268) ───────────────
//
// The operator, with a screenshot of a stage TV: *"Stage message sent still not
// flashing catching attention as mentioned earlier... its just showing a
// gray/white text which can easily be missed."*
//
// RG-239 gave the phone's ordinary message the caution ink, a rule down its edge
// and a gentle pulse of its text. The big screen kept the strip it was built
// with: white on a black plate, a hairline white border, no colour and nothing
// moving. So the two stage surfaces disagreed about what an ordinary message
// looks like, and which one a preacher got depended on whether the church put a
// tablet or a TV in front of him — the same asymmetry RG-156 filed one door
// along, arriving as a whisper rather than as silence.
//
// WHAT MUST SURVIVE. An ALERT is still the full-bleed red panel that flashes
// (DECISIONS §116). A message pulses its TEXT; an alarm flashes its PANEL. That
// sentence is `Stage.svelte`'s and it is the whole distinction, so the tests
// below assert the ink, the rule and the pulse on the WORDS, and the two tests
// above assert that the plate they sit on still does neither.
//
// COLOUR LAW (rule 18) — AND THE INK CHANGED AGAIN ON 2026-09-24 (RG-295).
// These cases read `--v-caution` when they were written, which was right: ochre
// was the only free ink, and a message warns and promises nothing about a
// screen. The operator then said, about the one surface they are the sole judge
// of: *"when the message is sent make it flashing red text so it can catch
// attention of the preacher"*. The person the message is FOR reports it is not
// catching their eye, and that is evidence no colour rule outranks.
//
// So these now read `--v-red`. Amber and amethyst are still forbidden here and
// still asserted; red was never a promise colour, which is why the request could
// be granted at all. `colourlaw.test.js` holds both surfaces together — both or
// neither — and holds the shape test that §116's note/alarm line now rests on.
//
//   npx vitest run src/lib/stagemessagenative.test.js
describe('the quiet word is SEEN on the big screen as well — RG-268', () => {
  const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');
  const STYLE = RENDER.slice(RENDER.indexOf('<style>'));
  /** The CSS block of one selector, or '' if it is gone. */
  const rule = (sel) => {
    const m = STYLE.match(new RegExp(`${sel.replace(/[.]/g, '\\.')}\\s*\\{([^}]*)\\}`));
    return m ? m[1] : '';
  };

  it('the words are the red the operator asked for, not the grey they missed', () => {
    // The whole of the operator's complaint in one assertion. `color: #fff` on a
    // black plate is what the screenshot showed.
    const r = rule('.lmsg-v');
    expect(r, 'there is no element carrying the words on their own').not.toBe('');
    expect(r, 'the words are still painted in a colour the preacher missed').toMatch(
      /var\(--v-red/,
    );
  });

  it('and the plate carries a visible rule in the same ink', () => {
    // A border the operator can see from a platform, rather than the hairline
    // `rgba(255,255,255,.22)` that read as part of the slide.
    const r = rule('.lmsg');
    expect(r).not.toBe('');
    expect(r, 'no left rule — the strip is still an unmarked plate').toMatch(
      /border-left:[^;]*var\(--v-red/,
    );
    expect(r, 'the strip still wears the hairline white border it was missed in').not.toMatch(
      /border:\s*1px solid rgba\(255, 255, 255/,
    );
  });

  // EVERY `@media (prefers-reduced-motion: <pref>)` body, rather than the first
  // one. `stagealerttemplate.test.js` reads the FIRST of each for `.lalert`, so a
  // rule for the strip placed above it would quietly retarget that instrument at
  // this one — a scanner that narrows while still passing, which is the shape
  // `ipc.test.js` records twice. This one enumerates instead.
  const motionBlocks = (pref) =>
    [...STYLE.matchAll(new RegExp(`@media \\(prefers-reduced-motion: ${pref}\\)\\s*\\{`, 'g'))]
      .map((m) => STYLE.slice(m.index, STYLE.indexOf('\n  }\n', m.index) + 5));

  it('the words pulse, which is the phone’s answer and now this one', () => {
    // BEHIND `no-preference`, exactly as `Stage.svelte` has it. A pulse that runs
    // for a viewer who asked for no animation is not a gentler alarm, it is a
    // setting ignored.
    expect(STYLE, 'nothing pulses at all').toMatch(/@keyframes stagemsg/);
    const blocks = motionBlocks('no-preference').filter((b) => b.includes('.lmsg-v'));
    expect(blocks.length, 'the pulse is not gated on no-preference').toBe(1);
    expect(blocks[0]).toMatch(/animation: stagemsg/);
  });

  it('and reduced motion keeps the colour and drops the movement', () => {
    // AN EQUIVALENT, NOT A QUIETER STATE — the phone's rule verbatim: the text
    // rests AT the message ink rather than pulsing to it, so the message is still
    // a coloured message rather than a plain one.
    const blocks = motionBlocks('reduce').filter((b) => b.includes('.lmsg-v'));
    expect(blocks.length, 'reduced motion is not answered for the strip').toBe(1);
    expect(blocks[0]).toMatch(/var\(--v-red/);
    expect(blocks[0], 'the reduced-motion answer is the pulse under another word').not.toMatch(
      /animation|filter:\s*brightness/,
    );
  });

  it('the scanner sees a motion block when there is one, and none when there is not', () => {
    // The guard on the guard. A helper that silently matched nothing would make
    // the two tests above pass by counting zero, and this repository has shipped
    // that mistake twice.
    expect(motionBlocks('no-preference').length).toBeGreaterThan(0);
    expect(motionBlocks('no-such-preference').length).toBe(0);
    expect(motionBlocks('reduce').some((b) => b.includes('.lalert'))).toBe(true);
  });

  it('the note and the alarm are still different things, now that they share an ink', () => {
    // REWRITTEN 2026-09-24 (RG-295). This used to assert that the note must NOT
    // reach for red at all, and that was the whole of how the two were kept
    // apart. The operator asked for the note to be red, so that separation had
    // to move somewhere or be given up — and giving it up would mean a preacher
    // cannot tell "wrap up" from "stop the service".
    //
    // It moved to SHAPE, which is the stronger test anyway because it is what a
    // person actually reads from the back of a room: the alarm is the whole
    // screen in a solid field, the note is a box with a rule down its side.
    expect(rule('.lalert'), 'the alarm stopped being red').toMatch(/#c8121c/);
    expect(rule('.lalert'), 'the alarm no longer fills the screen').toMatch(/inset:\s*0/);
    expect(rule('.lmsg'), 'the note became a full-bleed panel like the alarm').not.toMatch(
      /inset:\s*0/,
    );
    expect(rule('.lmsg'), 'the note lost the rule that distinguishes it').toMatch(/border-left:/);
    // The alarm's own solid field is NOT the note's ink, and never was: `#c8121c`
    // is a panel colour and `--v-red` is an ink. They are allowed to be cousins.
    expect(rule('.lmsg-v'), 'the words became the alarm’s panel colour').not.toMatch(/#c8121c/);
    for (const promise of ['--v-amber', '--v-cyan', '--v-amethyst']) {
      expect(rule('.lmsg'), `the strip paints ${promise}`).not.toContain(promise);
      expect(rule('.lmsg-v'), `the words paint ${promise}`).not.toContain(promise);
    }
  });
});
