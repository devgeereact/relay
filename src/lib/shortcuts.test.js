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
  const noop = { clearScreens: () => {}, blackScreen: () => {} };

  beforeEach(() => {
    cheatsheet.set(false);
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
