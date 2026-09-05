#!/usr/bin/env node
// THE FAST GATE — the cheap half of Relay's QA setup.
//
// `/qa-audit` is the expensive, exploratory half: six agents, minutes, real tokens,
// run before a release. This is the other half — it fires on every edit, costs a
// couple of seconds, and only ever runs deterministic tests that already exist.
//
// ── The constraint that shapes everything here is LATENCY ────────────────────
//
// This is wired on `PostToolUse` for `Write|Edit|MultiEdit` — so it runs on EVERY
// edit to any file, and the path filter below is the only thing keeping that cheap.
// A hook that adds more than a few seconds to every edit gets disabled within a
// week, and a disabled safety net is worse than none, because you still believe it
// is there. So:
//
// (An earlier version of this comment said `.claude/settings.json` "already wires
// twelve hook points". It wires ONE — this one. The latency argument never depended
// on the number, but the number was the premise it was written as, and a false fact
// at the top of a file is exactly what this repository keeps finding inside its own
// instruments. Corrected rather than deleted, per RG-46.)
//
//   1. Path filter FIRST. Almost every edit exits in under a millisecond.
//   2. Only vitest. `cargo test` takes ~50s in this repo — for a Rust fire-path
//      edit this prints the command to run rather than running it. A reminder you
//      read is worth more than a gate you turn off.
//   3. REPORT ONLY. Never exit 2, never block. A blocking contract test will
//      eventually block a legitimate refactor mid-flight, and then it gets removed.
//
// Reads a PostToolUse payload on stdin; emits `additionalContext` so a failure is
// surfaced to the agent that just made the edit, while it still remembers why.

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve, basename } from 'node:path';

const ROOT = process.env.CLAUDE_PROJECT_DIR ?? process.cwd();

/**
 * The fire path plus the contract surfaces: files where a silent break is measured
 * in Sundays. Everything else is somebody's stylesheet.
 */
const WATCHED = [
  { match: /src\/lib\/stores\/capture\.js$/, tests: ['ipc', 'panic', 'transport', 'nav', 'rehearsal', 'micstop'] },
  { match: /src\/lib\/shortcuts\.js$/, tests: ['shortcuts', 'panic'] },
  { match: /src\/lib\/(cues|plan|queue|passage)\.js$/, tests: ['transport', 'plan', 'queue', 'passage'] },
  { match: /src\/lib\/detect\.js$/, tests: ['detect', 'suggestions'] },
  { match: /src\/lib\/errors\.js$/, tests: ['errors'] },
  // Added 2026-08-31. These four shipped with RG-01 … RG-09 and were never added
  // here, so four service-critical files gained tests and no gate. That is the
  // failure this whole file exists to prevent, one level up: a watch list drifts
  // behind the code it watches and goes quiet rather than red.
  //
  // The bar for being on this list is unchanged — **a silent break is measured in
  // Sundays** — and these four clear it:
  //   · outputHealth  the ONE rule for what Live and the Outputs tab may say about
  //                   a screen. Break it and a dead projector reads On Air (RG-01).
  //   · degraded      the shell line on every tab. Break it and a lost model, a
  //                   disarmed detector or a CPU-only build goes silent again.
  //   · updater       refuses to install while capturing OR while a service is
  //                   locked. Break it and Relay restarts mid-sermon.
  { match: /src\/lib\/outputHealth\.js$/, tests: ['outputhealth', 'liveoutputrail'] },
  { match: /src\/lib\/degraded\.js$/, tests: ['degraded'] },
  { match: /src\/lib\/updater\.js$/, tests: ['updatesafety'] },
  { match: /src\/lib\/views\/library\/LiveOutputRail\.svelte$/, tests: ['liveoutputrail'] },
  { match: /src\/lib\/TemplateRender\.svelte$/, tests: ['layers', 'templatestyle', 'themerender', 'rendercontent'] },
  // Renaming a #[tauri::command] does not break the build, does not fail a test and
  // does not log — it just makes a button quietly stop working. `ipc.test.js` is the
  // only thing standing between that and a Sunday.
  { match: /src-tauri\/src\/main\.rs$/, tests: ['ipc'], rust: 'cargo test e2e' },
  { match: /src-tauri\/src\/(pipeline|router|channels)\.rs$/, tests: [], rust: 'cargo test e2e' },
  { match: /src-tauri\/src\/detection\.rs$/, tests: [], rust: 'cargo test detection:: eval' },
  { match: /src-tauri\/src\/db\//, tests: [], rust: 'cargo test db::' },
  //   · servicelock   what may NOT happen while a service records. Break it and a
  //                   mis-click deletes the template that is on the projector.
  { match: /src-tauri\/src\/servicelock\.rs$/, tests: ['servicelock'], rust: 'cargo test servicelock' },
  { match: /src-tauri\/src\/updates\.rs$/, tests: ['updatesafety'], rust: 'cargo test updates' },
];

function emit(context) {
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: { hookEventName: 'PostToolUse', additionalContext: context },
    }),
  );
  process.exit(0);
}

let payload = '';
try {
  payload = await new Promise((res) => {
    let buf = '';
    process.stdin.setEncoding('utf8');
    process.stdin.on('data', (d) => (buf += d));
    process.stdin.on('end', () => res(buf));
    // A hook that hangs on a missing stdin is a hook that hangs the session.
    setTimeout(() => res(buf), 2000).unref?.();
  });
} catch {
  process.exit(0);
}

let file;
try {
  file = JSON.parse(payload || '{}')?.tool_input?.file_path;
} catch {
  process.exit(0);
}
if (!file) process.exit(0);

const norm = file.split('\\').join('/');
const rule = WATCHED.find((w) => w.match.test(norm));
if (!rule) process.exit(0);

const notes = [];

// Frontend: run it. Seconds, and it is the half that catches renamed commands.
const files = rule.tests
  .map((t) => `src/lib/${t}.test.js`)
  .filter((p) => existsSync(resolve(ROOT, p)));

if (files.length) {
  try {
    execFileSync('npx', ['vitest', 'run', ...files], {
      cwd: ROOT,
      encoding: 'utf8',
      stdio: 'pipe',
      timeout: 60_000,
    });
  } catch (e) {
    const out = `${e.stdout ?? ''}${e.stderr ?? ''}`;
    // The failing assertions only — a wall of vitest banner text buries the signal.
    const lines = out
      .split('\n')
      .filter((l) => /(FAIL|×|AssertionError|Expected|Received|→)/.test(l))
      .slice(0, 25)
      .join('\n');
    notes.push(
      `FAST GATE — \`${basename(norm)}\` broke tests that guard it.\n` +
        `Ran: npx vitest run ${files.join(' ')}\n\n${lines || out.slice(-1500)}`,
    );
  }
}

// Rust: name the command, do not run it. A ~50s compile-and-test on every edit is
// how this hook would get deleted.
if (rule.rust) {
  notes.push(
    `FAST GATE — \`${basename(norm)}\` is on the fire path. Not run here (too slow ` +
      `for an edit hook); run it before you finish:\n    cd src-tauri && ${rule.rust}`,
  );
}

if (notes.length) emit(notes.join('\n\n'));
process.exit(0);
