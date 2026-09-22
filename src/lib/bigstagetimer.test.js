// THE STAGE TIMER IS READABLE ON THE BIG SCREEN TOO (RG-265).
//
// The operator: *"the timer I asked for is only on mobile not yet on the stage
// display big screen... work on this and make sure this is visible enough for
// preacher to know what time to get ready"*.
//
// **It was there, and it was tiny.** RG-224 gave `output.html` a fallback
// programme rail for a stage-role screen wearing a template that declares no
// timer layer — full width, the bottom **8%**, with the figure a share of that.
// On a 1080p projector across a platform that is 86 pixels of rail for a clock
// the preacher is working to.
//
// **And the size control did not reach it.** RG-240 put Normal / Large / Huge on
// the stage layout in Outputs, `stage_zones` carries it, and `stage.html` honours
// it. `output.html` never asked: neither `Output.svelte` nor `TemplateRender`
// mentioned `stage_zones` or `timer_size` at all. So an operator who set Huge for
// the platform monitor moved the phone and nothing else — a control that reports
// success and changes nothing on the screen it was set for.
import { describe, it, expect } from 'vitest';
import { railScale, RAIL_BASE_PCT } from './bigstagetimer.js';

describe('railScale — how much of the screen the fallback rail takes', () => {
  it('Normal is a multiplier of one, and the base itself grew', () => {
    // THE BASE MOVED, 8 to 12, and that was the point rather than a side
    // effect. The first draft of this case asserted 8 on the instinct that an
    // install which never opens the control should render as it did yesterday —
    // but the operator's complaint IS about that install, at whatever it is set
    // to now. 8% of 1080 is 86 pixels of rail, read from ten metres.
    expect(railScale('normal')).toBe(1);
    expect(RAIL_BASE_PCT).toBe(12);
  });

  it('Large and Huge are the operator’s own steps, in order', () => {
    expect(railScale('large')).toBeGreaterThan(railScale('normal'));
    expect(railScale('huge')).toBeGreaterThan(railScale('large'));
  });

  it('and Huge is big enough to matter from a platform', () => {
    // 8% of 1080 is 86px. A step that moved it to 100px would be a control an
    // operator sets, squints at, and sets back.
    expect(RAIL_BASE_PCT * railScale('huge')).toBeGreaterThanOrEqual(16);
  });

  it('an unknown or absent size is Normal, never nothing', () => {
    // A layout saved before this key existed, or a frame from an older build.
    // A zero would take the rail off a screen the operator never touched.
    for (const v of [undefined, null, '', 'enormous', 7]) {
      expect(railScale(v), `${String(v)} did not fall back`).toBe(1);
    }
  });
});

describe('the wiring, which is the half that was missing', () => {
  const read = (p) => require('node:fs').readFileSync(require('node:path').resolve(p), 'utf8');

  it('Output listens for the stage layout and reads the size off it', () => {
    const src = read('src/Output.svelte');
    expect(src, 'the big screen still never asks for the layout').toMatch(/stage_zones/);
    expect(src).toMatch(/readTimerSize/);
  });

  it('and hands the renderer a scale rather than a raw setting', () => {
    // ONE READER of the setting. `TemplateRender` is shared with the console's
    // panes and the Templates editor, and a second place that knows what `huge`
    // means is a second place that can disagree with the phone.
    const src = read('src/Output.svelte');
    expect(src).toMatch(/timerScale=\{/);
    const tr = read('src/lib/TemplateRender.svelte');
    expect(tr).toMatch(/export let timerScale/);
    expect(tr, 'the renderer decided for itself what a size means').not.toMatch(/'huge'|"huge"/);
  });

  it('the fallback rail actually grows by it', () => {
    const tr = read('src/lib/TemplateRender.svelte');
    const rail = tr.slice(tr.indexOf('lprog lprog-default'), tr.indexOf('aria-label="Programme"', tr.indexOf('lprog lprog-default')));
    expect(rail, 'the rail is still a fixed 8%').toMatch(/railPct/);
  });
});
