// HOW LONG IS LEFT OF THE CLIP, ON A STAGE TV (RG-280).
//
// The operator, third time of asking: *"Media remaining time not on the TV stage
// display still"*.
//
// **It was never there.** RG-256 put a frosted clip clock on `stage.html`, which
// is the phone. A stage screen served by `output.html` renders through
// `TemplateRender`, and `grep -c clipRemainingMs src/lib/TemplateRender.svelte`
// answered 0 — that page has never shown a clip countdown of any kind. The
// operator's own screenshot says `STAGE MONITOR attached`, so the screen they are
// describing is the one surface the feature had never reached.
//
// This is RG-265 again in a different coat: a thing built for the phone, asked
// for on the big screen, and the big screen never asked.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const TR = readFileSync(resolve('src/lib/TemplateRender.svelte'), 'utf8');
const OUT = readFileSync(resolve('src/Output.svelte'), 'utf8');

describe('a stage screen served by output.html', () => {
  it('times the clip it is showing', () => {
    expect(TR, 'the big screen still has no clip clock').toMatch(/clipRemainingMs/);
    expect(TR).toMatch(/class="lclip"/);
  });

  it('and only when this screen is a STAGE', () => {
    // A congregation must not be shown a countdown to the end of the clip they
    // are watching — it is the preacher's cue to get ready, not theirs.
    expect(TR).toMatch(/export let stageClip = false/);
    expect(TR).toMatch(/\{#if stageClip && clipLeft != null\}/);
    expect(OUT).toMatch(/stageClip=\{myRole === 'stage'\}/);
  });

  it('warns at the same thirty seconds the phone does', () => {
    // ONE threshold. `CLIP_WARN_MS` is named in `mediaclock.js` precisely
    // because two surfaces show this figure and a number typed twice will one
    // day differ between the stage and the desk.
    expect(TR).toMatch(/CLIP_WARN_MS/);
    const phone = readFileSync(resolve('src/Stage.svelte'), 'utf8');
    expect(phone).toMatch(/CLIP_WARN_MS/);
  });

  it('is frosted, for the reason the phone’s plate records', () => {
    // It sits over a moving picture. An opaque bar punches a hole through the
    // thing the room is watching, which is the trade RG-212 and RG-256 both
    // made, and the wash under the blur is what keeps a white figure off a
    // white frame where `backdrop-filter` is unsupported.
    const rule = TR.slice(TR.indexOf('.lclip {'), TR.indexOf('}', TR.indexOf('.lclip {')));
    expect(rule, 'no rule for .lclip').toBeTruthy();
    expect(rule).toMatch(/backdrop-filter:\s*blur\(/);
    expect(rule).toMatch(/background:/);
  });

  it('says nothing when there is no clip, and never a zero', () => {
    // `clipRemainingMs` answers null for every pre-knowledge state — a zero
    // reads as "it has finished" about a clip that has not started.
    // The reading is GUARDED by the role as well, so the regex allows a
    // condition between the two halves: a congregation screen must not compute
    // a countdown it is forbidden to show.
    expect(TR).toMatch(/clipLeft = [^;]*clipRemainingMs\(videoEl\)/);
  });
});
