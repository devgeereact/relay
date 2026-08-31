// RG-76 — the rules that caused real crashes are guarded by prose alone.
//
// `CLAUDE.md`'s "Architecture rules learned the HARD WAY" is forty numbered rules,
// and its own header says: *"These caused real crashes, freezes, or silent failures
// in front of people. Keep them."* Every one is a bug that reached, or would have
// reached, a congregation.
//
// **Most are judgement, and cannot be a test.** "Never hold a `Mutex` across
// `emit`" needs a reader. But five of them are mechanically checkable, and those
// five were protected by nothing at all — a regression would land, pass CI, and be
// found by whoever next ran a service.
//
// Checked here, and *only* those five. A test that guessed at the other
// thirty-five would be worse than none: it would fail on legitimate code, get
// weakened, and take the five real ones down with it.
//
// Each assertion below fails if its rule is broken — verified by breaking each one.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, join } from 'node:path';

const root = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

function tree(dir, ext) {
  const out = [];
  const walk = (d) => {
    for (const e of readdirSync(resolve(root, d), { withFileTypes: true })) {
      const rel = join(d, e.name);
      if (e.isDirectory()) walk(rel);
      else if (e.name.endsWith(ext)) out.push([rel, read(rel)]);
    }
  };
  walk(dir);
  return out;
}

const SVELTE = tree('src', '.svelte');
const RUST = tree('src-tauri/src', '.rs');
const mainRs = read('src-tauri/src/main.rs');

describe('RG-76 · the mechanically checkable hard-way rules', () => {
  it('reads a real tree (the guard on every assertion below)', () => {
    // All five would also pass over an empty file list.
    expect(SVELTE.length).toBeGreaterThan(30);
    expect(RUST.length).toBeGreaterThan(15);
  });

  // ── Rule 1 ────────────────────────────────────────────────────────────────
  it('rule 1 — no `tick()` inside a reactive `$:` block', () => {
    // It re-enters Svelte's scheduler and infinite-loops the webview JS thread:
    // a hard freeze with no error, no log, and no way back but killing the app.
    // `afterUpdate` is the replacement.
    const bad = [];
    for (const [file, src] of SVELTE) {
      src.split('\n').forEach((line, i) => {
        if (/^\s*\$:/.test(line) && /\btick\s*\(/.test(line)) bad.push(`${file}:${i + 1}`);
      });
    }
    expect(bad, `tick() in a reactive block hard-freezes the webview: ${bad.join(', ')}`).toEqual([]);
  });

  // ── Rule 3 ────────────────────────────────────────────────────────────────
  it('rule 3 — the STT worker thread keeps its 16 MB stack', () => {
    // `whisper_full()` is stack-hungry; the default 2 MB overflows into a SILENT
    // SIGSEGV after the first transcript. Nothing reports it — speech recognition
    // simply stops, mid-sermon.
    const stt = read('src-tauri/src/stt.rs');
    const m = stt.match(/\.stack_size\((\d+)\s*\*\s*1024\s*\*\s*1024\)/);
    expect(m, 'the STT worker no longer sets an explicit stack size').toBeTruthy();
    expect(Number(m[1]), 'the stack was reduced below 16 MB').toBeGreaterThanOrEqual(16);
  });

  // ── Rule 9 ────────────────────────────────────────────────────────────────
  it('rule 9 — nobody hand-rolls an app-data path from $HOME', () => {
    // A macOS-only `$HOME/Library/Application Support` variant meant packaged
    // WINDOWS never found the STT model and ran with speech recognition silently
    // dead. Windows has no `HOME`. `db::app_data_dir()` is the one way.
    const bad = [];
    for (const [file, src] of RUST) {
      src.split('\n').forEach((line, i) => {
        if (/^\s*(\/\/|\*)/.test(line)) return; // comments explain the bug on purpose
        if (/env::var\(\s*"HOME"|env!\(\s*"HOME"/.test(line)) bad.push(`${file}:${i + 1}`);
      });
    }
    expect(bad, `use db::app_data_dir() — Windows has no HOME: ${bad.join(', ')}`).toEqual([]);
  });

  // ── Rule 24 ───────────────────────────────────────────────────────────────
  it('rule 24 — the fire path stays generic over `tauri::Runtime`', () => {
    // This is what makes `e2e.rs` possible. Welded to the concrete desktop runtime,
    // the one path that puts scripture on a wall cannot be driven without a window,
    // and so was never tested. A concrete `AppHandle` quietly re-welds it.
    const missing = ['fire_manual', 'handle_nav', 'clear_or_report', 'persist_cue'].filter(
      (fn) => !new RegExp(`fn ${fn}<R: tauri::Runtime>`).test(mainRs),
    );
    expect(
      missing,
      `these must stay generic or e2e.rs cannot drive them: ${missing.join(', ')}`,
    ).toEqual([]);
  });

  // ── Rule 41 ───────────────────────────────────────────────────────────────
  it('rule 41 — no native confirm(), alert() or prompt()', () => {
    // Tauri's webview does not implement them. `confirm()` returns **false without
    // ever showing a dialog**, so a two-step delete guarded by one deletes nothing
    // and reports success — a control that lies. `prompt()` blocks the webview's own
    // event loop.
    //
    // Six files already document this in comments, which is how it was found: it was
    // the one invariant living in `docs/ARCHITECTURE.md` §8 and NOT in CLAUDE.md's
    // numbered list. Now it is rule 41, and now it is checked.
    const bad = [];
    for (const [file, src] of SVELTE) {
      // Strip comments first — every current mention is a comment explaining why
      // these are not used, and flagging those would make the test cry wolf.
      const code = src
        .replace(/\/\*[\s\S]*?\*\//g, '')
        .replace(/<!--[\s\S]*?-->/g, '')
        .split('\n')
        .filter((l) => !/^\s*(\/\/|\*)/.test(l))
        .join('\n');
      for (const m of code.matchAll(/(^|[^.\w])(confirm|alert|prompt)\s*\(/g)) {
        bad.push(`${file}: ${m[2]}()`);
      }
    }
    expect(
      bad,
      `Tauri's webview returns false without showing a dialog — use a two-step ` +
        `arm/confirm or a mounted [role="dialog"]: ${bad.join(', ')}`,
    ).toEqual([]);
  });

  // ── Not a numbered rule: a trap this session nearly walked into ──────────
  it('the files in docs/ that the Rust build compiles in still exist', () => {
    // **`docs/` is not purely documentation.** `db/mod.rs` does
    // `include_str!("../../../docs/data/schema.sql")` and the same for
    // `schema-baseline.sql`, so those two files are compiled into the binary —
    // `schema.sql` IS the shipped baseline schema, and the baseline is what proves
    // every column added since has a migration behind it.
    //
    // Deleting or moving them breaks `cargo build` and **nothing in the frontend
    // suite would notice** — which is the asymmetry that hides things. This session
    // was asked to delete unneeded folders under `docs/`; it checked first, and the
    // next person might not.
    const mod = read('src-tauri/src/db/mod.rs');
    const cited = [...mod.matchAll(/include_str!\("([^"]*docs\/[^"]+)"\)/g)].map((m) => m[1]);
    expect(cited.length, 'db/mod.rs no longer compiles anything in from docs/').toBeGreaterThan(0);
    for (const rel of cited) {
      const path = rel.replace(/^(\.\.\/)+/, '');
      expect(() => read(path), `${rel} is include_str!'d by the Rust build and is gone`).not.toThrow();
    }
  });

  // ── Rule 36 ───────────────────────────────────────────────────────────────
  it('rule 36 — `broadcast_content` has exactly one caller, inside `broadcast_with_clock`', () => {
    // The choke point. It is why the pre-air validator (DECISIONS §42) covers the AI
    // path, the manual box, spoken nav, plan cues, media, announcements and the
    // countdown at once. **A validator added at five call sites is a validator that
    // will be missing from the sixth** — four separate bugs in this repository have
    // that exact shape.
    const callers = [];
    for (const [file, src] of RUST) {
      if (file.endsWith('channels.rs')) continue; // its own definition and tests
      src.split('\n').forEach((line, i) => {
        if (/broadcast_content\s*\(/.test(line) && !/fn broadcast_content/.test(line)) {
          callers.push({ at: `${file}:${i + 1}`, file, line: i + 1 });
        }
      });
    }
    expect(
      callers.map((c) => c.at),
      'broadcast_content must have exactly ONE caller — the choke point is the guarantee',
    ).toHaveLength(1);

    // And that one caller must be inside `broadcast_with_clock`, not merely somewhere
    // in main.rs: the preflight lives in that function.
    const [only] = callers;
    expect(only.file).toBe('src-tauri/src/main.rs');
    const before = mainRs.split('\n').slice(0, only.line);
    const enclosing = [...before].reverse().find((l) => /^fn [a-z_]+/.test(l));
    expect(
      enclosing,
      'the single call moved out of broadcast_with_clock — the preflight is there',
    ).toMatch(/fn broadcast_with_clock/);
  });
});
