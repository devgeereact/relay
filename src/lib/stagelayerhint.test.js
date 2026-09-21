// A STAGE SCREEN SAYS WHEN RELAY IS PLACING ITS OWN LAYERS (RG-226).
//
// The operator, running the packaged build: *"I want to be able to confidure
// how the stage message is display from the templete editor"* — and it already
// can be: `stage_message` has been a layer binding in the editor's Content list
// the whole time, beside `programme`.
//
// So this was never a missing feature; it was a missing SENTENCE. A stage screen
// wearing a template with neither layer gets both placed for it — the message
// along the foot, and since RG-224 a default clock rail. That is the right
// behaviour, and silently doing the right thing is why an operator concluded the
// template was being ignored: the screen did not look like their design, and
// nothing anywhere said Relay had stepped in or how to take it back.
//
// The desk is where it is said, because the desk is where the screen was made a
// stage in the first place.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';
import { stagePlacement } from './stagelayers.js';

const CH = codeOnly(readFileSync(resolve(__dirname, 'views/Channels.svelte'), 'utf8'));

const tpl = (...binds) => ({
  layout: { layers: binds.map((b, i) => ({ id: `l${i}`, type: 'text', bind: b })) },
});

describe('stagePlacement — which of the two Relay is placing itself', () => {
  it('says both when the template declares neither', () => {
    expect(stagePlacement(tpl('verse', 'reference'))).toEqual({
      message: true,
      programme: true,
      any: true,
    });
  });

  it('says neither when the template declares both', () => {
    expect(stagePlacement(tpl('verse', 'stage_message', 'programme'))).toEqual({
      message: false,
      programme: false,
      any: false,
    });
  });

  it('answers per layer, because a template may declare one and not the other', () => {
    expect(stagePlacement(tpl('stage_message'))).toMatchObject({ message: false, programme: true });
    expect(stagePlacement(tpl('programme'))).toMatchObject({ message: true, programme: false });
  });

  it('a hidden layer is not a placement — the designer put it there and switched it off', () => {
    const hidden = { layout: { layers: [{ id: 'a', bind: 'programme', visible: false }] } };
    expect(stagePlacement(hidden).programme).toBe(true);
  });

  it('and a template it cannot read claims nothing', () => {
    // NOT "both": a null template is "we do not know yet", and a desk that says
    // Relay is placing two layers over a screen whose look has not loaded is a
    // sentence that will be wrong for a moment on every mount.
    for (const t of [null, undefined, {}, { layout: {} }])
      expect(stagePlacement(t).any, JSON.stringify(t)).toBe(false);
  });
});

describe('and the desk says it, where the screen was made a stage', () => {
  it('names both layers, so the operator knows what to add and where', () => {
    const at = CH.indexOf("sel.role === 'stage'");
    expect(at, 'the stage branch is gone').toBeGreaterThan(-1);
    const branch = CH.slice(at, at + 3000);
    expect(branch, 'nothing tells the operator Relay is placing these').toMatch(/stagePlacement|stagePlace/);
    expect(branch).toMatch(/Stage Message/);
    expect(branch).toMatch(/Programme/);
  });

  it('and it is a caution, never a colour that claims a screen', () => {
    // Rule 18: this is advice about a template, not a claim that anything is on
    // air. Amber means ON AIR, cyan means the AI guessed, amethyst means
    // rehearsal — none of the three is true of a sentence about a layer.
    const at = CH.indexOf('.ch-stagehint');
    expect(at, 'the hint has no rule of its own').toBeGreaterThan(-1);
    const rule = CH.slice(at, CH.indexOf('}', at));
    expect(rule).not.toMatch(/--v-amber|--v-cyan|--v-amethyst/);
  });
});
