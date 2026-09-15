// X1 · THE OPERATOR'S TRANSITION OVERRIDE (docs/REBRAND.md §8 · DECISIONS §84).
//
// `transitions.test.js` holds the seven and the two ways an animation damages a
// service. This file holds the thing that was added on top of them: a SECOND
// authority over one property.
//
// That is the defect §3.1 of the rebrand spec exists to prevent, so what is
// asserted here is not "the override works" but the three things that make a
// second authority safe:
//
//   1. The order is stated once and only once — `resolveTransition`.
//   2. Which authority is answering is a fact the control can read back, so a
//      picker can say it rather than an operator having to guess.
//   3. A nonsense override falls through to the template rather than flattening a
//      working wall to a cut.
//
// And one more, which is the whole reason any of this reaches a congregation
// screen safely: the wall applies an override WITH content and never on its own.
import { describe, it, expect, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { get } from 'svelte/store';
import {
  TRANSITIONS,
  TRANSITION_MS,
  DEFAULT_TRANSITION,
  transitionDuration,
  DEFAULT_TRANSITION_MS,
  resolveTransition,
  isOverride,
  liveTransition,
} from './transitions.js';

const src = resolve(__dirname, '..');
const OUTPUT = readFileSync(resolve(src, 'Output.svelte'), 'utf8');
const RENDER = readFileSync(resolve(src, 'lib/TemplateRender.svelte'), 'utf8');
const CSS = readFileSync(resolve(src, 'app.css'), 'utf8');

describe('which transition is in force, and on whose authority', () => {
  beforeEach(() => {
    liveTransition.set(null);
  });

  it('falls to the template when nobody has overridden it — §71 untouched', () => {
    expect(resolveTransition({ transition: 'slideup', transitionMs: 500 }, null)).toEqual({
      mode: 'slideup',
      ms: 500,
      source: 'template',
    });
  });

  it('falls to the default when neither authority has said anything', () => {
    // The DEFAULT moved to `crossfade` on 2026-09-14 to match the reference. What
    // has NOT changed is the other half: a mode this build does not recognise is
    // still a cut, because guessing at motion a newer template asked for is the
    // one thing an override may not do.
    expect(resolveTransition({}, null).mode).toBe(DEFAULT_TRANSITION);
    expect(resolveTransition(undefined, undefined).mode).toBe(DEFAULT_TRANSITION);
    // An unknown mode is carried through as a NAME and neutralised where motion is
    // decided — `transitionDuration` returns 0 for anything outside the register —
    // so the resolver stays a pure ranking and the renderer stays the one place
    // that says what animates. Asserted as the EFFECT, not as the string.
    expect(transitionDuration('a-mode-from-2027', 400, false)).toBe(0);
  });

  it('lets the operator overrule a template that wanted motion', () => {
    // The case the whole control exists for: four templates in rotation, the
    // preacher off-plan, and "make everything cut, now". An override that could
    // not say `cut` would be an override that cannot do the one urgent thing.
    const r = resolveTransition({ transition: 'crossfade', transitionMs: 800 }, { mode: 'cut' });
    expect(r.mode).toBe('cut');
    expect(r.source).toBe('operator');
  });

  it('says WHICH authority answered, so a control can report it rather than guess', () => {
    expect(resolveTransition({ transition: 'dissolve' }, null).source).toBe('template');
    expect(resolveTransition({ transition: 'dissolve' }, { mode: 'pushleft' }).source).toBe('operator');
  });

  it('treats an override from a newer version as no override, NOT as a cut', () => {
    // A frame naming a mode this build has never heard of must not be able to
    // silently unstyle a wall that was working. Falling through to the template is
    // the conservative answer; flattening to a cut would be a stranger's message
    // changing what a congregation sees.
    const r = resolveTransition({ transition: 'materialise', transitionMs: 320 }, { mode: 'kenburns' });
    expect(r.mode).toBe('materialise');
    expect(r.source).toBe('template');
    expect(isOverride({ mode: 'kenburns' })).toBe(false);
  });

  it('offers exactly the seven and no eighth — the sentinel is not a transition', () => {
    // `FOLLOW` in App.svelte is the empty string. If it were ever added to
    // TRANSITIONS there would be two registers of what a transition is, and one of
    // them would be the one somebody read.
    expect(isOverride({ mode: '' })).toBe(false);
    expect(TRANSITIONS.map((t) => t.id)).toHaveLength(7);
    expect(TRANSITION_MS).toContain(DEFAULT_TRANSITION_MS);
    expect(TRANSITION_MS[0]).toBe(0);
  });
});

describe('the renderer ranks the two, and replays only what an operator chose', () => {
  beforeEach(() => {
    liveTransition.set(null);
  });

  it('keys the slide on the OVERRIDE and not on the resolved mode', () => {
    // Choosing one replays it at once (§8) — that is what keying on the override
    // buys. Keying on the RESOLVED mode would also replay whenever a live template
    // edit arrives, which pushes a new `template` frame to every screen: a verse
    // already up on the wall would re-animate because somebody saved a template.
    expect(RENDER).toMatch(/overrideKey = isOverride\(activeOverride\)/);
    expect(RENDER).toMatch(/slideKey = .*\$\{overrideKey\}`/);
    expect(RENDER).not.toMatch(/slideKey = .*resolvedTransition/);
  });

  it('lets an explicit prop beat the store, so the wall is never read from a console store', () => {
    expect(RENDER).toMatch(
      /activeOverride = transitionOverride === undefined \? \$liveTransition : transitionOverride/,
    );
  });

  it('resolves in ONE place — no surface may rank the two for itself', () => {
    const ranks = RENDER.match(/resolveTransition\(/g) ?? [];
    expect(ranks).toHaveLength(1);
    expect(get(liveTransition)).toBe(null);
  });
});

describe('a congregation screen applies an override WITH content, never on its own', () => {
  it('snapshots the override at content time on both doors', () => {
    // The rule that keeps a preference change from repainting a wall. The console
    // replays at once because that is the point of the picker; a congregation
    // screen must not, or an operator adjusting a dropdown makes a verse that is
    // already up re-animate, mid-reading, in front of people.
    //
    // Asserted on the source because the two lines ARE the rule: there is nothing
    // pure to extract, and a component test of `output.html` would need the whole
    // Tauri bridge and the WS hub to reach three assignments.
    const applies = OUTPUT.match(/appliedTransition = pendingTransition;/g) ?? [];
    expect(applies).toHaveLength(2); // the kiosk frame path and the Tauri event path
    expect(OUTPUT).toMatch(/transitionOverride=\{appliedTransition\}/);
  });

  it('never applies one on the transition frame itself', () => {
    const branch = OUTPUT.slice(
      OUTPUT.indexOf("m.kind === 'transition'"),
      OUTPUT.indexOf("m.kind === 'clear'"),
    );
    expect(branch).toContain('noteTransition(m.mode, m.ms)');
    expect(branch).not.toContain('appliedTransition');
  });

  it('leaves both panic controls untouched by the override', () => {
    // A blackout that could fade is a blackout that can be late (rule 15,
    // DECISIONS §20). Neither branch may read or move the override, and the
    // renderer has no `out:` transition at all, so a clear is instant at every
    // duration the picker offers.
    // Comments stripped: a comment about the rule is prose, and only the code is
    // the claim. The first version of this assertion failed on its own comment.
    const panic = OUTPUT.slice(
      OUTPUT.indexOf("m.kind === 'clear'"),
      OUTPUT.indexOf("m.kind === 'channel_template'"),
    ).replace(/\/\/.*$/gm, '');
    expect(panic).toContain('visible = false');
    expect(panic).toContain('black = true');
    expect(panic).not.toContain('Transition');
    expect(RENDER).not.toMatch(/out:slideIn/);
  });
});

describe('L4 · the picker is in the take rack, and the take is never below it', () => {
  const LIVE = readFileSync(resolve(src, 'lib/views/Live.svelte'), 'utf8');
  const APP = readFileSync(resolve(src, 'App.svelte'), 'utf8');

  // WHY THIS DESCRIBE CHANGED. The control used to share the right end of a 34px
  // chrome bar with the screen lamps and Emergency Stop, so what had to be proved
  // was that it disappeared before the panic control moved. It is now in Live's
  // take rack, beside the transport whose look it changes, and the claim that
  // replaces it is the same one in the new room: a picker may never sit between an
  // operator's hand and the two controls that put content on a wall.
  it('has left the chrome bar entirely, leaving no second copy behind', () => {
    const chrome = APP.slice(APP.indexOf('<header class="topbar-v">'), APP.indexOf('</header>'));
    expect(chrome).not.toContain('xfade');
    expect(chrome).not.toContain('Slide transition');
    // And nothing in the shell still drives it. A picker that relocated but left
    // its command behind is the "two authorities" defect one level down.
    expect(APP).not.toContain('setLiveTransition');
    expect(APP).not.toContain('TRANSITIONS');
    // `loadLiveTransition` STAYS in the shell's mount, deliberately: the override
    // is a backend fact every surface reads, including the five workspaces that
    // draw no picker, and a console reopened mid-service must not disagree with
    // screens that are already crossfading (rule 35).
    //
    // COMMENTS STRIPPED. The first version of this line matched the whole file,
    // and the paragraph three lines above this one contains the call verbatim —
    // so commenting the real call out left the assertion passing over a shell
    // that no longer read the override at all. Only the code is the claim.
    const code = APP.replace(/\/\/.*$/gm, '').replace(/<!--[\s\S]*?-->/g, '');
    expect(code).toMatch(/loadLiveTransition\(\)/);
  });

  it('is the LAST band in the rack, under TAKE and under the arrows', () => {
    const rack = LIVE.slice(LIVE.indexOf('<aside class="rack">'), LIVE.indexOf('</aside>'));
    expect(rack).toContain('aria-label="Slide transition"');
    expect(rack).toContain('aria-label="Transition duration"');
    // The order is the claim. TAKE and `Next` are what a hurried hand reaches for
    // without reading; a dropdown above either of them is a dropdown that gets
    // opened by mistake in front of a congregation.
    expect(rack.indexOf('>TAKE<')).toBeLessThan(rack.indexOf('rk-x'));
    expect(rack.indexOf('Next ›')).toBeLessThan(rack.indexOf('rk-x'));
  });

  it('cannot grow tall enough to push the take out of the rack', () => {
    // The rack is `align-self:start` and each band pads itself; the two selects
    // are STACKED at a fixed 22px rather than laid across a 118px column, which
    // is what keeps the band's height a constant rather than a function of the
    // longest option label.
    const style = LIVE.slice(LIVE.indexOf('<style>'));
    expect(style).toMatch(/\.rack\{[^}]*align-self:start/);
    expect(style).toMatch(/\.rk-x \.xpick\{[^}]*width:100%[^}]*height:22px/);
  });

  it('does not claim a colour the law has already spoken for', () => {
    // amber = on air, amethyst = rehearsal, cyan = a guess, grey = cued. An
    // override in force is none of those four, so it says so with the accent line
    // and never borrows a state colour it is not entitled to.
    const style = LIVE.slice(LIVE.indexOf('<style>'));
    const band = style.slice(style.indexOf('  .rk-x .xcap{'), style.indexOf('  .ibtn{'));
    const rules = band.replace(/\/\*[\s\S]*?\*\//g, '');
    expect(rules).not.toMatch(/--v-amber|--v-amethyst|--v-cyan/);
    expect(rules).toMatch(/\.rk-x\.on \.xpick\{border-color:var\(--v-accent-line\)/);
  });

  it('left no dead rule behind it in the shared stylesheet', () => {
    // A block in `app.css` that no element matches any more is the thing the next
    // agent reads and believes. The X1 heading survives as a tombstone that says
    // where the control went; the rules did not.
    const block = CSS.slice(CSS.indexOf('X1 · THE TRANSITION CONTROL'));
    const next = block.indexOf('B1 · ONE BUTTON');
    const x1 = block.slice(0, next === -1 ? undefined : next);
    expect(x1).not.toMatch(/\.xfade\{/);
    expect(x1).not.toMatch(/@media \(max-width:1180px\)/);
    expect(x1).toMatch(/MOVED/);
  });
});
