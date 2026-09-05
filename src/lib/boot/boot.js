// Launch & Startup — the boot state machine.
//
// Every launch/startup screen is a RENDERING OF THIS FILE. The screens hold no
// facts of their own; they read the stores here. That is deliberate: a boot
// screen that computes its own answer is a boot screen that can disagree with
// the app it is booting.
//
// ── The one rule ───────────────────────────────────────────────────────────
//
//   A BOOT SCREEN MAY NEVER REPORT A CHECK IT DID NOT RUN.
//
// Relay's whole product claim is "it works with the network unplugged, in a
// church, on a borrowed laptop". A startup sequence that green-ticks a GPU it
// never probed teaches an operator to trust a screen that is lying, and the
// first time they need it — Sunday, 40 minutes before the service — it will
// still be lying. So every check carries its own PROVENANCE:
//
//   probe: 'live'  — a real Tauri command ran and this is its answer.
//   probe: 'stub'  — NO backend exists for this yet. Rendered as UNKNOWN, never
//                    as a pass. (The working design log this once cited is not in
//                    this repository; the rule stands on its own.)
//
// The UI paints a stub check grey with the word "not probed", at any state. It
// is not possible to make a stub check look green from here, and that is the
// point.

import { writable, derived, get } from 'svelte/store';
// CLAUDE.md: errors.js is the ONE backend-error humaniser. A failed probe used to
// put `String(e)` straight into the check's note — so Boot Diagnostics and the
// Dashboard's health panel would show a volunteer a raw JS TypeError, six rows at
// a time, on the two screens whose whole job is telling them whether the machine
// works.
import { humanError } from '../errors.js';

const KEY = 'relay.boot.v1';

/** Ordered boot stages. Each is a full screen; gates interrupt them. */
export const STAGES = ['diagnostics', 'hardware', 'plugins', 'migration'];

/** Gates need an operator decision, so they OUTRANK the running stage. */
export const GATES = ['crash', 'recover', 'update', 'safemode'];

// ── Persisted boot facts ───────────────────────────────────────────────────
// Tiny and content-free, exactly like session.js: whether the last run ended
// cleanly, and whether the operator asked for safe mode. Never sermon data.

const EMPTY_RECORD = {
  /** Did the previous run unload without the crash guard firing? */
  cleanExit: true,
  /** { at, message } from the last crash-guard trigger, else null. */
  lastCrash: null,
  /** How many boots in a row ended badly. Three is what offers safe mode. */
  crashStreak: 0,
  /** Operator-requested safe mode. Survives a reboot until they turn it off. */
  safeMode: false,
};

function loadRecord() {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...EMPTY_RECORD };
    return { ...EMPTY_RECORD, ...JSON.parse(raw) };
  } catch {
    // A corrupt boot record must never block a boot. That would turn a cosmetic
    // problem into an app that cannot start on a Sunday morning.
    return { ...EMPTY_RECORD };
  }
}

export const bootRecord = writable(loadRecord());

bootRecord.subscribe((r) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(r));
  } catch {
    /* quota / private mode — persistence is a nicety, never a blocker */
  }
});

export function patchRecord(patch) {
  bootRecord.update((r) => ({ ...r, ...patch }));
}

/**
 * The crash guard fired. Called from lib/crash.js so the NEXT boot can offer
 * Crash Report Recovery and, after a streak, Safe Mode.
 */
export function markCrash(message) {
  bootRecord.update((r) => ({
    ...r,
    cleanExit: false,
    lastCrash: { at: new Date().toISOString(), message: String(message ?? '').slice(0, 4000) },
    crashStreak: (r.crashStreak ?? 0) + 1,
  }));
}

/** The window is unloading and nothing had crashed. */
export function markCleanExit() {
  bootRecord.update((r) => ({ ...r, cleanExit: true }));
}

/** The operator dealt with the crash report — stop offering it. */
export function clearCrash() {
  bootRecord.update((r) => ({ ...r, lastCrash: null, cleanExit: true, crashStreak: 0 }));
}

// ── Safe mode ──────────────────────────────────────────────────────────────
// Safe mode is NOT cosmetic. It is a promise that nothing this app does can
// reach a congregation, so a volunteer can poke at a broken install during a
// service without putting anything on the wall. Whatever consumes it must
// honour it (App.svelte disarms detection and refuses to open outputs).

export const safeMode = derived(bootRecord, ($r) => !!$r.safeMode);

export function setSafeMode(on) {
  patchRecord({ safeMode: !!on });
}

// ── Checks ─────────────────────────────────────────────────────────────────

/** state: pending | running | ok | warn | fail | unknown */
const mk = (id, label, detail, probe = 'live') => ({
  id,
  label,
  detail,
  probe,
  state: 'pending',
  note: '',
});

/**
 * The check table, by stage. `probe: 'stub'` entries have NO backend command in
 * src-tauri yet — they render as "not probed", never as a pass.
 */
export function freshChecks() {
  return {
    diagnostics: [
      mk('engine', 'Relay engine', 'Rust core attached over IPC'),
      mk('version', 'Build', 'Version the updater compares against'),
      mk('database', 'Local database', 'SQLite opened, scripture readable'),
      mk('stt', 'Speech recognition', 'Local whisper model'),
      mk('audio', 'Audio input', 'Capture devices enumerated'),
      mk('network', 'Network', 'Offline is a valid, expected result'),
    ],
    // Every one of these is now a REAL read. CPU, memory, GPU and disk used to
    // be `probe: 'stub'` — see `system_hardware` in src-tauri/src/sysprobe.rs.
    hardware: [
      mk('inputs', 'Microphone inputs', 'Devices offered to capture'),
      mk('displays', 'Displays', 'Monitors available as output targets'),
      mk('lan', 'LAN address', 'For OBS and kiosk browser sources'),
      mk('cpu', 'Processor', 'Threads this process may use'),
      mk('memory', 'Memory', 'Headroom for the loaded model'),
      mk('gpu', 'Whisper acceleration', 'Backends compiled into this build'),
      mk('disk', 'Disk space', 'Room for models, media and history'),
    ],
    plugins: [
      mk('kiosk', 'Kiosk / OBS hub', 'WebSocket on :8031'),
      mk('http', 'Output server', 'Output and stage pages on :8032'),
      mk('propresenter', 'ProPresenter', 'Import of .pro files'),
      mk('ndi', 'NDI output', 'Requires the proprietary NDI SDK'),
      mk('obs', 'OBS WebSocket', 'Default port 4455 on this machine'),
      mk('atem', 'ATEM', 'Default port 9910 on this machine'),
    ],
    // The runner is not observable — it executes once, synchronously, before
    // this webview exists (src-tauri/src/db/mod.rs). So these report what the
    // schema ACTUALLY looks like now, read out of `sqlite_master` and
    // `pragma_table_info` by the `migration_status` command. They are a
    // verification, not a progress bar, and the screen says so.
    migration: [
      mk('schema', 'Schema version', 'Recorded against the version this build expects'),
      mk('objects', 'Schema objects', 'Every table and column the app needs'),
      mk('manualstatus', 'Detection status rebuild', "Allows a human's fire to be logged as manual"),
      mk('scratch', 'Leftover scratch table', 'The fingerprint of a failed rebuild'),
    ],
  };
}

export const checks = writable(freshChecks());

/** Patch one check in place. */
export function setCheck(stage, id, patch) {
  checks.update((all) => ({
    ...all,
    [stage]: all[stage].map((c) => (c.id === id ? { ...c, ...patch } : c)),
  }));
}

/** Every stage's roll-up: fail beats warn beats running beats ok. */
export function rollUp(list) {
  if (list.some((c) => c.state === 'fail')) return 'fail';
  if (list.some((c) => c.state === 'running' || c.state === 'pending')) return 'running';
  if (list.some((c) => c.state === 'warn')) return 'warn';
  return 'ok';
}

export const stageState = derived(checks, ($c) =>
  Object.fromEntries(STAGES.map((s) => [s, rollUp($c[s] ?? [])])),
);

// ── The sequence ───────────────────────────────────────────────────────────

/** Which stage screen is showing. null once boot is done. */
export const stage = writable(STAGES[0]);
/** Which gate is showing, if any. Outranks the stage. */
export const gate = writable(null);
/** True until the whole launch sequence has handed over to the console. */
export const booting = writable(true);

/**
 * Decide which gate (if any) the operator must answer before the console.
 * Pure — takes facts, returns a gate name or null. Order is the priority order:
 * a crash outranks a resume, which outranks an update.
 */
export function pickGate({ record, session, update }) {
  if (record?.lastCrash) return 'crash';
  if ((record?.crashStreak ?? 0) >= 3 && !record?.safeMode) return 'safemode';
  if (session && hasResumePoint(session)) return 'recover';
  if (update) return 'update';
  return null;
}

/** Is there anything worth offering to resume? */
export function hasResumePoint(s) {
  if (!s) return false;
  return !!(s.serviceId || s.planId || s.liveCueId);
}

/** Human summary of the resume point, for the Recover screen. */
export function describeResume(s) {
  const bits = [];
  if (s?.serviceId) bits.push(`service #${s.serviceId}`);
  if (s?.planId) bits.push(`plan #${s.planId}`);
  if (s?.liveCueId) bits.push(`cue #${s.liveCueId}, slide ${(s.liveSlide ?? 0) + 1}`);
  if (s?.activeTab) bits.push(`${s.activeTab} tab`);
  return bits.join(' · ');
}

/**
 * Run one stage's probes.
 *
 * `deps` is injected so this is testable without Tauri: every call is a plain
 * async function returning the same thing the real command does.
 */
/**
 * Run a list of checks and RETURN the results. Touches no store.
 *
 * This is the shared implementation, extracted so the **Dashboard's System
 * Health panel is literally the boot check** — same list, same probes, same
 * severity rules. A second health check written against the same backend would
 * drift from this one, and then two screens in the same app would disagree about
 * whether the machine is ready. `onProgress` is called after each check so a
 * caller can render them landing one at a time.
 */
export async function runChecks(list, deps, onProgress) {
  const out = list.map((c) => ({ ...c }));
  for (let i = 0; i < out.length; i++) {
    const c = out[i];
    // A stub check is never "run". It is reported as unknown, immediately, and
    // the screen labels it. Faking a pass here is the exact failure this file
    // exists to prevent.
    if (c.probe === 'stub') {
      out[i] = { ...c, state: 'unknown', note: 'no probe implemented' };
      onProgress?.(out.slice());
      continue;
    }
    out[i] = { ...c, state: 'running', note: '' };
    onProgress?.(out.slice());
    try {
      const r = await (deps[c.id]?.() ?? Promise.resolve({ state: 'unknown', note: 'no probe' }));
      out[i] = { ...c, state: r.state, note: r.note ?? '' };
    } catch (e) {
      out[i] = { ...c, state: 'fail', note: humanError(e) };
    }
    onProgress?.(out.slice());
  }
  return out;
}

/** The boot sequence's wrapper: the same run, written into the shared store. */
export async function runStage(name, deps) {
  const list = get(checks)[name] ?? [];
  const done = await runChecks(list, deps, (partial) =>
    checks.update((all) => ({ ...all, [name]: partial })),
  );
  checks.update((all) => ({ ...all, [name]: done }));
  return rollUp(done);
}

export function resetBoot() {
  checks.set(freshChecks());
  stage.set(STAGES[0]);
  gate.set(null);
  booting.set(true);
}
