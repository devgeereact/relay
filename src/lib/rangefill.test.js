// The filled part of a slider is drawn by CSS from `--rp`, so `--rp` being
// right IS the control being right. `accent-color` used to do this, badly: it
// let every platform draw its own idea of a slider, and the sensitivity dial and
// a template's letter spacing ended up as two different instruments.
//
// What makes this worth a test rather than three lines in a component: the
// failure is silent and it looks like a design decision. A slider whose `--rp`
// is stale paints a filled track that disagrees with its own value — it says
// 50% while the number beside it says 80 — and nothing throws, nothing logs, and
// the operator reads the picture rather than the number.
//
// The third case below is the one that actually bit the prototype: a panel that
// rebuilds (a different template selected, a different screen inspected) sets
// the input's value from JavaScript, which fires NO `input` event. Listening to
// `input` alone is a painter that is correct until the moment the operator
// changes what they are looking at.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { rangeFill, fillPercent } from './rangefill.js';

/** A real `<input type=range>`, because the action reads min/max/value off it. */
function slider({ min = 0, max = 100, value = 50 } = {}) {
  const el = document.createElement('input');
  el.type = 'range';
  el.min = String(min);
  el.max = String(max);
  el.value = String(value);
  document.body.appendChild(el);
  return el;
}

const rp = (el) => el.style.getPropertyValue('--rp');

describe('fillPercent — the arithmetic on its own', () => {
  it('maps a value onto its share of the track', () => {
    expect(fillPercent(0, 0, 100)).toBe(0);
    expect(fillPercent(50, 0, 100)).toBe(50);
    expect(fillPercent(100, 0, 100)).toBe(100);
    expect(fillPercent(30, 10, 50)).toBe(50);
  });

  it('clamps rather than painting past the ends of the track', () => {
    expect(fillPercent(-20, 0, 100)).toBe(0);
    expect(fillPercent(999, 0, 100)).toBe(100);
  });

  it('a zero-width range is 0%, not NaN', () => {
    // `(v - min) / (max - min)` divides by zero here. NaN reaches CSS as an
    // invalid value, the declaration is dropped, and the track falls back to the
    // stylesheet's 50% default — a slider that reads half full at every value.
    expect(fillPercent(5, 5, 5)).toBe(0);
    expect(Number.isFinite(fillPercent(5, 5, 5))).toBe(true);
  });

  it('a non-numeric value is 0%, not NaN', () => {
    expect(fillPercent('', 0, 100)).toBe(0);
  });
});

describe('rangeFill — the action', () => {
  it('paints on mount, before anyone touches it', () => {
    const el = slider({ value: 25 });
    rangeFill(el);
    expect(rp(el)).toBe('25%');
  });

  it('repaints on input, which is what a drag is', () => {
    const el = slider({ value: 25 });
    rangeFill(el);
    el.value = '80';
    el.dispatchEvent(new Event('input'));
    expect(rp(el)).toBe('80%');
  });

  it('repaints when the panel rebuilds and sets the value in code', () => {
    // No `input` event is fired by assigning `.value`. Svelte calls `update()`
    // when the bound value changes, and that is the only signal there is.
    const el = slider({ value: 25 });
    const action = rangeFill(el);
    el.value = '90';
    action.update();
    expect(rp(el)).toBe('90%');
  });

  it('honours min and max rather than assuming 0-100', () => {
    const el = slider({ min: 1, max: 9, value: 5 });
    rangeFill(el);
    expect(rp(el)).toBe('50%');
  });

  it('stops listening when the control goes away', () => {
    const el = slider({ value: 10 });
    const action = rangeFill(el);
    action.destroy();
    el.value = '70';
    el.dispatchEvent(new Event('input'));
    expect(rp(el)).toBe('10%');
  });
});

// `--rp` being right is only half the control. The other half is the STYLESHEET
// reading it — and that half has no component to test, so it is tested as text.
//
// This is the merge that made it necessary. `new_look_refresh` styled every
// slider once, by element as well as by class, with the filled share drawn from
// `--rp`. `audit/field-2026-09-13` had independently fixed the same white-track
// defect with its own `input[type="range"]` block. Both survived the merge, the
// audit's block sat LATER in the file at equal specificity, and it won: every
// bare range input in Settings, Templates and Themes went back to a flat track
// that paints the same at every value. Nothing above this line would have
// noticed — `--rp` was still correct, and it was still being ignored.
describe('app.css — one slider block, and it stays the only one', () => {
  const css = readFileSync(resolve(__dirname, '../app.css'), 'utf8');

  /**
   * Every rule whose selector list names the bare `input[type=range]` ELEMENT —
   * the ones that set the control's own box. Pseudo-element rules
   * (`::-webkit-slider-thumb` and friends) are a different surface and there are
   * properly several of those.
   */
  const boxRules = () =>
    // A selector can hold no brace, so `[^{}]*` before the `{` IS the selector —
    // no need to anchor on the previous rule's `}`, and anchoring on it was
    // wrong: consuming that `}` made the scanner skip every other rule, and it
    // reported one block while two were present.
    [...css.matchAll(/([^{}]*)\{([^{}]*)\}/g)]
      .map((m) => ({ sel: m[1].trim(), body: m[2] }))
      .filter(({ sel }) =>
        sel.split(',').some((s) => /^input\[type=["']?range["']?\]$/.test(s.trim())),
      );

  it('sets the slider box exactly once', () => {
    expect(boxRules().map((r) => r.sel)).toHaveLength(1);
  });

  it('paints the filled share from --rp, so the track follows the value', () => {
    expect(css).toMatch(/slider-runnable-track[\s\S]{0,200}var\(--rp/);
  });

  it('gives the input a real box, not a bar-sized one', () => {
    // A 4px-tall input is a 4px pointer target, and a near-miss on a live
    // console lands on whatever is underneath it.
    expect(boxRules()[0].body).toMatch(/height:\s*18px/);
  });

  it('takes back the margin the browser gives a range input', () => {
    // Chromium's UA sheet sets `input[type=range]{ margin:2px; }` and
    // `appearance:none` does NOT clear it. Every slider in the product therefore
    // sat 2px inside the right edge its neighbours sat on — measured in the
    // Theme editor at two viewports: sliders ended at x=1251 and x=871 while the
    // select and the colour wells beside them ended at x=1253 and x=873.
    //
    // Two pixels cannot be seen by reading this stylesheet, which is exactly why
    // it is asserted here rather than trusted: the same class of defect as the
    // seven-pixel-wide screen name, and found the same way.
    expect(boxRules()[0].body).toMatch(/(?:^|;)\s*margin:\s*0(?:px)?\s*(?:;|$)/);
  });
});
