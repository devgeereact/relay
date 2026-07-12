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
  // NEVER shows again — a wizard that reappears is a wizard that gets clicked
  // through blindly, and everything it configures also lives in Settings.
  setupDone: false,
  activeTab: 'console',
  planId: null,
  liveCueId: null,
  liveSlide: 0,
  serviceId: null,
};

function load() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...EMPTY };
    const parsed = JSON.parse(raw);
    // Merge over EMPTY so an older/partial payload can't leave holes.
    return { ...EMPTY, ...parsed };
  } catch {
    // Corrupt payload must never block boot — a broken resume is recoverable,
    // a console that won't start is not.
    return { ...EMPTY };
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
