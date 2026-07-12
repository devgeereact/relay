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

import { writable, derived, get } from 'svelte/store';

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

/**
 * Which context actions are live RIGHT NOW. The cheatsheet reads this, so it can
 * only ever advertise keys that actually do something.
 *
 * It used to lie. The Planner registers only `next`/`prev`, so on that tab `A`
 * (accept), `D` (dismiss) and `/` (search) were DEAD KEYS — while the cheatsheet
 * cheerfully listed all three. An operator pressing `A` mid-service to put the
 * AI's suggestion on screen would have got nothing, and no explanation.
 *
 * A help screen that lists a key which does nothing is worse than no help screen:
 * it teaches the operator something false, under pressure.
 */
export const activeActions = writable([]);

/** Called by a view on mount. Returns an unregister fn for onDestroy. */
export function registerContext(handlers) {
  ctx = handlers ?? {};
  activeActions.set(Object.keys(ctx).filter((k) => typeof ctx[k] === 'function'));
  return () => {
    ctx = {};
    activeActions.set([]);
  };
}

/** Whether the shortcut cheatsheet overlay is open (bound to `?`). */
export const cheatsheet = writable(false);

/**
 * The canonical shortcut table — also what the cheatsheet renders, so the help
 * can never drift out of sync with the actual bindings.
 */
export const SHORTCUTS = [
  { keys: ['Esc'], label: 'Clear all screens', always: true },
  { keys: ['B'], label: 'Blackout — kill every output', always: true },
  { keys: ['?'], label: 'Show this cheatsheet', always: true },
  // `needs` names the context action a key depends on. If the current surface has
  // not registered that action, the key does nothing — and the cheatsheet does not
  // claim otherwise.
  { keys: ['A'], label: 'Accept the top AI suggestion', needs: 'accept' },
  { keys: ['D'], label: 'Dismiss the top AI suggestion', needs: 'dismiss' },
  // Advance/back are MODE-DEPENDENT — they step the service plan when a plan cue
  // is live, and walk the passage when a detected or manually-fired verse is. The
  // Live transport bar always says which, because the same key doing two things
  // silently is how an operator puts the wrong thing in front of a congregation.
  { keys: ['→', 'PgDn', 'Space'], label: 'Next slide / next verse', needs: 'next' },
  { keys: ['←', 'PgUp'], label: 'Previous slide / previous verse', needs: 'prev' },
  { keys: ['/'], label: 'Jump to the manual reference box', needs: 'search' },
];

/** The shortcuts that actually work on the surface the operator is looking at. */
export const liveShortcuts = derived(activeActions, ($active) =>
  SHORTCUTS.filter((s) => s.always || $active.includes(s.needs)),
);

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

    // ---- ALWAYS-ON.
    //
    // Escape fires even mid-typing: if the wrong thing is on the screen in front of
    // a congregation, the operator must not first have to work out which field their
    // cursor is in. (`B` cannot do the same — see below.)
    if (e.key === 'Escape') {
      e.preventDefault();
      if (typing && e.target.blur) e.target.blur();

      // If the help overlay is open, Escape CLOSES THE OVERLAY — and does nothing
      // else. It used to close the overlay *and* clear the screens, unconditionally,
      // because there was no guard here at all. So an operator who hit `?` mid-service
      // to check a binding, then hit Escape to put the help away, wiped the wall. The
      // cheatsheet is a read-only overlay; dismissing it is not a live action.
      if (get(cheatsheet)) {
        cheatsheet.set(false);
        return;
      }

      clearScreens();
      return;
    }

    // Everything below yields to text entry — including `B`.
    //
    // This is deliberate, and it is why the cheatsheet must NOT claim that `B` works
    // while typing (it said exactly that). `B` cannot fire from inside a text field:
    // an operator typing "Habakkuk" into the reference box would black out the
    // congregation's screens on the second keystroke. Escape is the panic key that
    // survives a focused input — one press blurs the field AND clears the screens.
    // That is the honest instruction, and it is what App.svelte now prints.
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
