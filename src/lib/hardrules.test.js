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

/**
 * Blank out `/* … *\/` and `<!-- … -->` regions, keeping every other character.
 *
 * Walks the text once and replaces the inside of each comment with spaces, so
 * offsets and line numbers are unchanged and an unterminated comment runs to the
 * end rather than being silently dropped.
 */
function stripComments(src) {
  const out = src.split('');
  let i = 0;
  while (i < src.length) {
    const block = src.startsWith('/*', i) ? '*/' : src.startsWith('<!--', i) ? '-->' : null;
    if (!block) {
      i += 1;
      continue;
    }
    const end = src.indexOf(block, i + 2);
    const stop = end === -1 ? src.length : end + block.length;
    for (let j = i; j < stop; j += 1) if (out[j] !== '\n') out[j] = ' ';
    i = stop;
  }
  return out.join('');
}

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
      //
      // Blanked by a WALK rather than by `.replace(/<!--[\s\S]*?-->/g, '')`.
      // That regex is the shape CodeQL calls an incomplete multi-character
      // sanitizer (`js/incomplete-multi-character-sanitization`), and although
      // nothing here sanitizes anything — the input is this repository's own
      // source and the output is never rendered — the alert is right that the
      // pattern mishandles overlapping delimiters. A scanner that has to be told
      // to ignore a pattern stops being read at all, so it is removed rather than
      // dismissed. The walk is also simply more correct: it cannot leave a
      // trailing `<!--` behind, and it never rewrites the file's length.
      const code = stripComments(src)
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

  // ── Not a numbered rule: a constant four documents hard-code ─────────────
  it('RG-98 · the dev server does not put itself on the LAN unless asked', () => {
    // It bound every interface by default, and `npm audit`'s two reachable
    // advisories — Vite's optimized-deps path traversal and esbuild's "any website
    // can send any request to the dev server and read the response" — are reachable
    // exactly through that. The fixes are all semver majors, so the version numbers
    // are not the lever; who can connect is.
    const cfg = read('vite.config.js');
    expect(cfg, 'the LAN opt-in is gone').toMatch(/RELAY_DEV_LAN/);
    for (const block of ['server', 'preview']) {
      const at = cfg.indexOf(`  ${block}: {`);
      expect(at, `${block} block is gone — update this test with it`).toBeGreaterThan(-1);
      const body = cfg.slice(at, cfg.indexOf('},', at));
      expect(body, `${block} binds every interface by default`).not.toMatch(/host:\s*true/);
      expect(body, `${block} does not read the opt-in`).toMatch(/host:\s*DEV_LAN/);
    }
  });

  it('the docs cite the same default sensitivity the router actually uses', () => {
    // `Thresholds::default()` calls `from_sensitivity(DEFAULT_SENSITIVITY)`, so the
    // "exactly ONE baseline" invariant is unrepresentable otherwise — good. But four
    // documents write the NUMBER out as `from_sensitivity(50)`, and the seed
    // thresholds they quote beside it (0.50 auto-fire / 0.35 suggest) are the
    // safety-relevant half.
    //
    // Two baselines once existed and disagreed (0.50/0.35 vs 0.90/0.60), and a
    // profile save silently snapped the live gate from one scale to the other,
    // wiping the operator's calibration. Changing the constant without the docs
    // would put that number back into circulation on paper.
    //
    // Reads the constant and requires the docs to agree — drift in EITHER direction
    // fails, and nothing here needs updating when the constant legitimately moves.
    const router = read('src-tauri/src/router.rs');
    const m = router.match(/DEFAULT_SENSITIVITY:\s*u8\s*=\s*(\d+)/);
    expect(m, 'DEFAULT_SENSITIVITY is gone or no longer a literal').toBeTruthy();
    const actual = m[1];

    // ONLY the "one baseline" claim — `default() == from_sensitivity(N)`. A bare
    // `from_sensitivity(100)` is a legitimate and different statement about the TOP
    // of the dial, and the first version of this test flagged one, which would have
    // shipped a false failure. Match the claim, not the call.
    const wrong = [];
    const CLAIM = /default\(\)\s*==\s*from_sensitivity\((\d+)\)/g;
    let checked = 0;
    for (const doc of ['CLAUDE.md', 'docs/DATA_MODEL.md', 'docs/DECISIONS.md', 'docs/ARCHITECTURE.md']) {
      for (const c of read(doc).matchAll(CLAIM)) {
        checked += 1;
        if (c[1] !== actual) wrong.push(`${doc} says default() == from_sensitivity(${c[1]})`);
      }
    }
    // The claim is load-bearing enough that its disappearance is also a finding.
    expect(checked, 'no document states the one-baseline claim any more').toBeGreaterThan(1);
    expect(
      wrong,
      `the router's DEFAULT_SENSITIVITY is ${actual}: ${wrong.join(', ')}`,
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

  // ── Rule 33, the two hops it did not cover ────────────────────────────────
  it('rule 33 — no unbounded queue anywhere on the audio path', () => {
    // RG-84. Rule 33 describes the queue into detection as bounded, shedding
    // partials and COUNTING what it sheds. That was true of the third hop. The two
    // in front of it — the capture callback into the clean/chunk thread, and that
    // thread into the whisper worker — were plain `mpsc::channel()` with no
    // capacity and no counter, and the handbook's wording read as though the whole
    // path was bounded.
    //
    // An unbounded queue in front of a decoder does not prevent a backlog. It
    // converts one into memory, and then into a transcript arriving minutes after
    // the sentence — which is rule 31's failure exactly: a pipeline permanently
    // behind reports a *zero backlog* for a whole service, and every instrument
    // reads green.
    //
    // Checked here rather than in Rust because it is a claim about the SHAPE of the
    // code, and the shape is what regressed. `mpsc::channel` is a one-word edit away
    // from returning.
    const AUDIO_PATH = ['src-tauri/src/audio.rs', 'src-tauri/src/stt.rs'];
    const unbounded = [];
    for (const [file, src] of RUST) {
      if (!AUDIO_PATH.includes(file)) continue;
      src.split('\n').forEach((line, i) => {
        // `sync_channel(N)` is the bounded one. Anything else that constructs an
        // mpsc channel here is not.
        // The turbofish may itself contain `>` (`::<Vec<f32>>`), so this must not
        // be `[^>]*`. It was, and the check silently matched nothing — the same
        // way every other scanner in this repository has failed: by narrowing and
        // staying green. The guard below counts what it found for that reason.
        if (/mpsc::channel\s*(::<.*>)?\s*\(/.test(line)) {
          unbounded.push(`${file}:${i + 1}`);
        }
      });
    }
    expect(
      unbounded,
      `unbounded mpsc channels on the audio path: ${unbounded.join(', ')}`,
    ).toEqual([]);

    // THE GUARD ON THE ASSERTION ABOVE. It would also pass over a regex that
    // matches nothing, which is precisely how it was first written. Both files must
    // still construct the BOUNDED form, so a scanner that has stopped seeing
    // channels fails here instead of reporting a clean path.
    const bounded = [];
    for (const [file, src] of RUST) {
      if (!AUDIO_PATH.includes(file)) continue;
      if (/mpsc::sync_channel\s*(::<.*>)?\s*\(/.test(src)) bounded.push(file);
    }
    expect(bounded.sort(), 'the scanner can no longer see either audio queue').toEqual(
      [...AUDIO_PATH].sort(),
    );

    // A bound with no counter is worse than no bound: it turns unbounded memory
    // into a silent gap in the sermon. Both producers must count what they shed.
    const audio = read('src-tauri/src/audio.rs');
    const main = read('src-tauri/src/main.rs');
    expect(audio, 'the capture callback sheds without counting').toMatch(
      /note_dropped_audio/,
    );
    expect(main, 'the chunk hand-off to whisper sheds without counting').toMatch(
      /note_dropped_audio/,
    );
    // And the capture callback must never BLOCK — stalling a device's real-time
    // callback is how a capture stream is killed outright.
    expect(audio).toMatch(/try_send/);
  });
});
