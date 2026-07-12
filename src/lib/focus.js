// Focus management for modal dialogs — the ONE implementation.
//
// Relay had zero focus traps across three dialogs, and only the crash overlay even
// moved focus into itself. So a keyboard operator opening the first-run wizard could
// Tab straight past it into the app behind, driving controls they cannot see, in a
// modal they cannot leave.
//
// It is a Svelte action, so a dialog opts in with one attribute and cannot forget the
// teardown:
//
//     <div role="dialog" aria-modal="true" use:trapFocus>
//
// ── What it does NOT do ────────────────────────────────────────────────────────
//
// It does not bind Escape. Escape is a PANIC KEY in this app — it clears the
// congregation's screens — and the rule about who may consume it lives in exactly one
// place, `shortcuts.js`, which refuses to clear while any [role="dialog"] is mounted.
// A trap that also grabbed Escape would be a second opinion on that, and two opinions
// about a panic key is how the wall gets wiped by accident.

/** Elements that can actually take focus, in DOM order. */
const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

/**
 * Is this element actually reachable?
 *
 * Deliberately NOT `offsetParent !== null`. That is the usual idiom and it is a trap:
 * it depends on layout, so in any non-rendering context (jsdom, and therefore every
 * test we would write for this) it reports EVERY element as hidden — the trap silently
 * becomes a no-op and its tests pass by finding nothing to do.
 *
 * `checkVisibility()` is the real answer where it exists; the attribute check is a
 * correct-enough fallback, because a modal does not hide its own controls with CSS.
 */
function visible(el) {
  if (typeof el.checkVisibility === 'function') return el.checkVisibility();
  return !el.hidden && !el.closest('[hidden], [aria-hidden="true"]');
}

function focusable(node) {
  return Array.from(node.querySelectorAll(FOCUSABLE)).filter(visible);
}

/**
 * Svelte action: trap Tab inside `node`, focus the first control, and give focus back
 * to wherever it was when the dialog closes.
 *
 * Restoring focus is the half people forget, and it is the half a keyboard operator
 * notices: dismiss a dialog without it and focus falls to `<body>`, so the next Tab
 * starts from the top of the app rather than from the button they just pressed.
 */
export function trapFocus(node) {
  const previous = document.activeElement;

  // Focus the first real control, not the dialog container — an operator should land
  // on something they can act on.
  const first = focusable(node)[0];
  if (first) first.focus();
  else if (node.tabIndex < 0) {
    // Nothing focusable inside (a purely informational dialog). Make the container
    // itself the focus stop, so focus is at least INSIDE the modal.
    node.tabIndex = -1;
    node.focus();
  }

  function onKeydown(e) {
    if (e.key !== 'Tab') return;
    // A trap whose dialog is gone must not still be grabbing Tab. The action's
    // destroy() normally removes this listener, but the document-level one outlives a
    // detached node just long enough to matter — and a stale trap hijacking Tab into a
    // dialog that is no longer on screen is worse than no trap at all.
    if (!node.isConnected) return;
    const items = focusable(node);
    if (items.length === 0) {
      e.preventDefault(); // nowhere to go; do not let Tab escape
      return;
    }
    const firstEl = items[0];
    const lastEl = items[items.length - 1];
    const active = document.activeElement;

    // Wrap at both ends. Without this, Tab off the last control lands in the app
    // BEHIND the modal — which is the whole bug.
    if (e.shiftKey && (active === firstEl || !node.contains(active))) {
      e.preventDefault();
      lastEl.focus();
    } else if (!e.shiftKey && (active === lastEl || !node.contains(active))) {
      e.preventDefault();
      firstEl.focus();
    }
  }

  node.addEventListener('keydown', onKeydown);
  // Also on the document: if focus has somehow escaped the modal already (a click on
  // the scrim, say), Tab must still be brought back rather than continuing into the app.
  document.addEventListener('keydown', onKeydown, true);

  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      document.removeEventListener('keydown', onKeydown, true);
      // Give focus back to whatever opened this. `isConnected` because the opener may
      // itself have been removed while the dialog was up.
      if (previous && previous.isConnected && typeof previous.focus === 'function') {
        previous.focus();
      }
    },
  };
}
