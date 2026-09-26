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
/**
 * Is a modal or a transient overlay mounted? Read from the DOM rather than a
 * registry of open overlays: a registry is a list somebody has to remember to add
 * the next dialog to, and the whole point is that this must not depend on anybody
 * remembering. `alertdialog` is a dialog; `menu`/`listbox` are transient overlays.
 * If you add an overlay kind, add it here — `panic.test.js` will tell you if not.
 */
const OVERLAY_ROLES = '[role="dialog"],[role="alertdialog"],[role="menu"],[role="listbox"]';
function overlayOpen() {
  return !!document.querySelector(OVERLAY_ROLES);
}

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
 * Does this element activate on Space by itself?
 *
 * Only `Space` consults this, and only to STAND DOWN — see the `' '` case below.
 * A focused `<button>` (or an element that has taken a button's role, or a link)
 * is activated by Space on every platform, so claiming the key there makes the
 * focused control do something other than what it says.
 *
 * Deliberately narrow. A disabled button activates nothing, so the transport
 * keeps the key; and this asks about the ELEMENT, never about which view is
 * mounted, so it cannot drift as surfaces are added.
 */
function isActivatable(el) {
  if (!el || typeof el.tagName !== 'string') return false;
  if (el.disabled) return false;
  const role = el.getAttribute?.('role');
  return (
    el.tagName === 'BUTTON' ||
    role === 'button' ||
    role === 'switch' ||
    (el.tagName === 'A' && el.hasAttribute?.('href'))
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
 *
 * ── THE `short` GLOSS IS GONE (2026-09-14) ─────────────────────────────────
 *
 * Every entry briefly carried a two-or-three-word `short` field, for a keys
 * legend across the chrome bar. The operator asked for the chrome to be cleared
 * back to the wordmark and the six workspaces, so the legend went — and the
 * field went with it rather than staying behind as a column nothing renders.
 * The cheatsheet `?` opens is once again the ONE place the keys are documented,
 * which is what it was before and is the arrangement this table was built for.
 *
 * WHAT IS DELIBERATELY ABSENT, and still is. `R` rehearse and `1–6` workspace.
 * Neither is bound — `installShortcuts` below has no branch for either, and `R`
 * could not safely acquire one while the section keys (REBRAND §10) hand out
 * every unclaimed letter. Listing them anywhere would teach an operator a key
 * that does nothing, under pressure, which is the exact defect `activeActions`
 * above exists to prevent. They are absent until something binds them, and then
 * this table is where it goes and the cheatsheet picks it up for free.
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

      // ANY modal is open → Escape belongs to that modal, and to nothing else.
      //
      // The cheatsheet was guarded above and the arrangement pickers were not, so
      // Escape inside one of those wiped the congregation's screens — and did not even
      // close the picker, because its Escape handler is bound to the backdrop element
      // and only fires when the backdrop happens to hold focus, which it usually does
      // not. The operator got the one outcome they did not ask for and none of the one
      // they did.
      //
      // Read from the DOM rather than a registry of open overlays: a registry is a
      // list somebody has to remember to add the next dialog to, and the whole point
      // is that this must not depend on anybody remembering.
      //
      // ── Why this is a LIST of roles, and why it grew (2026-08-14) ────────────
      //
      // It probed for `[role="dialog"]` alone, and the audit found SEVEN overlays
      // it therefore did not cover: six popup menus and the console crash panel.
      // Pressing Escape in any of them wiped the congregation's screens AND left
      // the overlay open — the same "one outcome they did not ask for, none of the
      // one they did" this guard was written to end, on the doors nobody
      // enumerated.
      //
      // The crash panel is the sharpest of the seven. It uses `role="alertdialog"`
      // — the ARIA role for exactly the modals that matter most — and its own copy
      // reads "Your output screens are still live". So the reflex key for
      // dismissing a modal blanked the wall at the one moment the product
      // guarantees the wall is hot.
      //
      // `alertdialog` is a dialog and `menu`/`listbox` are transient overlays; in
      // every case Escape belongs to the thing on top. If you add an overlay kind,
      // add it here — and `panic.test.js` will tell you if you don't.
      if (overlayOpen()) {
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
    //
    // …AND ONLY IF NOTHING IS OPEN ON TOP OF THE VIEW (2026-09-21, Live D1). The
    // overlay probe above lived inside the Escape branch alone, so `A`, `D`, the
    // arrows and Space kept acting on the surface UNDER an open dialog. Opening
    // "Why this match?" on the third claim and pressing `A` accepted the first —
    // the keyboard half of the bug `Live.svelte` had already fixed for the
    // buttons — and `→` stepped the plan while the operator was reading a panel.
    // `B` stays above this line: it is a panic key (rule 15), not a context key.
    if (overlayOpen()) return;
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
        //
        // ONE EXCEPTION, and it is not a second meaning: a FOCUSED BUTTON.
        //
        // Space is the platform's activation key for a focused `<button>`, and
        // `isTyping` only excluded text fields — so a keyboard operator who
        // tabbed to `Rehearse`, `Blackout` or `Clear screens` and pressed Space
        // advanced the programme instead of pressing the control under their
        // finger, and `preventDefault` suppressed the button's own click. On
        // Live that is a key silently doing something other than what the
        // focused control says it does, which is the shape rule 11 exists to
        // stop rather than an application of it.
        //
        // The arrows and PageDown are deliberately NOT narrowed: they keep
        // working from anywhere, focused button or not, so the transport is
        // never lost. Yielding Space to the thing that has focus costs the
        // operator nothing, because `→` is beside it and does the same job.
        if (e.key === ' ' && isActivatable(e.target)) break;
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
      default:
        // ---- SECTION KEYS (REBRAND §10). A surface showing a song's sections
        // registers `sectionKey` and gets the letters nothing above has claimed.
        //
        // Its POSITION is the whole guarantee, and it is three guarantees deep:
        //   · it is below the always-on block, so `Escape` and `b` are already
        //     gone — a panic key can never be shadowed by a section;
        //   · it is below `if (typing) return`, so a letter typed into the
        //     reference box or the lyric editor fires nothing;
        //   · it is below `a`, `d` and `/`, so a surface offering both keeps the
        //     older meaning of those three.
        // `sectionkeys.js::RESERVED` reads this file's own SHORTCUTS table and
        // refuses to hand out any of those letters in the first place, so the
        // rule is kept on both doors rather than only on this one.
        //
        // With nothing registered it is a no-op and does NOT preventDefault —
        // a dead branch must not eat a keystroke the browser had a use for.
        if (ctx.sectionKey && /^[a-z0-9]$/.test(e.key)) {
          e.preventDefault();
          ctx.sectionKey(e.key);
        }
        break;
    }
  }

  window.addEventListener('keydown', onKey);
  return () => window.removeEventListener('keydown', onKey);
}
