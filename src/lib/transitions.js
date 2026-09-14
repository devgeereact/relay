/**
 * HOW ONE SLIDE BECOMES THE NEXT — seven of them, in one register.
 *
 * Two things were true here before this module existed, and neither was
 * obvious from either end:
 *
 *   1. The theme editor offered a transition and a duration, and both were
 *      SAVED and then ignored. `TemplateRender` says so in a comment — "the
 *      theme editor's transition control is a no-op by design" — which is a
 *      control that changes nothing, documented rather than fixed.
 *   2. `layers.js` carried `slideRevealCss`, with tests, and nothing rendered
 *      it. A helper with tests and no caller looks exactly like working code.
 *
 * Both are replaced by this: a register of what exists, and one pure function
 * that turns a mode and a progress into inline CSS. The renderer drives it with
 * a Svelte transition, and the test drives it directly — same definition.
 *
 * ONLY opacity, transform and filter are animated. Those three are composited
 * and never trigger layout, which is what keeps a transition off the critical
 * path of a fire — and, just as importantly, keeps it away from the auto-fit:
 * the fitter reads `scrollHeight` against `clientHeight`, and neither is moved
 * by any of the three. A transition that animated width, padding or font-size
 * would make the fit measure a shape the slide is not going to settle at, which
 * is how transitions were removed from this renderer in the first place.
 */

/** The seven, in the order an operator sees them. `cut` is first and default. */
export const TRANSITIONS = Object.freeze([
  { id: 'cut', label: 'Cut', hint: 'Instant. Nothing animates.' },
  { id: 'crossfade', label: 'Crossfade', hint: 'The new slide fades up.' },
  { id: 'dissolve', label: 'Dissolve', hint: 'Fades up through a soft blur.' },
  { id: 'fadeblack', label: 'Fade through black', hint: 'Darkens, then comes back up.' },
  { id: 'pushleft', label: 'Push left', hint: 'Slides in from the right.' },
  { id: 'slideup', label: 'Slide up', hint: 'Rises into place.' },
  { id: 'materialise', label: 'Materialise', hint: 'Scales up as it fades in.' },
]);

/** Is this a mode we know? Anything else is treated as a cut. */
export const isTransition = (id) => TRANSITIONS.some((t) => t.id === id);

/** The default, and what an unknown or absent mode becomes. */
export const DEFAULT_TRANSITION = 'cut';

/**
 * The inline CSS for `mode` at progress `t` (0 → 1).
 *
 * `t` is how far through the transition the incoming slide is: 0 is the instant
 * it appears, 1 is settled. Every mode ends at exactly the settled state — no
 * leftover transform, no opacity below 1 — because a slide that finishes a
 * transition 2% transparent stays that way for as long as it is on the wall.
 */
export function transitionCss(mode, t) {
  const p = Math.max(0, Math.min(1, Number(t)));
  switch (mode) {
    case 'crossfade':
      return `opacity:${p};`;
    case 'dissolve':
      // The blur has to reach exactly 0, or the verse is permanently soft.
      return `opacity:${p}; filter:blur(${((1 - p) * 0.6).toFixed(3)}cqw);`;
    case 'fadeblack':
      // Down through black and back up: dark for the first half, then revealed.
      return p < 0.5
        ? `opacity:0; filter:brightness(0);`
        : `opacity:${((p - 0.5) * 2).toFixed(3)}; filter:brightness(${((p - 0.5) * 2).toFixed(3)});`;
    case 'pushleft':
      return `opacity:${p}; transform:translateX(${((1 - p) * 6).toFixed(3)}cqw);`;
    case 'slideup':
      return `opacity:${p}; transform:translateY(${((1 - p) * 4).toFixed(3)}cqh);`;
    case 'materialise':
      return `opacity:${p}; transform:scale(${(0.94 + p * 0.06).toFixed(4)});`;
    case 'cut':
    default:
      // A cut has no intermediate state. Returning an opacity here would make
      // `cut` a one-frame fade, which is not the same thing.
      return '';
  }
}

/**
 * How long this transition should run, in ms.
 *
 * Zero for a cut, zero when the viewer has asked for reduced motion, and zero
 * for a mode nothing knows — in every one of those cases the slide simply
 * appears, which is the behaviour an operator asked for in the first place
 * ("quick as light, remove every animation") and the safe answer when in doubt.
 */
export function transitionDuration(mode, ms, reducedMotion = false) {
  if (reducedMotion || !isTransition(mode) || mode === 'cut') return 0;
  const n = Number(ms);
  if (!Number.isFinite(n) || n <= 0) return 0;
  // A transition longer than a second on a wall is an operator waiting for the
  // machine, mid-service.
  return Math.min(1000, n);
}
