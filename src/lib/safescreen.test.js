// RG-05 — SAFE SCREEN, the renderer's half.
//
// Rust refuses the two payloads that are unambiguously broken (`pipeline::
// preflight`, with its own tests and two e2e tests driving the real commands).
// It cannot answer the third question, because fit is a layout problem and only a
// browser can measure it:
//
//   Is the verse on the wall at a size anybody past the third row can read?
//
// The fit loop always "succeeded" — there is no verse so long that 40 rounds of
// ×0.95 cannot squeeze it in — so a template that had stopped working looked
// exactly like one that was working. It still shrinks and it still shows the
// verse (blanking the screen would be strictly worse for the congregation); what
// changed is that it now SAYS when it had to go below the size the template's
// designer asked for.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const render = read('src/lib/TemplateRender.svelte');
const live = read('src/lib/views/Live.svelte');

describe('the fit loop has a floor', () => {
  it('measures a ratio of the template’s own size, not an absolute point size', () => {
    // The unit is cqw — a share of the OUTPUT's width — so a template designed at
    // 6cqw is making a different claim from one designed at 3cqw. The question is
    // "did we have to shrink this beyond what its designer intended", and that is
    // a ratio. An absolute floor would flag a deliberately small template and miss
    // a badly overflowing large one.
    expect(render).toMatch(/MIN_LEGIBLE_SCALE\s*=\s*0?\.\d+/);
    const floor = Number(/MIN_LEGIBLE_SCALE\s*=\s*(0?\.\d+)/.exec(render)[1]);
    expect(floor).toBeGreaterThan(0.2);
    expect(floor).toBeLessThan(0.8);
  });

  it('still renders — it reports, it does not refuse', () => {
    // Blanking a screen because the text is small would be strictly worse for the
    // congregation, and refusing to render is not this component's call: Rust owns
    // refusals. The shrink loop must still run to completion.
    const fit = render.slice(render.indexOf('function fitOne'), render.indexOf('function fitText'));
    expect(fit).toMatch(/while \(keepShrinking\(\{ overflowing: overflows\(\), scale \}\)\)/);
    expect(fit).not.toMatch(/return;/);
    expect(fit).toMatch(/return scale;/);
  });

  it('is bounded by a SIZE, not by a round count', () => {
    // `guard < 40` bounded the loop at 0.95^40 = 0.1285, and a box needing less
    // than that got the last guess and kept it — still overflowing, inside an
    // `overflow: hidden` box. That is rule 42's sliced verse by a different road,
    // and it takes only one short line at a large designed size in a shallow box.
    // A count cannot express "small enough"; a scale can.
    const fit = render.slice(render.indexOf('function fitOne'), render.indexOf('function fitText'));
    expect(fit).not.toMatch(/while \(overflows\(\) && guard < 40\)/);
    expect(render).toMatch(/import \{[^}]*keepShrinking[^}]*\} from '\.\/templatemodel\.js'/);
  });

  it('reports the WORST slide on screen, not the last one fitted', () => {
    // During a crossfade two slides coexist. Reporting whichever happened to be
    // fitted last would call a shrunken verse legible half the time.
    expect(render).toMatch(/Math\.min\(worst, fitOne\(box, stageEl\)\)/);
  });

  it('a reporter that throws may not take the render down', () => {
    // This runs inside a requestAnimationFrame on the page that is ON THE WALL.
    const report = render.slice(render.indexOf('if (onFit)'));
    expect(report.slice(0, 300)).toMatch(/try \{/);
    expect(report.slice(0, 300)).toMatch(/catch/);
  });
});

describe('what Live tells the operator', () => {
  it('the program pane — which renders through the SAME component as the wall — listens', () => {
    // The measurement has to be the wall's, not a guess about it. The console
    // preview uses the identical renderer, which is what makes this honest.
    //
    // Anchored on the MARKUP, not on the first mention of the resolver: the
    // slide grid now renders every cell through that same resolver too, and a
    // slice taken from the first textual hit silently moved to a thumbnail —
    // which would still have passed had the thumbnails been the ones reporting.
    const from = live.indexOf('<section class="pane mon prog"');
    expect(from, 'the program pane is still identifiable in the markup').toBeGreaterThan(-1);
    const pane = live.slice(from, live.indexOf('</section>', from));
    // The resolution moved OUT of the markup into one reactive value, because the
    // pane now branches on whether a template resolved at all (it used to draw an
    // ON AIR frame over a black rectangle and say nothing). Two calls could
    // disagree, so there is one — and the claim this test makes is unchanged:
    // the programme renders through the SAME resolution the wall does.
    expect(pane).toMatch(/template=\{progTpl\}/);
    expect(live).toMatch(/\$: progTpl = resolveOutputTemplate\(previewTpl, \$liveTemplateOverride/);
    expect(pane).toMatch(/onFit=\{noteFit\}/);
    // And there is exactly ONE of it. Twenty thumbnails each reporting their own
    // fit would bury the one report that is about a congregation's screen.
    expect([...live.matchAll(/onFit=\{noteFit\}/g)]).toHaveLength(1);
  });

  it('says how small it went, and what to do about it', () => {
    expect(live).toMatch(/may not be readable from the back/);
    expect(live).toMatch(/Math\.round\(f\.scale \* 100\)/);
  });

  it('warns when nothing is showing what is on air — and never blocks the fire', () => {
    // REPORTED, never enforced. A service runs on the console preview alone all
    // the time (setup, rehearsal, someone re-cabling a projector), and refusing to
    // fire because no screen is attached would take the operator's tool away at
    // the exact moment they are fixing the screen.
    expect(live).toMatch(/nowhereToShow/);
    expect(live).toMatch(/Relay is still sending/);
    // It must be a passive notice, not a guard on any fire path.
    expect(live).not.toMatch(/if \(nowhereToShow\) return/);
  });

  it('the notice is neither amber nor a panic colour', () => {
    // Amber means ON AIR (DECISIONS §22) and Relay IS still sending — calling this
    // a failure would overstate it, and calling it on-air would be a lie.
    const styles = live.slice(live.indexOf('.out-warn{'));
    expect(styles.slice(0, 260)).not.toMatch(/--v-amber/);
    expect(styles.slice(0, 260)).toMatch(/--v-dim/);
  });
});

describe('the gate is at the one door, not at the callers', () => {
  it('every broadcast goes through preflight, because there is only one way out', () => {
    // A validator added at five call sites is a validator that will be missing
    // from the sixth. This repository has produced four separate bugs of exactly
    // that shape, so the check lives where `broadcast_content`'s single caller is.
    const main = read('src-tauri/src/main.rs');
    expect(main).toMatch(/pipeline::preflight\(&content\)/);
    // Exactly one call to the channels-level broadcast, and it is inside the
    // function that preflights.
    const calls = main.match(/channels::broadcast_content\(/g) ?? [];
    expect(calls.length).toBe(1);
  });

  it('a refused payload is never followed by a detection event saying it went out', () => {
    // That would be the console reporting a success it did not achieve, in a new
    // place (DECISIONS §20).
    const main = read('src-tauri/src/main.rs');
    expect(main).toMatch(/broadcast_with_clock\(handle, fire\.output\(\)\)\.is_err\(\)/);
  });

  it('the panic controls do not pass through it at all', () => {
    // A validator that could refuse a clear or a blackout would be a panic control
    // that can fail. They call channels::clear / channels::black directly.
    const main = read('src-tauri/src/main.rs');
    const clear = main.slice(main.indexOf('fn clear_or_report'));
    expect(clear.slice(0, 400)).not.toMatch(/preflight/);
  });
});

// ── A BLANK PROGRAMME SAYS WHY ──────────────────────────────────────────────
//
// Measured in a render on 2026-09-14: with no template resolved, the programme
// pane drew an amber `Program · On Air` frame over a black rectangle and said
// nothing — while the PREVIEW pane, in the identical situation one column left,
// said `No active template — activate one in Templates`. One pane explained
// itself and the other did not, and the silent one was the one claiming to be on
// air. A blank frame and a blackout look the same and are not the same fact.
describe('the programme pane never claims to be on air in silence', () => {
  const live = fs.readFileSync(path.join(ROOT, 'src/lib/views/Live.svelte'), 'utf8');
  const from = live.indexOf('<section class="pane mon prog"');
  const pane = live.slice(from, live.indexOf('</section>', from));

  it('renders only when a template actually resolved', () => {
    expect(pane).toMatch(/\{#if \$live && progTpl\}/);
  });

  it('and says what is wrong when one did not, naming the way out', () => {
    // The branch ONLY: the `{:else}` below it is the genuinely-clear case, whose
    // words are different on purpose.
    // The pane has TWO `{:else if $live}` — the header's ON AIR tag and this one.
    // Anchor on the screen body, or the assertion reads the wrong branch.
    const body = pane.slice(pane.indexOf('<div class="screen">'));
    const start = body.indexOf('{:else if $live}');
    const branch = body.slice(start, body.indexOf('{:else}', start));
    expect(branch).toMatch(/is on air, but no template is active/);
    expect(branch).toMatch(/activate one in Templates/);
    // Not the blackout's words, and not the clear's: three different facts.
    expect(branch).not.toMatch(/Screens clear/);
  });
});
