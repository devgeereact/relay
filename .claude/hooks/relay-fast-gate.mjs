#!/usr/bin/env node
// THE FAST GATE — the cheap half of Relay's QA setup.
//
// `/qa-audit` is the expensive, exploratory half: six agents, minutes, real tokens,
// run before a release. This is the other half — it fires on every edit, costs a
// couple of seconds, and only ever runs deterministic tests that already exist.
//
// ── The constraint that shapes everything here is LATENCY ────────────────────
//
// `.claude/settings.json` already wires twelve hook points. A hook that adds more
// than a few seconds to every edit gets disabled within a week, and a disabled
// safety net is worse than none, because you still believe it is there. So:
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
  { match: /src\/lib\/views\/library\/LiveOutputRail\.svelte$/, tests: ['liveoutputrail'] },
  { match: /src\/lib\/TemplateRender\.svelte$/, tests: ['layers', 'templatestyle', 'themerender', 'rendercontent'] },
  // Renaming a #[tauri::command] does not break the build, does not fail a test and
  // does not log — it just makes a button quietly stop working. `ipc.test.js` is the
  // only thing standing between that and a Sunday.
  { match: /src-tauri\/src\/main\.rs$/, tests: ['ipc'], rust: 'cargo test e2e' },
  { match: /src-tauri\/src\/(pipeline|router|channels)\.rs$/, tests: [], rust: 'cargo test e2e' },
  { match: /src-tauri\/src\/detection\.rs$/, tests: [], rust: 'cargo test detection:: eval' },
  { match: /src-tauri\/src\/db\//, tests: [], rust: 'cargo test db::' },
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
