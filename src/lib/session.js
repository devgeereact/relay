// Operator session state that must SURVIVE a reload or a crash.
//
// Everything the operator was doing lived in component-local variables: the
// active tab, which plan was open, which cue and slide were live. A reload —
// or an uncaught error tearing down the console — dumped all of it and dropped
// them back on the Console tab with no idea where they had been in the service.
//
// That is the wrong failure mode for software someone is running live in front
// of a congregation, so this is persisted to localStorage on every change and
// restored on boot. It is deliberately TINY and content-free: ids and positions
// only, never transcript or verse text (that is congregation/sermon data and it
// stays in SQLite, per the local-first rule in CLAUDE.md).

import { writable } from 'svelte/store';

const KEY = 'relay.session.v1';

const EMPTY = {
  // Has the operator been through (or skipped) first-run setup? Once true it
  // NEVER shows again BY ITSELF — a wizard that reappears is a wizard that gets
  // clicked through blindly, and everything it configures also lives in Settings.
  //
  // But it can be ASKED for: `restartSetup()`, from a button in Settings. An operator
  // who skipped the wizard (or inherited the laptop from the last volunteer) could not
  // get it back at all, and it is the only place that walks them through the projector,
  // the microphone and a proof verse in one go — ending with them having SEEN it work.
  // Never showing up uninvited and never being reachable are two different things, and
  // only the first one is the good idea.
  setupDone: false,
  // A FRESH install lands on the Dashboard — the "is this machine going to work?"
  // surface. Everyone else is restored to whatever tab they were last on, because
  // this whole object is persisted: an operator who was running a service
  // yesterday comes back to Live, not to a summary screen.
  activeTab: 'dashboard',
  planId: null,
  liveCueId: null,
  liveSlide: 0,
  // Whether the plan was actually ON AIR, not just where the playhead was. Without
  // this, leaving the Live tab and coming back would restore the position AND
  // claim it was on the congregation's screen — lighting the amber ON AIR ring for
  // content that had been cleared. Amber means live. It is never allowed to lie.
  liveOnAir: false,
  serviceId: null,
  // How the run surface is presented. Persisted because a booth's screen does not
  // change between Sundays: an operator on a 13" laptop who chose compact should
  // not have to choose it again every week.
  liveDensity: 'normal', // 'normal' | 'compact'
  liveFullscreen: false, // hide the shell chrome around Live
};

function load() {
  const raw = (() => {
    try {
      return localStorage.getItem(KEY);
    } catch {
      return null;
    }
  })();
  // NOTHING SAVED = a genuinely fresh install. Land on the Dashboard: nobody has
  // ever run a service on this machine, and "is this going to work?" is the only
  // question they have.
  if (!raw) return { ...EMPTY };
  try {
    const parsed = JSON.parse(raw);
    // Merge over EMPTY so an older/partial payload can't leave holes.
    return { ...EMPTY, ...parsed };
  } catch {
    // A CORRUPT payload is NOT a fresh install, and the difference matters. There
    // was a session here — it may have been mid-service thirty seconds ago — we
    // simply cannot read it. So fall back to the RUN SURFACE, not to a summary
    // screen: if this happened during a service, the operator needs the console,
    // not a readiness report about a service that is already happening.
    //
    // Either way it must never block boot. A broken resume is recoverable; a
    // console that will not start is not.
    return { ...EMPTY, activeTab: 'live' };
  }
}

export const session = writable(load());

session.subscribe((s) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(s));
  } catch {
    // Quota/private-mode. Persistence is a nicety; never let it break the app.
  }
});

/** Patch one or more fields. */
export function setSession(patch) {
  session.update((s) => ({ ...s, ...patch }));
}

/** Forget the resume point — call when a service is properly finished. */
export function clearSession() {
  session.set({ ...EMPTY });
}

/**
 * Show the first-run wizard again, because the operator asked for it.
 *
 * ONLY from an explicit click in Settings. The wizard still never appears uninvited —
 * see `setupDone` above. This just means a volunteer who skipped it, or who took over a
 * laptop from whoever ran the desk last year, can get it back.
 *
 * It deliberately touches nothing else: not the playhead, not the open plan, not the
 * active tab. Re-running setup is not a reset, and it must not lose the operator's place
 * in a service that may be running while they poke at Settings.
 */
export function restartSetup() {
  setSession({ setupDone: false });
}
