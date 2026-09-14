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

// ── THE OPERATOR'S LIVE OVERRIDE (docs/DECISIONS.md §84) ───────────────────────
//
// §71 made a transition a TEMPLATE's choice. That is the right home for a decision
// somebody made on a Tuesday, and the wrong home for the one an operator has to
// make at 10:42 on a Sunday: the preacher has gone off-plan, four templates are in
// rotation, and "make everything cut, now" is not an instruction you can carry out
// by editing four templates.
//
// So there are now two authorities over ONE property, which is exactly the defect
// §3.1 of the rebrand spec exists to prevent — unless the order is stated and the
// operator can see which one is in force. Both halves are here: `resolveTransition`
// is the one place the two are ranked, and the picker's first option is
// **Follow template**, so an override is something an operator turned on and can
// see is on, rather than a silent second home for the same property.

import { writable } from 'svelte/store';

/**
 * The override in force right now — `{ mode, ms }`, or `null` to follow the template.
 *
 * It lives HERE rather than in `stores/capture.js` for one concrete reason:
 * `TemplateRender` reads it, and `TemplateRender` also renders `output.html`, which
 * is served to an OBS/kiosk browser source with NO backend at all. Importing the
 * Tauri bridge module into that bundle to read one field would put the whole
 * command surface on a congregation screen. `capture.js` still owns the COMMAND
 * (`setLiveTransition`) — the bridge rule is unchanged, and this is the same split
 * `session.js`, `updater.js` and `countdown.js` already use.
 *
 * DELIBERATELY NOT PERSISTED. It is a live control like the blackout: a Relay that
 * has restarted has no opinion about how last Sunday was cutting, and a remembered
 * one would be an unattended change to every screen at boot.
 */
export const liveTransition = writable(null);

/** Has this override any effect, or is it an absent value in a box? */
export const isOverride = (o) => !!o && isTransition(o.mode);

/**
 * WHICH TRANSITION IS IN FORCE, and on whose authority (DECISIONS §84).
 *
 * The order, stated the way §29 states it for templates:
 *
 *   1. THE OPERATOR'S LIVE OVERRIDE, at any value — `cut` included, which is the
 *      whole point. A deliberate act now outranks a saved default, and the one
 *      thing an operator must always be able to do is take the motion away.
 *   2. Otherwise THE TEMPLATE'S OWN CHOICE (§71), already resolved through its
 *      theme by the caller before it reaches here.
 *   3. Otherwise a CUT.
 *
 * An override naming a mode nothing knows is NOT an override: it falls through to
 * the template rather than becoming a cut, because a frame from a newer version
 * must not be able to silently unstyle a wall that was working.
 *
 * `source` comes back so a surface can SAY which of the two it is showing. A picker
 * that cannot tell an operator whether they are reading their own choice or the
 * template's is the second-home defect with a control bolted onto it.
 */
export function resolveTransition(style, override) {
  if (isOverride(override)) {
    return { mode: override.mode, ms: override.ms, source: 'operator' };
  }
  return {
    mode: (style && style.transition) || DEFAULT_TRANSITION,
    ms: style ? style.transitionMs : undefined,
    source: 'template',
  };
}

/** The durations the chrome picker offers, in ms. 0 is a cut however it is dressed. */
export const TRANSITION_MS = Object.freeze([0, 200, 320, 500, 800]);

/** What a newly-chosen override starts at — the prototype's default. */
export const DEFAULT_TRANSITION_MS = 320;
