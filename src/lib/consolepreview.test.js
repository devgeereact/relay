// THE CONSOLE'S PROGRAM PANE IS A PICTURE OF THE WALL, OR IT IS NOTHING (RG-222).
//
// The operator's words: *"When the media is paused, everything about the media is
// paused... the programme dosent keep playing and the screens are paused."*
//
// Two props were missing from the one `TemplateRender` on Live that claims to
// show what the congregation is seeing, and each is a different lie:
//
//   · `mediaTransport` — Pause holds every SCREEN in the building and the
//     operator's own preview carried on playing, so the one surface they watch
//     to decide whether a control worked said it had not.
//   · `programme` — the preacher's clocks never reached the pane at all, so a
//     template carrying a programme layer previewed without them and an operator
//     could not see what the preacher's screen was showing.
//
// This is rule 35 on the pane's own terms: it exists to be trusted, and a pane
// that disagrees with the wall about whether a clip is running is worse than no
// pane, because an operator stops pressing the control that did work.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');
const OUTPUT = readFileSync(resolve(__dirname, '../Output.svelte'), 'utf8');

/** The program pane's own `<TemplateRender …>` tag, markup only. */
const programPane = () => {
  const src = codeOnly(LIVE);
  const at = src.indexOf('<TemplateRender\n            template={progTpl}');
  expect(at, 'the program pane is no longer identifiable in the markup').toBeGreaterThan(-1);
  return src.slice(at, src.indexOf('/>', at));
};

describe('the pane is handed the same facts the wall is', () => {
  it('carries the media transport, so a held clip is held here too', () => {
    // The store form here (`mediaTransport={$mediaTransport}`), the shorthand on
    // the output page, which holds its own local copy. Same prop, two idioms.
    expect(programPane(), 'Pause moves every screen and not this pane').toMatch(
      /mediaTransport=\{\$mediaTransport\}/,
    );
  });

  it('does NOT carry the programme — this pane previews a CONGREGATION screen', () => {
    // RG-222 handed it the clocks so a stage template would preview with them.
    // That was wrong and the operator saw it within the day: this pane is a
    // picture of the MAIN screen, which never shows the running order (the rail
    // is role-gated at the output page for exactly that reason), and RG-224's
    // fallback rail then drew `14:57` over a clip that was on air.
    //
    // "Sermon · 4:12 left" on a preview of a congregation screen is the same
    // claim as on the screen itself: it says the wall is showing it.
    expect(programPane(), 'the running order is previewed on a congregation screen').not.toMatch(
      /programme=\{/,
    );
  });

  it('and the output page — the other door — carries both, as it already did', () => {
    // The twin this file exists to close. Stated as an assertion rather than as
    // a comment, so a change that strips the output page has to argue with a
    // test rather than with prose.
    const out = codeOnly(OUTPUT);
    expect(out).toMatch(/\{mediaTransport\}/);
    // The output page still carries it, gated on the screen's ROLE — which is
    // what Live has no business second-guessing.
    expect(out).toMatch(/programme=\{shownProgramme\}/);
  });
});

// ── AND THE TWO SHAPES A TIMER COMES IN (RG-222) ────────────────────────────
//
// The registry and the wire name the same facts differently — `target_ms` here,
// `countdown_to` there — because Rust's `timer_frame_json` renames them on the
// way out. The console reads the REGISTRY (`list_timers`) and `TemplateRender`
// reads the WIRE, so the pane needs a projection, and it is a shared function
// rather than an object literal inside a view for the ordinary reason: two
// copies of a rename is how a field silently stops arriving on one surface.
describe('timerAsRow — the registry row a renderer can read', () => {
  const reg = (over = {}) => ({
    id: 4,
    label: 'Sermon',
    scope: 'stage',
    target_ms: 1_700_000_240_000,
    from_ms: 1_700_000_000_000,
    paused_ms: null,
    done_msg: 'Time',
    warn_ms: 60_000,
    ...over,
  });

  it('renames every field the wire renames, and drops nothing', async () => {
    const { timerAsRow } = await import('./timers.js');
    expect(timerAsRow(reg())).toEqual({
      id: 4,
      label: 'Sermon',
      countdown_to: 1_700_000_240_000,
      countdown_from: 1_700_000_000_000,
      countdown_paused_ms: null,
      countdown_done: 'Time',
      warn_ms: 60_000,
    });
  });

  it('and what it produces is what the rail actually reads', async () => {
    // The claim that matters: not "the keys are these" but "the renderer's own
    // rule accepts it". `programmeRows` is what the rail is built from.
    const { timerAsRow, programmeRows } = await import('./timers.js');
    const rows = programmeRows([timerAsRow(reg({ target_ms: Date.now() + 240_000 }))], Date.now());
    expect(rows).toHaveLength(1);
    expect(rows[0].label).toBe('Sermon');
    expect(rows[0].v).toMatch(/\d:\d\d/);
  });

  it('a held timer survives the rename, because that is the field a pause writes', async () => {
    const { timerAsRow } = await import('./timers.js');
    expect(timerAsRow(reg({ paused_ms: 90_000 })).countdown_paused_ms).toBe(90_000);
  });
});
