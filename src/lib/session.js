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
  // Everyone lands on LIVE, including a fresh install, because the whole object is
  // persisted: an operator who was running a service yesterday comes back to Live,
  // not to a summary screen.
  //
  // This used to say a fresh install lands on the Dashboard, and it had not been
  // true since the Dashboard stopped being a top-level tab and became a section
  // inside Settings. The value below never changed — only the comment describing
  // it went stale, which is the more dangerous half: it reads as intent, and the
  // next person to "restore" it would be reintroducing a tab that no longer exists.
  activeTab: 'live',
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
  // NOTHING SAVED = a genuinely fresh install. Falls through to EMPTY, which lands
  // on the run surface; the "is this machine going to work?" question is answered
  // by the LAUNCH & STARTUP sequence and the Dashboard section inside Settings,
  // not by a tab that no longer exists.
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
    //
    // ── AND `setupDone` SURVIVES IT, WHICH IS THE WHOLE POINT ────────────────
    //
    // This used to return `{ ...EMPTY, activeTab: 'live' }` — and `EMPTY.setupDone`
    // is `false`, which IS the fresh-install signal and the only thing App.svelte
    // reads. So the branch that exists to say "this is NOT a fresh install" said
    // exactly that, and the six-step modal wizard opened over a console that may
    // have been mid-service.
    //
    // **A key that exists is proof the app has run on this machine.** A genuinely
    // fresh install has no key at all and is handled above. So the wizard — which
    // is for somebody who has never set Relay up — is not what this operator needs;
    // everything it configures also lives in Settings, and `restartFirstRun()` is
    // one click away if they do want it.
    //
    // The unreadable bytes are KEPT, under a sidecar key, because
    // `session.subscribe` persists on every change and fires immediately — so the
    // fallback is written straight back over the corrupt payload before any other
    // module gets a chance to look at it. That destroyed the very resume point the
    // comment above is worried about. Nothing reads the sidecar yet, and that is
    // stated rather than implied: it exists so the evidence survives, and so a
    // future repair has something to repair FROM.
    try {
      localStorage.setItem(`${KEY}.corrupt`, raw);
    } catch {
      // Quota or private mode. Preserving the bytes is a courtesy, never a
      // precondition for booting.
    }
    return { ...EMPTY, activeTab: 'live', setupDone: true };
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

// ── Tabs that MOVED ─────────────────────────────────────────────────────────
//
// `activeTab` is persisted, so it long outlives the layout it was written under.
// When a surface stops being a top-level tab, every operator who was last on it
// still has its key in localStorage — and `tabs.some(...)` says "unknown", which
// lands them on Live.
//
// Live is the safe fallback, but it is the WRONG answer here: none of these
// surfaces was deleted. They were relocated, and the operator asked for the
// thing, not for the tab strip. Sending them where it went is the difference
// between "moved" and "vanished".
//
//   stagedisplays → Outputs  — the old localStorage-only Stage Displays gallery
//                              was absorbed into real backend channels.
//   dashboard     → Settings — became a Settings section (records/overview, not
//                              a run surface).
//   history       → Settings — same: a record of past services is config, not a
//                              tab an operator runs a service from.
//
// Add an entry here whenever a tab is folded into another surface. A key that is
// genuinely gone (not moved) belongs nowhere in this map — it should fall through
// to the run surface.
export const MOVED_TABS = {
  stagedisplays: 'channels',
  dashboard: 'settings',
  history: 'settings',
};

/**
 * The tab to actually render, given what the session remembers.
 *
 * Pure and exported so it is testable — App.svelte is not. The redirect lived
 * inline as a one-key ternary and quietly stopped covering two of the three tabs
 * that had moved, because nothing could fail when it went stale.
 *
 * @param saved  the persisted `activeTab` (may be stale, unknown, or missing)
 * @param known  the keys currently in the tab strip
 */
export function resolveActiveTab(saved, known) {
  const target = MOVED_TABS[saved] ?? saved;
  return known.includes(target) ? target : 'live';
}
