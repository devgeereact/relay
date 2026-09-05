// RG-69 — the edit-time gate can only guard files that still exist.
//
// `.claude/hooks/relay-fast-gate.mjs` runs on every edit and re-runs the tests that
// guard the file just changed. Its whole value is that it is quiet when things are
// fine — which is also how it fails: **a rule whose path no longer matches anything,
// or that names a test file that has been renamed, simply stops firing.** It does
// not error. It goes quiet, and quiet is what "everything is fine" looks like.
//
// That is the same failure this repository found three times in one day at a larger
// scale: an event scanner reading one Rust file, a command contract reading four
// frontend files out of nine, and a CI job on a single Node version. In each case
// the instrument had narrowed and nothing said so.
//
// This test cannot decide what SHOULD be watched — that is a judgement about which
// silent breaks are measured in Sundays, and the hook states the criterion in prose.
// What it can do is guarantee that every rule already there still points at
// something real.
import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync, readdirSync } from 'node:fs';
import { resolve, join } from 'node:path';

const root = resolve(__dirname, '../..');
const HOOK = 'src/../.claude/hooks/relay-fast-gate.mjs';
const src = readFileSync(resolve(root, '.claude/hooks/relay-fast-gate.mjs'), 'utf8');

/** Every `{ match: /…/, tests: [...] }` rule, read out of the source. */
function rules() {
  const block = src.slice(src.indexOf('const WATCHED = ['), src.indexOf('function emit('));
  return [...block.matchAll(/\{\s*match:\s*\/(.+?)\/,\s*tests:\s*\[([^\]]*)\]/g)].map((m) => ({
    pattern: m[1],
    tests: [...m[2].matchAll(/'([^']+)'/g)].map((t) => t[1]),
  }));
}

/** Every source file the hook could ever be handed. */
function allFiles() {
  const out = [];
  const walk = (dir) => {
    for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
      if (e.name === 'node_modules' || e.name === 'target' || e.name === 'dist') continue;
      const rel = join(dir, e.name);
      if (e.isDirectory()) walk(rel);
      else out.push(rel);
    }
  };
  walk('src');
  walk('src-tauri/src');
  return out;
}

const RULES = rules();
const FILES = allFiles();

describe('RG-69 · the fast gate still points at real things', () => {
  it('the rules parse at all (the guard on everything below)', () => {
    // If the hook's shape changes, this must fail loudly rather than quietly
    // asserting over an empty list — which would pass.
    expect(RULES.length).toBeGreaterThan(8);
    expect(RULES.some((r) => r.pattern.includes('capture'))).toBe(true);
  });

  it('every watched path matches at least one file that exists', () => {
    const dead = RULES.filter((r) => {
      const re = new RegExp(r.pattern);
      return !FILES.some((f) => re.test(f));
    }).map((r) => r.pattern);
    // A rule that matches nothing is a gate that has silently stopped firing —
    // the file was renamed or deleted and the hook never said a word.
    expect(dead, `watched paths that match no file: ${dead.join(', ')}`).toEqual([]);
  });

  it('every test file a rule names exists', () => {
    const missing = [];
    for (const r of RULES) {
      for (const t of r.tests) {
        const p = `src/lib/${t}.test.js`;
        if (!existsSync(resolve(root, p))) missing.push(`${r.pattern} → ${p}`);
      }
    }
    // The hook filters these with `existsSync` and runs whatever is left, so a
    // renamed test file does not fail — it just stops being run, and a rule whose
    // every test has been renamed runs nothing while still looking watched.
    expect(missing, `rules naming test files that do not exist: ${missing.join(', ')}`).toEqual([]);
  });

  it('the safety files this gate exists for are watched', () => {
    // Not an exhaustive list and not trying to be — the hook's own comment carries
    // the criterion ("a silent break is measured in Sundays"). These are the ones
    // that were added to the product and NOT added here, which is what prompted
    // this test: four files shipped with RG-01 … RG-09 and gained no gate.
    const mustWatch = [
      'src/lib/stores/capture.js',
      'src/lib/shortcuts.js',
      'src/lib/outputHealth.js',
      'src/lib/degraded.js',
      'src/lib/updater.js',
      'src-tauri/src/servicelock.rs',
      'src-tauri/src/pipeline.rs',
    ];
    const unwatched = mustWatch.filter(
      (f) => !RULES.some((r) => new RegExp(r.pattern).test(f)),
    );
    expect(unwatched, `service-critical files with no fast-gate rule: ${unwatched.join(', ')}`).toEqual([]);
  });
});
