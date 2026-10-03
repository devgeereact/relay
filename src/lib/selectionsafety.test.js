// SELECTING A PLANNER ITEM CHANGES NOTHING A CONGREGATION CAN SEE (RG-266).
//
// The operator: *"When planner items are selected... make sure what's on stays
// intact.... not changing the design of how the display or screen is."*
//
// **Checked rather than changed.** Selection in the Planner writes four
// component-local `let`s and nothing else — no store, no command, no event — and
// `plannerbuildonly.test.js` already holds that from two directions: statically
// (the component may not import a command that takes a screen) and behaviourally
// (it clicks every row three times and asserts none was dispatched).
//
// What this file adds is the half those do not say out loud: that selection
// cannot change WHICH TEMPLATE resolves, which is the mechanism by which "the
// design of how the display is" could move without any fire at all. DECISIONS
// §29 is the resolution rule, and the concern is a real one — a cue carries a
// pinned template, and a selection that fed it into the resolver would repaint
// a screen while claiming to have done nothing.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const read = (p) => readFileSync(resolve(p), 'utf8');
const PLANNER = read('src/lib/views/ServicePlanner.svelte');
const LIVE = read('src/lib/views/Live.svelte');

describe('what a Planner selection is allowed to touch', () => {
  it('writes no store and sends no command', () => {
    const fn = PLANNER.slice(PLANNER.indexOf('function pick('), PLANNER.indexOf('}', PLANNER.indexOf('if (cueDelArm')));
    expect(fn, 'the selection handler was not found').toContain('selId = id');
    for (const reaches of ['setSession', 'invoke', 'await ', 'Fire', 'fire']) {
      expect(fn, `selection reaches ${reaches}`).not.toContain(reaches);
    }
  });

  it('a cue’s pinned template is bound to the Template control, never to selection', () => {
    // `setPlanTemplate` is what pins one, and it takes effect when that cue is
    // fired in Live — not when somebody clicks the row to look at it.
    const fn = PLANNER.slice(PLANNER.indexOf('function pick('), PLANNER.indexOf('}', PLANNER.indexOf('if (cueDelArm')));
    expect(fn).not.toContain('setPlanTemplate');
  });
});

describe('and what the run surface resolves a template from', () => {
  it('the programme pane reads the SCREEN’s template, never the selection', () => {
    // DECISIONS §29: a screen's own template wins, a content look defers to it,
    // a cue's deliberate choice pins. None of those three is the cursor.
    const line = LIVE.slice(LIVE.indexOf('$: progTpl ='), LIVE.indexOf(';', LIVE.indexOf('$: progTpl =')));
    expect(line, 'the programme pane was not found').toContain('resolveOutputTemplate');
    for (const cursor of ['selCue', 'selId', 'gridPreview']) {
      expect(line, `the pane repaints from ${cursor}`).not.toContain(cursor);
    }
  });

  it('what a TAKE is pointed at does follow the cursor, and that is named', () => {
    // THE ONE REAL COUPLING, and it is worth saying rather than hiding: with no
    // plan open, `previewCue` falls back to the selected cue, so changing the
    // selection changes what the next Take would fire. It changes nothing that
    // is already on a screen — which is what the operator asked about — but an
    // operator who selects and then presses Take gets the thing they selected.
    expect(LIVE).toMatch(/previewCue = previewNext \?\? \(selCue/);
  });
});

// ── AND A STAGE MESSAGE FLASHES ON THE BIG SCREEN, NOT ONLY THE PHONE ────────
//
// The operator: *"when a stage message is sent to the stage it should flash like
// on the mobile on the big screen also"*.
//
// **Checked, and it already does** — RG-156 / DECISIONS §116 built it, through
// both doors, with the same colours and the same 1.4s cycle as `stage.html`. So
// nothing was rebuilt; what was missing is a test saying so in one place, and
// the one fact that decides whether an operator sees it at all.
describe('an urgent Stage Message on a screen served by output.html', () => {
  const TR = read('src/lib/TemplateRender.svelte');
  const OUT = read('src/Output.svelte');

  it('takes the whole screen and flashes, on the same cycle as the phone', () => {
    const rule = TR.slice(TR.indexOf('.lalert {'), TR.indexOf('}', TR.indexOf('.lalert {')));
    expect(rule).toMatch(/position: absolute/);
    expect(rule).toMatch(/inset: 0/);
    // THE SAME 1.4s, and the same keyframes name, as `Stage.svelte`. Two panels
    // flashing at different rates in one room is worse than one.
    expect(TR).toMatch(/animation: stagealert 1\.4s/);
    const phone = read('src/Stage.svelte');
    expect(phone).toMatch(/animation: stagealert 1\.4s/);
  });

  it('reaches the page through BOTH doors — the socket and the native window', () => {
    // RG-156's own finding: `channels::stage_alert` published to the hub and
    // emitted nothing, so a screen wired as a native window heard no Stage
    // Message at all while the console reported one sent.
    expect(OUT).toMatch(/m\.kind === 'stage_alert'/);
    expect(OUT).toMatch(/listen\('output:\/\/stage_alert'/);
  });

  it('and it is gated on the screen being a STAGE, which is where to look first', () => {
    // A lobby TV and a stream feed arrive with no role at all, and a filter
    // whose default is yes is not a filter. If a big screen does not flash, the
    // channel's role in Outputs is the first thing to check, not the renderer.
    expect(OUT).toMatch(/acceptsStageMessage\(myRole\)/);
    expect(read('src/lib/channelroles.js')).toMatch(/role === 'stage'/);
  });
});
