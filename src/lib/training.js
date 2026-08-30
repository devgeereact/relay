// PRACTICE — the drills a volunteer runs before their first Sunday.
//
// Single responsibility: hold the list of things an operator has to be able to do,
// and decide from Relay's own events whether they have just done one.
//
// ── What this is, and the thing it deliberately is not ────────────────────────
//
// It is **not a simulation of a service.** Relay cannot produce a sermon: there is
// no preacher, no room, and no way to synthesise speech offline. Anything that
// claimed to simulate one would be teaching a volunteer the shape of a fake.
//
// It is drills with the REAL controls, on the REAL surface, in rehearsal. Each drill
// names a situation and waits for the operator to do the right thing, and it knows
// they did because the same event fired that would fire on a Sunday. That is the
// difference between practising and reading a manual: the muscle memory is the point,
// and muscle memory is built on the actual key.
//
// ── The order is the argument ─────────────────────────────────────────────────
//
// The panic controls come FIRST, before anything about firing verses. A volunteer
// who can clear a screen is safe to leave alone; one who can fire beautifully and
// freezes when the wrong thing is up is not. Every list of operator training this
// product has ever sketched put "accept a suggestion" first, and that is backwards.
//
// ── It runs in rehearsal, or it does not run ─────────────────────────────────
//
// Same rule as the path check (RG-15) and for the same reason: the controls are
// real, so the sandbox has to be too. A drill that put a practice verse on a
// congregation's wall would be the single most embarrassing bug this product could
// ship.

/**
 * `check` is given `{ kind, payload }` for each observed event and returns true when
 * the drill has been satisfied. Pure — no stores, no DOM — so the whole curriculum
 * is testable without mounting anything.
 */
export const DRILLS = [
  {
    id: 'clear',
    title: 'Something wrong is on the screen. Get rid of it.',
    hint: 'Press Escape, or the Clear button on the Live tab. This is the one that matters most.',
    // Why first: an operator who can clear a screen is safe to leave alone. One who
    // can fire beautifully and freezes when the wrong thing is up is not.
    check: (e) => e.kind === 'clear',
  },
  {
    id: 'black',
    title: 'Now black the screens out completely.',
    hint: 'Press B, or Blackout on the Live tab. Clear leaves the screen empty; blackout kills it.',
    check: (e) => e.kind === 'black',
  },
  {
    id: 'fire',
    title: 'The preacher has jumped to John 3:16. Put it up by hand.',
    hint: 'Type the reference into the box at the bottom of the AI panel and press Fire. You never have to wait for Relay to hear it.',
    check: (e) => e.kind === 'content',
  },
  {
    id: 'next',
    title: 'They are reading on. Move to the next verse.',
    hint: 'Press → (or the ▶ in the transport). It walks the passage when a verse is up, and steps the plan when plan content is.',
    check: (e) => e.kind === 'content' || e.kind === 'nav',
  },
  {
    id: 'suggestion',
    title: 'Relay offers a suggestion you do not want. Turn it down.',
    hint: 'Press Dismiss on the suggestion card. Dismissing is not a failure — it is how Relay learns your preacher.',
    check: (e) => e.kind === 'dismiss',
  },
  {
    id: 'rehearsal',
    title: 'Finally: check you can tell rehearsal from live.',
    hint: 'Look at the top bar. It says REHEARSAL, in violet, the whole time you have been practising — that is what tells you nothing is reaching the congregation.',
    // Acknowledged rather than performed. The operator is already in rehearsal, so
    // asking them to turn it on would teach the wrong reflex; what they need is to
    // have LOOKED at the indicator once, deliberately.
    check: (e) => e.kind === 'acknowledge',
  },
];

export function newSession() {
  return { active: false, index: 0, done: [], startedAt: 0 };
}

export function start(now = Date.now()) {
  return { active: true, index: 0, done: [], startedAt: now };
}

export function current(s) {
  return s.active ? (DRILLS[s.index] ?? null) : null;
}

/**
 * Feed one observed event. Returns the next session state.
 *
 * Only the CURRENT drill can be satisfied. Letting a later drill complete out of
 * order would let an operator finish the course without ever pressing the control
 * it was there to teach — which is the failure mode of every checklist that scores
 * itself generously.
 */
export function observe(s, event) {
  const drill = current(s);
  if (!drill || !event) return s;
  if (!drill.check(event)) return s;
  return {
    ...s,
    done: [...s.done, drill.id],
    index: s.index + 1,
    active: s.index + 1 < DRILLS.length,
  };
}

/** Skip the current drill — an operator may be unable to do one right now. */
export function skip(s) {
  if (!s.active) return s;
  return { ...s, index: s.index + 1, active: s.index + 1 < DRILLS.length };
}

export function stop(s) {
  return { ...s, active: false };
}

/** Finished everything, rather than stopped part-way. */
export function isComplete(s) {
  return !s.active && s.done.length === DRILLS.length;
}

/**
 * What to say at the end.
 *
 * A partial run is reported as a partial run. Congratulating somebody who skipped
 * the blackout drill would be worse than saying nothing, because they would believe
 * it.
 */
export function summary(s) {
  if (s.active) return '';
  if (isComplete(s))
    return 'That is the whole set. You can do everything a service needs, including the two that matter when something goes wrong.';
  const missed = DRILLS.filter((d) => !s.done.includes(d.id));
  if (!s.done.length) return 'Practice stopped. Nothing was completed.';
  const panic = missed.filter((d) => d.id === 'clear' || d.id === 'black');
  return panic.length
    ? `${s.done.length} of ${DRILLS.length} done — but not ${panic
        .map((d) => (d.id === 'clear' ? 'clearing the screens' : 'the blackout'))
        .join(' or ')}. Those are the two worth coming back for.`
    : `${s.done.length} of ${DRILLS.length} done. The rest are worth another go before Sunday.`;
}
