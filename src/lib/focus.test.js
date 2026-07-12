// A keyboard operator must not be able to Tab out of a modal into the app behind it.
//
// Relay had ZERO focus traps across three dialogs. Opening the first-run wizard and
// pressing Tab walked straight past it into the console underneath — driving controls
// the operator cannot see, in a modal they cannot leave.
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { trapFocus } from './focus.js';

function dialogWith(n) {
  const app = document.createElement('button');
  app.id = 'behind';
  document.body.appendChild(app);

  const dlg = document.createElement('div');
  dlg.setAttribute('role', 'dialog');
  for (let i = 0; i < n; i++) {
    const b = document.createElement('button');
    b.id = `b${i}`;
    b.textContent = `b${i}`;
    dlg.appendChild(b);
  }
  document.body.appendChild(dlg);
  return { app, dlg };
}

function tab(shift = false) {
  const e = new KeyboardEvent('keydown', { key: 'Tab', shiftKey: shift, bubbles: true, cancelable: true });
  document.activeElement.dispatchEvent(e);
  return e;
}

describe('trapFocus', () => {
  beforeEach(() => (document.body.innerHTML = ''));
  afterEach(() => (document.body.innerHTML = ''));

  it('focuses the first real control, not the container', () => {
    const { dlg } = dialogWith(3);
    const t = trapFocus(dlg);
    expect(document.activeElement.id).toBe('b0');
    t.destroy();
  });

  // A trap whose dialog has been removed must stop grabbing Tab. (Found by a leaked
  // trap in this very file hijacking the next test's Tab — the same thing would happen
  // to a real operator if a teardown were ever missed.)
  it('a trap on a detached dialog stops intercepting Tab', () => {
    const { dlg } = dialogWith(2);
    trapFocus(dlg); // deliberately NOT destroyed
    dlg.remove();

    const other = document.createElement('button');
    document.body.appendChild(other);
    other.focus();
    const e = tab();

    expect(e.defaultPrevented).toBe(false);
    expect(document.activeElement).toBe(other);
  });

  it('Tab off the LAST control wraps to the first — it never reaches the app behind', () => {
    const { dlg } = dialogWith(3);
    const t = trapFocus(dlg);

    document.getElementById('b2').focus();
    const e = tab();

    expect(e.defaultPrevented).toBe(true); // we took the key
    expect(document.activeElement.id).toBe('b0');
    t.destroy();
  });

  it('Shift+Tab off the FIRST control wraps to the last', () => {
    const { dlg } = dialogWith(3);
    const t = trapFocus(dlg);

    document.getElementById('b0').focus();
    tab(true);

    expect(document.activeElement.id).toBe('b2');
    t.destroy();
  });

  it('Tab in the MIDDLE is left alone — the trap must not fight normal navigation', () => {
    const { dlg } = dialogWith(3);
    const t = trapFocus(dlg);

    document.getElementById('b0').focus();
    const e = tab();

    expect(e.defaultPrevented).toBe(false); // the browser moves focus itself
    t.destroy();
  });

  // The half everyone forgets, and the half a keyboard operator notices: without it,
  // focus falls to <body> and the next Tab restarts from the top of the app instead of
  // from the button they just pressed.
  it('gives focus BACK to whatever opened the dialog', () => {
    const { app, dlg } = dialogWith(2);
    app.focus();
    expect(document.activeElement.id).toBe('behind');

    const t = trapFocus(dlg);
    expect(document.activeElement.id).toBe('b0');

    t.destroy();
    expect(document.activeElement.id).toBe('behind');
  });

  it('does not throw if the opener was removed while the dialog was up', () => {
    const { app, dlg } = dialogWith(2);
    app.focus();
    const t = trapFocus(dlg);
    app.remove();
    expect(() => t.destroy()).not.toThrow();
  });

  // An informational dialog with nothing to click must still hold focus, or Tab
  // silently walks into the app.
  it('an empty dialog still keeps focus inside itself', () => {
    const { dlg } = dialogWith(0);
    const t = trapFocus(dlg);
    expect(document.activeElement).toBe(dlg);

    const e = tab();
    expect(e.defaultPrevented).toBe(true);
    t.destroy();
  });

  // Escape is a PANIC KEY (it clears the congregation's screens). Exactly one place
  // decides who consumes it — shortcuts.js — and the trap must not be a second opinion.
  it('does NOT consume Escape', () => {
    const { dlg } = dialogWith(2);
    const t = trapFocus(dlg);

    const e = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    document.activeElement.dispatchEvent(e);

    expect(e.defaultPrevented).toBe(false);
    t.destroy();
  });
});
