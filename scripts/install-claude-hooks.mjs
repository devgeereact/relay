#!/usr/bin/env node
// Register Relay's fast-gate hook on THIS machine.
//
// ## Why this is a script and not a committed settings file
//
// Hooks can only be declared in a Claude Code settings file, and the project-level
// one — `.claude/settings.json` — is, in this repo, full of one developer's
// claude-flow wiring: twelve hook points, a status line, model preferences, daemon
// schedules. None of that is Relay's, and committing it would hand every
// contributor a machine configuration they did not ask for. So `.gitignore` keeps
// it out, and this script adds the one entry Relay actually needs to whatever
// settings file the machine already has.
//
//   npm run hooks:install     # add it (idempotent — safe to run repeatedly)
//   npm run hooks:check       # is it registered? exits 1 if not. Writes nothing
//
// It is deliberately narrow: it appends ONE hook to the PostToolUse
// Write|Edit|MultiEdit matcher and touches nothing else in the file. A backup is
// written next to the original before any change.
//
// The hook itself is `.claude/hooks/relay-fast-gate.mjs` — path-filtered,
// report-only, seconds not minutes. See docs/Working-Agent.md §5.

import { readFileSync, writeFileSync, existsSync, copyFileSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const SETTINGS = resolve(ROOT, '.claude/settings.json');
const HOOK_SCRIPT = resolve(ROOT, '.claude/hooks/relay-fast-gate.mjs');
const MATCHER = 'Write|Edit|MultiEdit';

/** The entry we add. `CLAUDE_PROJECT_DIR` keeps it correct from any working directory. */
const ENTRY = {
  type: 'command',
  command: 'node "${CLAUDE_PROJECT_DIR:-.}/.claude/hooks/relay-fast-gate.mjs"',
  timeout: 90000,
};

/** Recognise our hook however its path was written, so we never install it twice. */
const isOurs = (h) => typeof h?.command === 'string' && h.command.includes('relay-fast-gate.mjs');

const check = process.argv.includes('--check');
const say = (s) => process.stdout.write(`${s}\n`);

if (!existsSync(HOOK_SCRIPT)) {
  say(`✗ ${HOOK_SCRIPT} is missing — nothing to register.`);
  process.exit(1);
}

let settings = {};
if (existsSync(SETTINGS)) {
  try {
    settings = JSON.parse(readFileSync(SETTINGS, 'utf8'));
  } catch (e) {
    // Never overwrite a file we could not read. A malformed settings file is
    // somebody's broken session, not an invitation to replace it.
    say(`✗ ${SETTINGS} is not valid JSON (${e.message}). Fix it first; nothing was written.`);
    process.exit(1);
  }
}

const post = settings.hooks?.PostToolUse ?? [];
const already = post.some((g) => (g.hooks ?? []).some(isOurs));

if (check) {
  say(already ? '✓ Relay fast-gate hook is registered.' : '✗ Relay fast-gate hook is NOT registered — run: npm run hooks:install');
  process.exit(already ? 0 : 1);
}

if (already) {
  say('✓ Already registered — nothing to do.');
  process.exit(0);
}

// Add to the existing Write|Edit|MultiEdit group if there is one, so the file keeps
// its shape; otherwise create that group. Everything else is left untouched.
settings.hooks ??= {};
settings.hooks.PostToolUse ??= [];
const group = settings.hooks.PostToolUse.find((g) => g.matcher === MATCHER);
if (group) {
  group.hooks = [...(group.hooks ?? []), ENTRY];
} else {
  settings.hooks.PostToolUse.push({ matcher: MATCHER, hooks: [ENTRY] });
}

// Back it up if it is there, and create the directory if it is not — decided by
// ATTEMPTING each one rather than by asking first. `existsSync` followed by
// `copyFileSync` is a check-then-use race (CodeQL `js/file-system-race`): nothing
// hostile is plausible on a developer's own `.claude/`, but a scanner that has to
// be told to ignore a pattern stops being read at all, and this one is two lines
// to remove rather than to dismiss.
try {
  copyFileSync(SETTINGS, `${SETTINGS}.bak`);
  say(`  backed up → ${SETTINGS}.bak`);
} catch (e) {
  if (e.code !== 'ENOENT') {
    say(`✗ could not back up ${SETTINGS} (${e.message}). Nothing was written.`);
    process.exit(1);
  }
  // No settings file yet: make sure its directory exists before the write below.
  mkdirSync(dirname(SETTINGS), { recursive: true });
}

writeFileSync(SETTINGS, `${JSON.stringify(settings, null, 2)}\n`);
say('✓ Registered the Relay fast-gate hook in .claude/settings.json.');
say('  It runs on edits to the fire path and the contract surfaces, reports, and never blocks.');
say('  Restart Claude Code (or start a new session) for it to take effect.');
