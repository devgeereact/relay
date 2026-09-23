// A PLAN CUE MAY NOT REDESIGN THE PREACHER'S SCREEN (RG-272).
//
// The operator, with a screenshot: *"When Planner is Loaded and clicked.... it
// changes everything on the stage display... is there a way to keep consistency
// on the stage display even if its planner or the main slide?"*
//
// **Why it happens.** A plan cue can carry its own `template_id`, and
// `cue_or_content_tpl` marks that choice `template_pinned`. DECISIONS §29 then
// says a pinned cue overrides the screen's own template — which is right for a
// congregation screen, because the operator picked that look for that item.
//
// A STAGE screen is not a congregation screen. It is an instrument: zones, a
// clock, a message, a reading, laid out for one person reading from a platform.
// A scripture template pinned for the projector took it over, so the preacher's
// monitor was redesigned mid-service by a decision that was never about it, and
// the layout the operator had assigned in Outputs vanished until the next cue.
//
// The rule is narrow: PINNED loses to a stage screen's own template, and only
// to a stage screen's. Everything else about §29 is untouched.
import { describe, it, expect } from 'vitest';
import { resolveOutputTemplate } from './layers.js';

const screenTpl = { id: 9, name: 'Stage layout', regions: [], style: {} };
const cueTpl = { id: 4, name: 'Classic Serif', regions: [], style: {} };

describe('a pinned cue template on a STAGE screen', () => {
  it('loses to the screen’s own template', () => {
    expect(resolveOutputTemplate(screenTpl, cueTpl, true, null, null, 'scripture', 'stage')).toBe(
      screenTpl,
    );
  });

  it('and a screen with no template of its own still takes the cue’s', () => {
    // NOT a refusal to paint. A stage screen that was never given a template
    // has nothing to protect, and showing the verse is better than showing
    // nothing at all.
    expect(resolveOutputTemplate(null, cueTpl, true, null, null, 'scripture', 'stage')).toBeTruthy();
  });
});

describe('and §29 is otherwise untouched', () => {
  it('a pinned cue still overrides a congregation screen', () => {
    // THE HALF THAT MUST NOT MOVE. The operator picked that look for that item,
    // and on the wall it wins.
    expect(resolveOutputTemplate(screenTpl, cueTpl, true, null, null, 'scripture', 'main')).toBe(
      cueTpl,
    );
    // No role at all is not a stage — a lobby TV and a stream feed both arrive
    // with none, and a filter whose default is yes is not a filter.
    expect(resolveOutputTemplate(screenTpl, cueTpl, true)).toBe(cueTpl);
  });

  it('an unpinned content look still defers to every screen', () => {
    expect(resolveOutputTemplate(screenTpl, cueTpl, false, null, null, 'scripture', 'stage')).toBe(
      screenTpl,
    );
    expect(resolveOutputTemplate(screenTpl, cueTpl, false, null, null, 'scripture', 'main')).toBe(
      screenTpl,
    );
  });
});

describe('the output page asks with its own role', () => {
  it('passes it, or the rule can never fire', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const out = readFileSync(resolve('src/Output.svelte'), 'utf8');
    const call = out.slice(out.indexOf('resolveOutputTemplate('));
    expect(call.slice(0, call.indexOf(')')), 'the page never says what it is').toMatch(/myRole/);
  });
});
