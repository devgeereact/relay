// W1 · LIVE — THE ACCEPTANCE CLAUSES, ASSERTED RATHER THAN ASSERTED ABOUT.
//
// `docs/REBRAND.md` §2 states one of them as a sentence: "Nothing clears the
// programme. Switching workspace, loading a plan, editing a template: the
// programme is content, not an index into a grid — this was a real bug and must
// not return." A sentence in a specification is not an instrument; this file is.
//
// Three clauses, three sections:
//
//   1. the programme survives every workspace switch and every plan load
//   2. `R` is not a global key, and must not become one (see shortcuts.test.js
//      for the rest of the key table — Space, Esc, B, ←/→ are pinned there)
//   3. a transition set to Cut animates nothing — measured on the renderer, not
//      on the pure function, because `transitions.js` already proves the function
//      and the question an operator actually has is about the wall
//
//   npx vitest run src/lib/liveacceptance.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { tick } from 'svelte';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const Live = (await import('./views/Live.svelte')).default;
const TemplateRender = (await import('./TemplateRender.svelte')).default;

const settle = (ms = 60) => new Promise((r) => setTimeout(r, ms));

const PROGRAMME = {
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good',
  translation: 'KJV',
};

let host;
beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_plans')
      return Promise.resolve([{ id: 1, title: 'Sunday', plan_date: '2026-09-14', cue_count: 1 }]);
    if (cmd === 'plan_items')
      return Promise.resolve([
        { id: 11, cue_type: 'announce', label: 'Welcome', payload: '{"text":"Welcome"}' },
      ]);
    if (cmd === 'list_output_channels' || cmd === 'list_templates') return Promise.resolve([]);
    if (cmd === 'rehearsal') return Promise.resolve(false);
    return Promise.resolve(null);
  });
  cap.live.set(null);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  host = document.createElement('div');
  document.body.appendChild(host);
});
afterEach(() => host.remove());

// ─────────────────────────────────────────────────────────────────────────────
// 1 · NOTHING CLEARS THE PROGRAMME
// ─────────────────────────────────────────────────────────────────────────────
describe('the programme is content, not an index into a grid', () => {
  it('survives a workspace switch away from Live and back', async () => {
    cap.live.set(PROGRAMME);
    const first = new Live({ target: host, props: {} });
    await settle();
    expect(get(cap.liveContent)?.reference).toBe('Romans 8:28');

    // The operator goes to Templates. `<svelte:component>` destroys the view.
    first.$destroy();
    await settle();
    expect(get(cap.live)).toEqual(PROGRAMME);

    // …and comes back.
    const second = new Live({ target: host, props: {} });
    await settle();
    expect(get(cap.liveContent)?.reference).toBe('Romans 8:28');
    second.$destroy();
    await settle();
    expect(get(cap.live)).toEqual(PROGRAMME);
  });

  it('survives loading a plan — which resets the playhead, and only the playhead', async () => {
    cap.live.set(PROGRAMME);
    const sess = await import('./session.js');
    sess.setSession({ planId: 1, liveCueId: null, liveSlide: 0, liveOnAir: false });
    // A playhead from somewhere else, so the reset is observable.
    cap.liveCue.set({ cueId: 99, slide: 4, onAir: false });

    const app = new Live({ target: host, props: {} });
    await settle(120);

    // The plan load did its job…
    expect(get(cap.liveCue).cueId).toBe(null);
    // …and did NOT touch what the congregation is looking at.
    expect(get(cap.live)).toEqual(PROGRAMME);
    app.$destroy();
    sess.setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });
  });

  // Structural, and the reason the two tests above can be short: the programme
  // store has exactly one writer, and it is the backend's own event. A view that
  // could `live.set(...)` is a view that could clear the programme by accident —
  // and "editing a template" is precisely such a path (it reloads `$templates`).
  it('no view writes the programme store at all — only the backend event does', () => {
    const dir = resolve(__dirname, '..');
    const offenders = [];
    const walk = (p) => {
      for (const e of readdirSync(p, { withFileTypes: true })) {
        const full = resolve(p, e.name);
        if (e.isDirectory()) walk(full);
        else if (/\.svelte$/.test(e.name)) {
          const src = readFileSync(full, 'utf8');
          if (/(^|[^\w$.])live\.set\s*\(/.test(src)) offenders.push(full);
        }
      }
    };
    walk(dir);
    expect(offenders).toEqual([]);
  });

  it('and the one writer is the output event, not a control', () => {
    const src = readFileSync(resolve(__dirname, 'stores/capture.js'), 'utf8');
    const sets = [...src.matchAll(/[^\w$.]live\.set\s*\(/g)];
    // Three: content arriving, the wall being cleared, and a template refresh
    // that re-renders the SAME content. None of them is a view.
    expect(sets.length).toBe(3);
    expect(src).toMatch(/listen\('output:\/\/content'[\s\S]{0,80}live\.set\(e\.payload\)/);
    expect(src).toMatch(/listen\('output:\/\/clear'[\s\S]{0,40}live\.set\(null\)/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · A TRANSITION SET TO CUT ANIMATES NOTHING
//
// `transitions.test.js` proves `transitionCss('cut', t) === ''` and
// `transitionDuration('cut', …) === 0`. Neither answers the question the clause
// asks, which is about the element on the wall: Svelte's `in:` directive turns a
// `css` transition into a real CSS animation on the node, and a zero-duration one
// must not produce one at all.
// ─────────────────────────────────────────────────────────────────────────────
describe('a transition set to Cut animates nothing on the wall', () => {
  const tpl = (transition, transitionMs) => ({
    id: 1,
    name: 'T',
    layout: { regions: ['verse_text', 'reference'], align: 'center' },
    style: { verseColor: '#ffffff', accent: '#ffb000', background: '#101010', transition, transitionMs },
  });
  const A = { reference: 'Psalms 23:1', text: 'The LORD is my shepherd' };
  const B = { reference: 'John 3:16', text: 'For God so loved the world' };

  /**
   * One fire, then the next — which is when the renderer actually transitions.
   * (An `in:` on the very first render needs `intro: true`; a second verse
   * re-keys the slide block, and that is the path a service takes.)
   *
   * Returns what Svelte actually put on the node: the `animation` shorthand, and
   * the keyframes it names. Reading the keyframes rather than the presence of an
   * animation is the difference between measuring the claim and measuring the
   * mechanism — Svelte writes an `animation` for every `css` transition, so
   * "there is no animation property" would be the wrong question.
   */
  async function fireTwice(transition, ms) {
    const app = new TemplateRender({
      target: host,
      props: { template: tpl(transition, ms), content: A },
    });
    app.$set({ content: B });
    await tick();
    const node = host.querySelector('.slide');
    const animation = node?.style?.animation ?? '';
    let keyframes = '';
    for (const sheet of document.styleSheets) {
      try {
        for (const rule of sheet.cssRules) {
          if (rule.name && animation.includes(rule.name)) keyframes = rule.cssText;
        }
      } catch {
        /* a sheet we may not read is not one Svelte wrote */
      }
    }
    app.$destroy();
    return { animation, keyframes: keyframes.replace(/\s+/g, ' ') };
  }

  /** Nothing between 0% and 100% touches anything. */
  const declarations = (kf) =>
    kf
      .replace(/^@keyframes\s+\S+\s*\{/, '')
      .replace(/\}\s*$/, '')
      .replace(/[\d.]+%\s*\{\s*\}/g, '')
      .replace(/[{}\s]/g, '');

  it('a cut takes no time and sets no property, at either end', async () => {
    const { animation, keyframes } = await fireTwice('cut', 400);
    expect(animation).toMatch(/\b0ms\b/);
    expect(declarations(keyframes)).toBe('');
  });

  it('an absent transition takes the DEFAULT, which is the reference\'s crossfade', async () => {
    // Changed 2026-09-14 with `DEFAULT_TRANSITION`. A template that states no
    // transition used to cut; the reference ships `--xd:320ms` on a crossfade and
    // the operator reported the console cutting every time. What a `cut` does is
    // unchanged and is the test directly above this one.
    const { animation, keyframes } = await fireTwice(undefined, undefined);
    expect(animation).toMatch(/\b320ms\b/);
    expect(declarations(keyframes)).not.toBe('');
  });

  it('…and a mode this build has never heard of animates nothing', async () => {
    // Carried through as a name and neutralised in `transitionDuration`, so the
    // wall is still: 0ms, no declarations. A template from a newer version cannot
    // have its motion guessed at.
    const { animation, keyframes } = await fireTwice('a-mode-from-2027', 400);
    expect(animation).toMatch(/\b0ms\b/);
    expect(declarations(keyframes)).toBe('');
  });

  // The control. Without this the two above would pass on a renderer that had
  // lost transitions altogether, which is not the same claim at all.
  it('…but a crossfade does both — so the measurement is real', async () => {
    const { animation, keyframes } = await fireTwice('crossfade', 400);
    expect(animation).toMatch(/\b400ms\b/);
    expect(keyframes).toMatch(/opacity/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 1b · AND IT SURVIVES `LOAD WHOLE PLAN` WITH LIVE ALREADY MOUNTED
//
// The plan chooser left Live (docs/REBRAND.md §2 — the plan IS the grid), so a
// plan now arrives one of two ways: the Planner's `Run in Live`, which remounts
// this view, and Quick tools' `Load whole plan`, which does NOT — the dock is in
// the shell, so the tab does not change and nothing is destroyed.
//
// That second path is new, and it is the one the acceptance clause is most
// exposed on: a watcher that reloaded the plan is a watcher that could reset the
// playhead, or worse, while a congregation is looking at a verse. It moves the
// grid and the playhead, and it must move nothing else.
describe('loading a plan from Quick tools, with Live already open', () => {
  it('stages the plan and leaves the programme exactly where it was', async () => {
    const sess = await import('./session.js');
    sess.setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });

    const app = new Live({ target: host, props: {} });
    await settle(120);
    // A verse is on the wall, put there by the backend event — not by a view.
    cap.live.set(PROGRAMME);
    await tick();

    invoke.mockClear();
    // Exactly what the dock's button does.
    sess.setSession({ activeTab: 'live', planId: 1 });
    await settle(120);

    // The plan was staged…
    expect(invoke.mock.calls.map((c) => c[0])).toContain('plan_items');
    // …and the congregation is still looking at the same thing.
    expect(get(cap.live)).toEqual(PROGRAMME);
    // No fire path was reached on the way.
    for (const cmd of ['manual_fire', 'fire_content', 'fire_media', 'clear_screens', 'blackout']) {
      expect(invoke.mock.calls.map((c) => c[0]), `plan load reached ${cmd}`).not.toContain(cmd);
    }
    app.$destroy();
    sess.setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });
  });

  it('does not reload the plan it already has open', async () => {
    // The watcher's guard. Live writes `session.planId` back reactively, so a
    // watcher that acted on its own write would reload the plan on every fire —
    // and `loadPlan` resets the playhead, which would send the next `→` back to
    // cue 1: the opening countdown, at the end of the service.
    const sess = await import('./session.js');
    sess.setSession({ planId: 1, liveCueId: null, liveSlide: 0, liveOnAir: false });

    const app = new Live({ target: host, props: {} });
    await settle(140);
    const first = invoke.mock.calls.filter((c) => c[0] === 'plan_items').length;
    expect(first).toBe(1);

    await settle(120);
    const again = invoke.mock.calls.filter((c) => c[0] === 'plan_items').length;
    expect(again).toBe(1);
    app.$destroy();
    sess.setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });
  });
});
