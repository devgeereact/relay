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
    expect(fit).toMatch(/while \(overflows\(\) && guard < 40\)/);
    expect(fit).not.toMatch(/return;/);
    expect(fit).toMatch(/return scale;/);
  });

  it('reports the WORST slide on screen, not the last one fitted', () => {
    // During a crossfade two slides coexist. Reporting whichever happened to be
    // fitted last would call a shrunken verse legible half the time.
    expect(render).toMatch(/Math\.min\(worst, fitOne\(box\)\)/);
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
    const pane = live.slice(live.indexOf('resolveOutputTemplate(previewTpl'));
    expect(pane.slice(0, 400)).toMatch(/onFit=\{noteFit\}/);
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
