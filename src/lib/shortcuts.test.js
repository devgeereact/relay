import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  installShortcuts,
  registerContext,
  cheatsheet,
  SHORTCUTS,
  liveShortcuts,
} from './shortcuts.js';

function press(key, target = document.body) {
  const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
  Object.defineProperty(e, 'target', { value: target });
  window.dispatchEvent(e);
  return e;
}

describe('global panic keys', () => {
  let clearScreens, blackScreen, teardown;

  beforeEach(() => {
    document.body.innerHTML = '';
    cheatsheet.set(false);
    registerContext({}); // no view mounted
    clearScreens = vi.fn();
    blackScreen = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen });
  });

  // The listener is on `window`, which jsdom shares across tests — leaving it
  // installed would double-fire every handler in the next test.
  afterEach(() => teardown?.());

  // The bug: Escape was bound per-view, so on the Templates/Library/Settings tabs
  // the operator's panic key did nothing at all.
  it('Escape clears the screens even when no view has registered anything', () => {
    press('Escape');
    expect(clearScreens).toHaveBeenCalledOnce();
  });

  it('B blacks out the screens with no view registered', () => {
    press('b');
    expect(blackScreen).toHaveBeenCalledOnce();
  });

  // If the wrong thing is in front of a congregation, the operator must not first
  // have to work out which text box their cursor is in.
  it('Escape still fires while typing, and blurs the field', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();
    const blur = vi.spyOn(input, 'blur');

    press('Escape', input);

    expect(clearScreens).toHaveBeenCalledOnce();
    expect(blur).toHaveBeenCalled();
  });

  it('every other key yields to text entry — typing "b" must not black out the room', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);
    press('b', input);
    expect(blackScreen).not.toHaveBeenCalled();
  });

  it('a textarea counts as typing too', () => {
    const ta = document.createElement('textarea');
    document.body.appendChild(ta);
    press('b', ta);
    expect(blackScreen).not.toHaveBeenCalled();
  });

  it('leaves OS shortcuts alone (Cmd/Ctrl combos are not ours)', () => {
    const e = new KeyboardEvent('keydown', { key: 'b', metaKey: true, bubbles: true });
    Object.defineProperty(e, 'target', { value: document.body });
    window.dispatchEvent(e);
    expect(blackScreen).not.toHaveBeenCalled();
  });
});

describe('context actions', () => {
  let teardown, unregister;
  // Spies rather than bare no-ops: the `R` test below has to be able to say that
  // the panic controls did NOT fire, which is half of what it claims.
  const noop = { clearScreens: vi.fn(), blackScreen: vi.fn() };

  beforeEach(() => {
    cheatsheet.set(false);
    noop.clearScreens.mockClear();
    noop.blackScreen.mockClear();
    teardown = installShortcuts(noop);
  });

  afterEach(() => {
    unregister?.();
    teardown?.();
  });

  // The other half of the bug: Space meant "next slide" in the Planner and "push
  // the AI's guess to the congregation" on the Console. Same key, two meanings,
  // one of them irreversible in front of an audience.
  it('Space means advance, and never accept', () => {
    const next = vi.fn();
    const accept = vi.fn();
    unregister = registerContext({ next, accept });

    press(' ');

    expect(next).toHaveBeenCalledOnce();
    expect(accept).not.toHaveBeenCalled();
  });

  // RULE 11'S ONE EXCEPTION, on the operator's decision (2026-09-15).
  //
  // Space is the platform's activation key for a focused button, and `isTyping`
  // only excluded text fields — so a keyboard operator who tabbed to `Rehearse`,
  // `Blackout` or `Clear screens` and pressed Space advanced the programme
  // instead of pressing the control under their finger, and `preventDefault`
  // suppressed the button's own click. On the run surface that is a key doing
  // something other than what the focused control says, which is the shape rule
  // 11 exists to stop rather than an application of it.
  //
  // The arrows are deliberately NOT narrowed, so the transport is never lost.
  // Each of these was watched to fail against the un-narrowed branch.
  describe('Space yields to a control that has focus', () => {
    it('a focused button keeps its own activation', () => {
      const next = vi.fn();
      unregister = registerContext({ next });
      const btn = document.createElement('button');
      document.body.appendChild(btn);

      press(' ', btn);

      expect(next, 'the programme must not advance from a focused button').not.toHaveBeenCalled();
      btn.remove();
    });

    it('…and so does anything wearing a button or switch role', () => {
      const next = vi.fn();
      unregister = registerContext({ next });
      for (const role of ['button', 'switch']) {
        const el = document.createElement('div');
        el.setAttribute('role', role);
        document.body.appendChild(el);
        press(' ', el);
        el.remove();
      }
      expect(next).not.toHaveBeenCalled();
    });

    it('but a DISABLED button activates nothing, so the transport keeps the key', () => {
      const next = vi.fn();
      unregister = registerContext({ next });
      const btn = document.createElement('button');
      btn.disabled = true;
      document.body.appendChild(btn);

      press(' ', btn);

      expect(next).toHaveBeenCalledOnce();
      btn.remove();
    });

    it('the ARROWS are not narrowed — the transport works from a focused button', () => {
      const next = vi.fn();
      const prev = vi.fn();
      unregister = registerContext({ next, prev });
      const btn = document.createElement('button');
      document.body.appendChild(btn);

      press('ArrowRight', btn);
      press('ArrowLeft', btn);

      expect(next, 'ArrowRight must still advance from anywhere').toHaveBeenCalledOnce();
      expect(prev, 'ArrowLeft must still step back from anywhere').toHaveBeenCalledOnce();
      btn.remove();
    });

    it('and Escape still clears the screens from a focused button', () => {
      // The narrowing must not touch a panic path. Rule 15 outranks all of this.
      unregister = registerContext({ next: vi.fn() });
      const btn = document.createElement('button');
      document.body.appendChild(btn);

      press('Escape', btn);

      expect(noop.clearScreens).toHaveBeenCalledOnce();
      btn.remove();
    });
  });

  it('accepting a suggestion has its own dedicated key', () => {
    const accept = vi.fn();
    const dismiss = vi.fn();
    unregister = registerContext({ accept, dismiss });

    press('a');
    expect(accept).toHaveBeenCalledOnce();

    press('d');
    expect(dismiss).toHaveBeenCalledOnce();
  });

  it('arrow keys drive the transport', () => {
    const next = vi.fn();
    const prev = vi.fn();
    unregister = registerContext({ next, prev });

    press('ArrowRight');
    press('ArrowLeft');

    expect(next).toHaveBeenCalledOnce();
    expect(prev).toHaveBeenCalledOnce();
  });

  it('a view that offers no action simply does nothing — no crash', () => {
    unregister = registerContext({});
    expect(() => press('a')).not.toThrow();
    expect(() => press(' ')).not.toThrow();
  });

  it('unregistering stops the view receiving keys after it unmounts', () => {
    const next = vi.fn();
    const stop = registerContext({ next });
    stop();
    press(' ');
    expect(next).not.toHaveBeenCalled();
  });

  // `R` IS NOT A KEY, AND MUST NOT BECOME ONE.
  //
  // The rebrand's Live brief lists it beside Space, Esc, B and the arrows, and the
  // obvious thing to bind it to is Rehearse — which is exactly why this test
  // exists. Rehearsal is the one toggle whose two states look like success from
  // the console and opposite from the room: entering it mid-service takes the
  // congregation's screens dark while everything in the booth still moves, and
  // leaving it by accident puts a practice run in front of people. A single
  // unmodified keystroke, on a surface where an operator's hand is already on the
  // keyboard, is not a proportionate control for that. Rehearse stays a button
  // that reports its own failure (CLAUDE.md rule 15 · DECISIONS §20).
  //
  // Stated as a test rather than a comment because the next person to read the
  // brief will reach for `case 'r':` and this is what tells them not to.
  it('R does nothing — not accept, not advance, and above all not rehearse', () => {
    const calls = {
      accept: vi.fn(),
      dismiss: vi.fn(),
      next: vi.fn(),
      prev: vi.fn(),
      search: vi.fn(),
    };
    unregister = registerContext(calls);
    press('r');
    press('R');
    for (const fn of Object.values(calls)) expect(fn).not.toHaveBeenCalled();
    // And it is not a panic key wearing another letter.
    expect(noop.clearScreens).not.toHaveBeenCalled();
    expect(noop.blackScreen).not.toHaveBeenCalled();
  });

  it('…and the table does not advertise it', () => {
    expect(SHORTCUTS.some((s) => s.keys.some((k) => k.toLowerCase() === 'r'))).toBe(false);
  });
});

describe('cheatsheet', () => {
  it('? toggles it, Escape closes it', () => {
    const stop = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
    cheatsheet.set(false);

    press('?');
    expect(get(cheatsheet)).toBe(true);

    press('Escape');
    expect(get(cheatsheet)).toBe(false);
    stop();
  });

  // THE BUG: Escape ran clearScreens() unconditionally and *also* closed the
  // overlay. So an operator who pressed `?` mid-service to check a binding, then
  // pressed Escape to put the help away, wiped the congregation's screens — with no
  // idea they had done it. Closing a read-only help panel is not a live action.
  it('Escape closes the cheatsheet WITHOUT clearing the congregation’s screens', () => {
    const clearScreens = vi.fn();
    const stop = installShortcuts({ clearScreens, blackScreen: () => {} });
    cheatsheet.set(true);

    press('Escape');

    expect(get(cheatsheet)).toBe(false);
    expect(clearScreens).not.toHaveBeenCalled();
    stop();
  });

  // The cheatsheet was guarded; the arrangement pickers were not. Escape inside one
  // of those wiped the congregation's screens AND left the picker open — its Escape
  // handler was bound to the backdrop element, which does not hold focus, so it never
  // fired. The operator got the outcome they did not want and none of the one they did.
  it('Escape inside ANY open dialog does not clear the screens', () => {
    const clearScreens = vi.fn();
    const stop = installShortcuts({ clearScreens, blackScreen: () => {} });
    cheatsheet.set(false);

    const modal = document.createElement('div');
    modal.setAttribute('role', 'dialog');
    document.body.appendChild(modal);

    press('Escape');
    expect(clearScreens).not.toHaveBeenCalled();

    // ...and once the dialog is gone, Escape is the panic key again.
    modal.remove();
    press('Escape');
    expect(clearScreens).toHaveBeenCalledTimes(1);
    stop();
  });

  // THE 2026-08-14 AUDIT, P1-3. The guard above probed for `[role="dialog"]` and
  // nothing else, so SEVEN overlays were not covered: six popup menus and the
  // console crash panel. Escape in any of them wiped the congregation's screens and
  // left the overlay open — the identical failure the test above was written to end,
  // on the doors nobody enumerated.
  //
  // Table-driven on purpose: the bug was never "this one role was missed", it was
  // "the list was a list". Adding an overlay kind means adding a row here.
  it.each([
    // The crash panel. Its own copy says "Your output screens are still live", and
    // Escape is the reflex key for dismissing a modal — so this was the wall going
    // dark at the one moment the product guarantees the wall is hot.
    ['alertdialog', 'the console crash panel'],
    // Countdown on the run rail, the VerseDeck kebab, two template-gallery menus,
    // two template-editor menus.
    ['menu', 'a popup menu'],
    ['listbox', 'an open combobox list'],
  ])('Escape inside role="%s" (%s) does not clear the screens', (role) => {
    const clearScreens = vi.fn();
    const stop = installShortcuts({ clearScreens, blackScreen: () => {} });
    cheatsheet.set(false);

    const overlay = document.createElement('div');
    overlay.setAttribute('role', role);
    document.body.appendChild(overlay);

    press('Escape');
    expect(clearScreens).not.toHaveBeenCalled();

    // …and the guard must not cost the operator their panic key once it closes.
    overlay.remove();
    press('Escape');
    expect(clearScreens).toHaveBeenCalledTimes(1);
    stop();
  });

  it('but Escape with NO cheatsheet open is still the panic key', () => {
    // The guard above must not cost the operator their panic key.
    const clearScreens = vi.fn();
    const stop = installShortcuts({ clearScreens, blackScreen: () => {} });
    cheatsheet.set(false);

    press('Escape');

    expect(clearScreens).toHaveBeenCalledTimes(1);
    stop();
  });

  // The cheatsheet footer used to read "Esc and B work on every tab, even while
  // typing." The B half was false — and it is help text about a PANIC key, read
  // only under pressure. The behaviour is correct (typing "Habakkuk" must not black
  // out the room on the 'b'); it was the promise that was wrong. Pinned here so the
  // copy in App.svelte cannot quietly drift back.
  it('B does NOT fire while typing — the help must not claim otherwise', () => {
    const blackScreen = vi.fn();
    const stop = installShortcuts({ clearScreens: () => {}, blackScreen });
    const input = document.createElement('input');
    document.body.appendChild(input);

    press('b', input);

    expect(blackScreen).not.toHaveBeenCalled();
    stop();
  });

  it('documents the panic keys, so the help cannot drift from the bindings', () => {
    const labels = SHORTCUTS.flatMap((s) => s.keys);
    expect(labels).toContain('Esc');
    expect(labels).toContain('B');
    // And they are marked always-on, which is the whole promise: the panic keys
    // work on every surface, so they are listed on every surface.
    expect(SHORTCUTS.find((s) => s.keys.includes('Esc')).always).toBe(true);
    expect(SHORTCUTS.find((s) => s.keys.includes('B')).always).toBe(true);
  });
});

describe('the cheatsheet must not lie', () => {
  let teardown, unregister;

  beforeEach(() => {
    teardown = installShortcuts({ clearScreens: () => {}, blackScreen: () => {} });
  });
  afterEach(() => {
    unregister?.();
    teardown?.();
  });

  const keysIn = (list) => list.flatMap((s) => s.keys);

  // THE bug. The Planner registers only next/prev, so `A`, `D` and `/` were DEAD
  // KEYS on that tab — while the cheatsheet cheerfully listed all three. An
  // operator pressing `A` mid-service to push the AI's suggestion would have got
  // nothing, and no explanation.
  //
  // A help screen that lists a key which does nothing is worse than no help
  // screen: it teaches the operator something false, under pressure.
  it('does not advertise a key the current surface cannot handle', () => {
    unregister = registerContext({ next: () => {}, prev: () => {} }); // the Planner
    let shown;
    liveShortcuts.subscribe((v) => (shown = v))();

    const keys = keysIn(shown);
    expect(keys).not.toContain('A'); // no accept handler here
    expect(keys).not.toContain('D');
    expect(keys).not.toContain('/');
    expect(keys).toContain('→'); // next IS registered
  });

  it('advertises the keys a surface DOES handle', () => {
    unregister = registerContext({
      accept: () => {},
      dismiss: () => {},
      next: () => {},
      prev: () => {},
      search: () => {},
    }); // the Console
    let shown;
    liveShortcuts.subscribe((v) => (shown = v))();

    const keys = keysIn(shown);
    for (const k of ['A', 'D', '/', '→', '←']) expect(keys).toContain(k);
  });

  // The panic keys must be listed on EVERY surface, always — including one with
  // no context at all.
  it('always lists the panic keys, even with nothing registered', () => {
    unregister = registerContext({});
    let shown;
    liveShortcuts.subscribe((v) => (shown = v))();

    const keys = keysIn(shown);
    expect(keys).toContain('Esc');
    expect(keys).toContain('B');
    expect(keys).toContain('?');
  });

  it('every non-always shortcut declares which action it needs', () => {
    for (const s of SHORTCUTS) {
      if (!s.always) {
        expect(typeof s.needs).toBe('string');
      }
    }
  });
});

// 2026-09-21 · Live D1. The `[role=dialog]` guard lived INSIDE the Escape branch,
// so `A`, `D`, the arrows and Space kept acting on the view underneath an open
// dialog. Opening "Why this match?" on the third claim and pressing `A` accepted
// the FIRST claim — the keyboard half of the bug `Live.svelte` had fixed for the
// buttons. `→` was worse: it stepped the plan while the operator read a panel.
describe('context keys yield to an open dialog', () => {
  let clearScreens, blackScreen, teardown, ctx;

  beforeEach(() => {
    document.body.innerHTML = '<div role="dialog">why this match?</div>';
    cheatsheet.set(false);
    ctx = { accept: vi.fn(), dismiss: vi.fn(), next: vi.fn(), prev: vi.fn() };
    registerContext(ctx);
    clearScreens = vi.fn();
    blackScreen = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen });
  });
  afterEach(() => teardown?.());

  it('A, D, the arrows and Space act on nothing while a dialog is open', () => {
    for (const key of ['a', 'd', 'ArrowRight', 'ArrowLeft', 'PageDown', 'PageUp', ' ']) press(key);
    expect(ctx.accept).not.toHaveBeenCalled();
    expect(ctx.dismiss).not.toHaveBeenCalled();
    expect(ctx.next).not.toHaveBeenCalled();
    expect(ctx.prev).not.toHaveBeenCalled();
  });

  it('but B is a panic key and still blacks the room', () => {
    press('b');
    expect(blackScreen).toHaveBeenCalledOnce();
  });

  it('and with the dialog gone the keys come back', () => {
    document.body.innerHTML = '';
    press('a');
    press('ArrowRight');
    expect(ctx.accept).toHaveBeenCalledOnce();
    expect(ctx.next).toHaveBeenCalledOnce();
  });
});
