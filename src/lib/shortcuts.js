// Global keyboard registry — ONE window listener, mounted once at the app shell.
//
// Why this exists: the panic keys used to be bound per-view (Console and
// Planner each registered their own `window.addEventListener('keydown')` in
// onMount). That meant Escape — the operator's "get it off the screen NOW" key —
// did nothing at all while they happened to be on the Templates, Library, or
// Settings tab. A panic control that works on some tabs is not a panic control.
//
// It also meant `Space` did two different things depending on which tab was
// mounted ("confirm the AI's top suggestion" on Console, "next slide" in the
// Planner). Under pressure, in the dark, that is a genuinely dangerous ambiguity,
// so `Space` now has exactly one meaning app-wide: advance. Accepting an AI
// suggestion — which puts scripture in front of the congregation — got its own
// unambiguous key instead.
//
// CLAUDE.md: "Operator override is a first-class control, never a fallback UI.
// It must always be reachable in one action from the main console, at every
// stage." Global keys are how that promise is kept for the keyboard.

import { writable } from 'svelte/store';

/** Are we inside a text field? Typing must never trigger a live action. */
function isTyping(e) {
  const el = e.target;
  if (!el) return false;
  return (
    el.tagName === 'INPUT' ||
    el.tagName === 'TEXTAREA' ||
    el.tagName === 'SELECT' ||
    el.isContentEditable === true
  );
}

/**
 * Context handlers registered by whichever view is mounted. A view supplies only
 * the actions that make sense for it; the always-on keys (Escape, B) live in the
 * global table below and are NEVER delegated to a view, so they cannot go missing.
 *
 * Shape: { accept, dismiss, next, prev, search }  (each optional)
 */
let ctx = {};

/** Called by a view on mount. Returns an unregister fn for onDestroy. */
export function registerContext(handlers) {
  ctx = handlers ?? {};
  return () => {
    ctx = {};
  };
}

/** Whether the shortcut cheatsheet overlay is open (bound to `?`). */
export const cheatsheet = writable(false);

/**
 * The canonical shortcut table — also what the cheatsheet renders, so the help
 * can never drift out of sync with the actual bindings.
 */
export const SHORTCUTS = [
  { keys: ['Esc'], label: 'Clear all screens', scope: 'Always' },
  { keys: ['B'], label: 'Blackout — kill every output', scope: 'Always' },
  { keys: ['A'], label: 'Accept the top AI suggestion', scope: 'Console' },
  { keys: ['D'], label: 'Dismiss the top AI suggestion', scope: 'Console' },
  { keys: ['→', 'PgDn', 'Space'], label: 'Next', scope: 'Console · Planner' },
  { keys: ['←', 'PgUp'], label: 'Previous', scope: 'Console · Planner' },
  { keys: ['/'], label: 'Focus search', scope: 'Console' },
  { keys: ['?'], label: 'Show this cheatsheet', scope: 'Always' },
];

/**
 * Install the single global keydown listener.
 *
 * `always` carries the panic actions, which are wired straight to the store and
 * deliberately do NOT go through the per-view context — they must fire from every
 * tab, including one whose view has crashed.
 *
 * Returns a teardown fn.
 */
export function installShortcuts({ clearScreens, blackScreen }) {
  function onKey(e) {
    if (e.metaKey || e.ctrlKey || e.altKey) return; // leave OS/browser combos alone

    const typing = isTyping(e);

    // ---- ALWAYS-ON. These fire even mid-typing: if the wrong thing is on the
    // screen in front of a congregation, the operator must not first have to
    // work out which field their cursor is in.
    if (e.key === 'Escape') {
      e.preventDefault();
      clearScreens();
      if (typing && e.target.blur) e.target.blur();
      cheatsheet.set(false);
      return;
    }

    // Everything below yields to text entry.
    if (typing) return;

    if (e.key === 'b' || e.key === 'B') {
      e.preventDefault();
      blackScreen();
      return;
    }
    if (e.key === '?') {
      e.preventDefault();
      cheatsheet.update((v) => !v);
      return;
    }

    // ---- CONTEXT. Only if the mounted view offers the action.
    switch (e.key) {
      case 'a':
      case 'A':
        if (ctx.accept) {
          e.preventDefault();
          ctx.accept();
        }
        break;
      case 'd':
      case 'D':
        if (ctx.dismiss) {
          e.preventDefault();
          ctx.dismiss();
        }
        break;
      case 'ArrowRight':
      case 'PageDown':
      case ' ':
        // Space means ADVANCE, everywhere, and nothing else. It used to also
        // mean "push the AI's guess live" on the Console — same key, two
        // meanings, one of them irreversible in front of an audience.
        if (ctx.next) {
          e.preventDefault();
          ctx.next();
        }
        break;
      case 'ArrowLeft':
      case 'PageUp':
        if (ctx.prev) {
          e.preventDefault();
          ctx.prev();
        }
        break;
      case '/':
        if (ctx.search) {
          e.preventDefault();
          ctx.search();
        }
        break;
    }
  }

  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}
