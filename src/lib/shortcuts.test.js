import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { installShortcuts, registerContext, cheatsheet, SHORTCUTS } from './shortcuts.js';

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

  it('documents the panic keys, so the help cannot drift from the bindings', () => {
    const labels = SHORTCUTS.flatMap((s) => s.keys);
    expect(labels).toContain('Esc');
    expect(labels).toContain('B');
    // And they are documented as always-on, which is the whole promise.
    expect(SHORTCUTS.find((s) => s.keys.includes('Esc')).scope).toBe('Always');
  });
});
