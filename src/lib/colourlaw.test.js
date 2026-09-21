// THE COLOUR LAW, HELD AT THE TWO TAXONOMY TABLES THAT BROKE IT.
//
// CLAUDE.md rule 18 / DECISIONS §21 / REBRAND §1: amber = ON AIR, amethyst =
// rehearsal, cyan = a guess, grey = CUED, red = destructive. A colour that
// carries a promise may be used ONLY where it means what it says.
//
// `plan.js` had two tables that each painted a KIND rather than a STATE, and
// between them they spent every one of those colours — on Live, the run surface,
// inches from the rules that signal the real states. A song cue's stripe was
// ON-AIR amber; a Chorus chip was ON-AIR amber and more saturated than a
// genuinely live row, whose amber is a 15% wash.
//
// This file fails if either table reaches for a promise colour again. It tests
// the VALUES the tables return, not the stylesheet, because that is where the
// defect lived: `Live.svelte` correctly wrote "Amber = it is in front of the
// congregation. Nothing else may use it" five lines above the rule that painted
// `--acc` with whatever `slideAccent` handed it.
import { describe, it, expect } from 'vitest';
import { codeOnly } from './codeonly.js';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { TYPE, TAXONOMY_INK, slideAccent, typeOf } from './plan.js';

/** Every token that carries a promise, with what it promises. */
const PROMISE = {
  '--v-amber': 'ON AIR',
  '--v-cyan': 'a guess',
  '--v-amethyst': 'rehearsal',
  '--v-grey': 'CUED',
  '--v-rose': 'destructive',
  '--v-red': 'destructive',
};

// ── THE INDIRECTION THIS SCANNER USED TO MISS ───────────────────────────────
//
// `plan.js` records the gap in the paragraph that refuses a per-kind ramp for the
// second time: "Its sweep over every `TYPE` entry is a SUBSTRING match on the
// token text, so `var(--v-col-scripture)` sails through it — the indirection is
// invisible to the scanner even though it resolves to amber."
//
// That was exact, and it was the dangerous kind of gap: `--v-col-scripture` IS
// `var(--v-amber)` and `--v-col-media` IS `var(--v-amethyst)` (tokens.css), so a
// taxonomy built out of the `--v-col-*` family would have painted ON AIR on the
// one cue kind the AI may fire by itself, and every grep for `--v-amber` would
// have come back clean. The wave-4 proposal asked for exactly that.
//
// So the matcher now RESOLVES. It reads both halves of the stylesheet, follows a
// `var()` chain to its literal, and reports the whole path — `--v-col-scripture
// → --v-amber (ON AIR)` — because a reader who is told only "amber" will look for
// an amber that is not in the file. A cycle terminates rather than recursing; a
// token that is not defined resolves to nothing and is not a false positive.
const STYLESHEET = ['../tokens.css', '../app.css']
  .map((f) => readFileSync(resolve(__dirname, f), 'utf8'))
  .join('\n');

/** The declared value of `--token`, or null. First definition wins, as CSS does not. */
function declarationOf(token) {
  const m = STYLESHEET.match(new RegExp(`${token.replace(/[-]/g, '\\-')}\\s*:\\s*([^;}]+)[;}]`));
  return m ? m[1].trim() : null;
}

/** Every `--token` a value names directly. */
function tokensIn(value) {
  return [...String(value || '').matchAll(/var\(\s*(--[\w-]+)/g)].map((m) => m[1]);
}

/**
 * Names a promise token, following `var()` indirection to any depth.
 *
 * The substring half is unchanged and still wanted: a VARIANT of a promise colour
 * is still that promise (`--v-amber-soft`, `--v-amber2`, `--v-cyan-line`), and no
 * non-promise token in the stylesheet begins with one of these names.
 *
 * The resolving half is the addition. It walks each named token's declaration,
 * and each token THAT names, until it reaches a literal or runs out — so an alias
 * of an alias of amber is still amber.
 */
function promiseIn(value, seen = new Set(), path = []) {
  const v = String(value || '');
  for (const [token, means] of Object.entries(PROMISE)) {
    if (v.includes(token)) {
      const via = [...path, token].join(' → ');
      return `${via} (${means})`;
    }
  }
  for (const token of tokensIn(v)) {
    if (seen.has(token)) continue; // a cycle is not a promise
    seen.add(token);
    const decl = declarationOf(token);
    if (!decl) continue; // undefined token — nothing to follow, not a hit
    const hit = promiseIn(decl, seen, [...path, token]);
    if (hit) return hit;
  }
  return null;
}

describe('the colour law — a taxonomy may not paint a promise', () => {
  it('the scanner recognises a promise colour when it sees one', () => {
    // Guards the guard. A matcher that silently matched nothing would make every
    // assertion below pass vacuously, which is how two scanners in this repo were
    // wrong while looking exhaustive.
    expect(promiseIn('var(--v-amber)')).toContain('ON AIR');
    expect(promiseIn('var(--v-amethyst-soft)')).toContain('rehearsal');
    expect(promiseIn('var(--v-cyan-line)')).toContain('a guess');
    expect(promiseIn('var(--v-rose)')).toContain('destructive');
    expect(promiseIn('var(--v-faint)')).toBeNull();
    expect(promiseIn('var(--v-sel)')).toBeNull();
  });

  it('…and it now FOLLOWS an alias, which is the gap plan.js recorded', () => {
    // THE HOLE THIS CLOSES, named. `plan.js` says of the pre-2026-09-20 version of
    // this file: "`var(--v-col-scripture)` sails through it — the indirection is
    // invisible to the scanner even though it resolves to amber". A taxonomy built
    // out of that family would have painted ON AIR on the one cue kind the AI may
    // fire by itself, with every grep for `--v-amber` coming back clean.
    //
    // Watched to fail against the substring-only matcher: all four of these
    // returned null, which is how the whole `--v-col-*` proposal would have passed.
    expect(promiseIn('var(--v-col-scripture)')).toContain('ON AIR');
    expect(promiseIn('var(--v-col-media)')).toContain('rehearsal');
    expect(promiseIn('var(--v-col-notice)')).toContain('destructive');
    // The PATH is reported, not just the verdict: a reader told only "amber" goes
    // looking for an amber that is not written anywhere in the file.
    expect(promiseIn('var(--v-col-scripture)')).toContain('--v-col-scripture → --v-amber');
  });

  it('…without inventing a promise where there is none', () => {
    // The opposite mistake. A resolver that flagged everything would be as useless
    // as one that flagged nothing, and the cheapest way to go green on a false
    // positive is to weaken the scanner.
    expect(promiseIn('var(--v-col-song)'), '--v-col-song is steel, i.e. selection').toBeNull();
    expect(promiseIn('var(--v-accent)'), '--v-accent is an alias of --v-sel').toBeNull();
    expect(promiseIn('var(--v-accent-soft)')).toBeNull();
    // The section bands, which is what this strengthening was written alongside.
    // They resolve to hexes of their own — the point of the exercise.
    expect(promiseIn('var(--v-sec-a)')).toBeNull();
    expect(promiseIn('var(--v-sec-b)')).toBeNull();
    expect(promiseIn('var(--v-sec-a-soft)')).toBeNull();
    // A token nothing defines is not a hit — it is a typo, and a different test's
    // problem. Claiming it is amber would be a verdict from an absence.
    expect(promiseIn('var(--v-not-a-real-token)')).toBeNull();
    // And a cycle terminates rather than recursing for ever.
    expect(promiseIn('var(--v-faint)', new Set(['--v-faint']))).toBeNull();
  });

  it('no cue type paints a promise colour', () => {
    const offenders = Object.entries(TYPE)
      .map(([kind, row]) => [kind, promiseIn(row.color)])
      .filter(([, hit]) => hit)
      .map(([kind, hit]) => `TYPE.${kind} → ${hit}`);
    expect(offenders).toEqual([]);
  });

  it('every cue type still NAMES its kind, because the label is now the taxonomy', () => {
    // The colour was removed on the argument that the words already carry it.
    // If a label were ever blank, that argument would be false and the kind
    // would be unreadable rather than merely uncoloured.
    for (const [kind, row] of Object.entries(TYPE)) {
      expect(row.label, `TYPE.${kind}.label`).toBeTruthy();
      expect(row.trig, `TYPE.${kind}.trig`).toBeTruthy();
    }
  });

  it('no slide tag paints a promise colour — including the ones that used to', () => {
    const tags = [
      'C', 'C1', 'C2', 'CHORUS', // was --v-amber, ON AIR
      'V', 'V1', 'V2', 'VERSE', // was --v-cyan, a guess
      'BR', 'B', 'B2', 'BRIDGE', // was --v-amethyst, rehearsal
      'NOTE', 'OUT', 'OUTRO', 'END', 'TAG', 'REF', // was --v-rose, destructive
      'BG', // was --v-amethyst, rehearsal
      'PC', 'PC2', 'INT', 'IL', // was --v-emerald
      '1', '2', '17', // was --v-faint, a hair from --v-grey / CUED
      '⏱', 'SCR', '', null, undefined, 'ZZZ-UNKNOWN',
    ];
    const offenders = tags
      .map((t) => [t, promiseIn(slideAccent(t))])
      .filter(([, hit]) => hit)
      .map(([t, hit]) => `slideAccent(${JSON.stringify(t)}) → ${hit}`);
    expect(offenders).toEqual([]);
  });

  it('both tables answer with the one taxonomy ink', () => {
    expect(slideAccent('C')).toBe(TAXONOMY_INK);
    expect(typeOf('song').color).toBe(TAXONOMY_INK);
    expect(promiseIn(TAXONOMY_INK)).toBeNull();
  });

  it('slideAccent does not mention a promise colour in its own source', () => {
    // The value test above can only see the tags it thought to try. This sees the
    // function: a new branch returning amber for some tag nobody listed fails here.
    //
    // IT USED TO NAME THE SIX TOKENS AND STOP THERE, which is the same substring
    // gap in a second place — a branch returning `var(--v-col-scripture)` passed
    // it. It now resolves every token the body names, through `promiseIn`.
    const src = readFileSync(resolve(__dirname, './plan.js'), 'utf8');
    const body = src.slice(src.indexOf('export function slideAccent'));
    const fn = body.slice(0, body.indexOf('\n}') + 2);
    const offenders = tokensIn(fn)
      .map((t) => [t, promiseIn(`var(${t})`)])
      .filter(([, hit]) => hit)
      .map(([t, hit]) => `slideAccent returns ${t} → ${hit}`);
    expect(offenders).toEqual([]);
  });

  it('every colour plan.js hands out at all resolves clear of a promise', () => {
    // The widest form of the sweep, and the one that does not depend on somebody
    // remembering to add a new table to this file. `plan.js` is the module whose
    // two taxonomy tables broke the law; this reads EVERY `var(--…)` in it and
    // resolves each one, so a third table — or a fourth — is covered on arrival.
    //
    // `SECTION_BANDS` is what made this worth writing: it is a third table in this
    // file, added the same day, and neither of the two existing sweeps would have
    // looked at it.
    const src = readFileSync(resolve(__dirname, './plan.js'), 'utf8');
    // Code only — the file's doc comments QUOTE the promise tokens at length in
    // order to explain what must not be done with them, and flagging that would
    // make deleting the explanation the cheapest way to go green.
    const offenders = tokensIn(codeOnly(src))
      .map((t) => [t, promiseIn(`var(${t})`)])
      .filter(([, hit]) => hit)
      .map(([, hit]) => `plan.js hands out ${hit}`);
    expect(offenders).toEqual([]);
    // Guards the guard: the scan must actually be seeing the tokens in the file.
    expect(tokensIn(codeOnly(src)).length).toBeGreaterThan(3);
  });

  it('Live still reserves amber for ON AIR, and a preview is steel — never amber', () => {
    // The other half of the law: the promise colours must still be used where they
    // DO mean what they say. Removing them from the taxonomy is only half a fix if
    // the states stop being signalled.
    //
    // THE CUE RAIL THIS USED TO READ HAS GONE. `.cue` and `.slide` were the plan
    // pane's rows; the plan IS the grid now (docs/REBRAND.md §2), so the two
    // states it signalled are signalled on the CELL instead — and the cell is
    // what the operator presses, which is the better place for both. The law is
    // unchanged: amber only for what a congregation is looking at, steel for the
    // thing you are working on, and a preview is never amber.
    const live = readFileSync(resolve(__dirname, './views/Live.svelte'), 'utf8');
    expect(live).toMatch(/\.sg-cell\.islive \.sg-thumb\{border-color:var\(--v-amber\)/);
    expect(live).toMatch(/\.sg-air\{background:var\(--v-amber\)/);
    // Steel, not cyan (a guess) and not amber (on air).
    expect(live).toMatch(/\.sg-cell\.cued \.sg-thumb\{border-color:var\(--v-sel\)/);
    expect(live).toMatch(/\.sg-prev\{background:var\(--v-sel\)/);
    expect(live).not.toMatch(/\.sg-prev\{background:var\(--v-amber/);
    expect(live).not.toMatch(/\.sg-cell\.cued \.sg-thumb\{border-color:var\(--v-(amber|cyan)/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// AMETHYST'S BOUNDARY — DECISIONS §93.
//
// Everything above holds the law at the two `plan.js` tables that broke it. This
// block holds the one colour whose RULE was self-contradicting rather than whose
// call sites were: `src/app.css` said "Rehearse amethyst — the rehearsal colour,
// and nothing else uses it", in a file that spends amethyst on about fourteen
// other surfaces.
//
// §93 settles it by reading those fourteen instead of counting them. Amethyst's
// promise is "NOTHING HERE REACHES A CONGREGATION"; rehearsal is the case it was
// named for and not the whole of it, because safe mode ("outputs disabled") and
// the launch sequence (no console, no output window, nothing on any wall) are the
// same fact said where the word "rehearsal" does not fit.
//
// What §93 deliberately leaves OPEN is caution. Four surfaces spend amethyst on a
// warning, each with the same argument recorded independently at its own call
// site: amber is the tally light, cyan means the AI is guessing, red means
// failure, and a caution is none of those. This palette publishes no caution ink.
// That gap is filed rather than improvised, and the four are enumerated below with
// that reason attached so the list can only shrink.
//
// A HUMAN MAY OVERRULE THIS. Ruling that caution is simply inside amethyst's
// promise, or adding a sixth colour, each changes this file by one list.
describe('the colour law — amethyst promises that nothing here reaches a congregation', () => {
  const ROOT = resolve(__dirname, '../..');
  const read = (f) => readFileSync(resolve(ROOT, f), 'utf8');
  /** Source with comments removed: only the code is the claim. */
  // ONE STRIPPER, SHARED. Three regexes here was the shape that blanked 7 KB of
  // `Stage.svelte` in `names.test.js`: a comment containing `:8032/api/*` opened a
  // block comment that ran to the next real `*/`. `codeOnly` scans left to right,
  // knows a quoted run cannot open a comment, and BLANKS rather than deletes, so
  // every offset in the result still matches the file.
  const code = (f) => codeOnly(read(f));

  /** Every .svelte under src/, derived rather than typed. */
  const components = (() => {
    const out = [];
    const walk = (dir) => {
      for (const name of readdirSync(join(ROOT, dir))) {
        const rel = `${dir}/${name}`;
        if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
        else if (name.endsWith('.svelte')) out.push(rel);
      }
    };
    walk('src');
    return out;
  })();

  const paintsAmethyst = (f) => /var\(--v-amethyst/.test(code(f));

  it('the scanner can see an amethyst, and does not see one in a comment', () => {
    // Guards the guard, for the third time in this repository and for the same
    // reason: `ipc.test.js` narrowed quietly twice while looking exhaustive, and a
    // sweep that matched nothing would make every assertion below pass vacuously.
    expect(components.length, 'no components found at all').toBeGreaterThan(30);
    expect(paintsAmethyst('src/lib/Splash.svelte'), 'Splash no longer paints amethyst').toBe(true);
    // `TemplateRender.svelte` mentions amethyst ONLY in prose explaining what it
    // must not do. A scanner that counted that would fail on a correct file, and
    // the cheapest way to make it green would be to delete the explanation.
    expect(read('src/lib/TemplateRender.svelte')).toMatch(/amethyst/i);
    expect(paintsAmethyst('src/lib/TemplateRender.svelte')).toBe(false);
  });

  it('NO congregation-facing surface paints it — the half that can never be argued', () => {
    // A promise that says "nothing here reaches a congregation" is self-refuting
    // the moment it is painted on something a congregation can see. These three
    // are the surfaces that reach a wall: the output window, the preacher's stage
    // page, and the one renderer both of them use.
    for (const f of ['src/Output.svelte', 'src/Stage.svelte', 'src/lib/TemplateRender.svelte']) {
      expect(paintsAmethyst(f), `${f} paints amethyst on a surface that reaches a room`).toBe(false);
    }
  });

  it('and every surface that does paint it is one of the five kinds §93 allows', () => {
    // Enumerated by file WITH its reason, in the shape `buttonshapes.test.js` uses
    // for its kept shapes: the only thing separating a deliberate use from drift is
    // somebody having said which it is. Widening this list is a decision; quietly
    // adding to it to make a build green is the drift it exists to catch.
    const ALLOWED = new Map([
      // REHEARSAL — the case the colour was named for.
      ['src/lib/views/Live.svelte', 'the rehearsal banner, dot, tag and programme ring'],
      ['src/lib/views/library/VerseDeck.svelte', 'the rehearsal card (.vd-card.reh)'],
      // SAFE MODE — "outputs disabled" is the promise, word for word. app.css's
      // own lamp comment already records safe mode as outranking rehearsal.
      ['src/lib/boot/SafeModeStartup.svelte', 'safe mode'],
      ['src/lib/boot/BootShell.svelte', 'the safe-mode line: outputs disabled'],
      // THE LAUNCH SEQUENCE — the console is not mounted and nothing is on a wall.
      // `.b-shell` renders instead of `App.svelte`, so a boot ladder and a
      // rehearsal badge can never be on screen together and there is no moment at
      // which an operator has to tell one amethyst from another.
      ['src/lib/Splash.svelte', 'the launch sequence'],
      ['src/lib/ui/BrandMark.svelte', 'the launch sequence lockup'],
      ['src/lib/boot/UpdateAvailable.svelte', 'the launch sequence'],
      // ── THE CAUTION GAP, §93's open question — PAID on 2026-09-21 ──────────
      // Five entries used to sit here (Settings' .s-netwarn, Help's callout,
      // LyricsPane's .ly-warn, ModelSetup's .ms-caution and .ms-locked, and
      // app.css's .b-check.warn), each amethyst because this palette published
      // no caution ink. It does now: `--v-caution` (DECISIONS §111, RG-207), and
      // the caution sweep at the bottom of this file enumerates every surface
      // that wears it. Nothing in this map is a caution any more, and a caution
      // arriving here again is the drift the gap was named to make visible.
    ]);
    // WHAT THIS DOES NOT SEE, said so it is not read as more. It matches
    // `var(--v-amethyst…)`, so a component that wears the shared `.r-badge
    // amethyst` or `.r-cbtn.rehearse` class is painted by `app.css` and is outside
    // this sweep by construction. That is the right boundary — those classes carry
    // the meaning at their definition, which is where `app.css` argues for it —
    // but it means this list is the files that paint amethyst THEMSELVES, not
    // every file on which amethyst appears.
    const offenders = components.filter((f) => paintsAmethyst(f) && !ALLOWED.has(f));
    expect(
      offenders,
      'amethyst means "nothing here reaches a congregation" — say which kind this is in DECISIONS §93, or use --v-sel',
    ).toEqual([]);

    // The list may only shrink. An entry for a file that has stopped painting
    // amethyst is an amnesty waiting for the next component of that name.
    const stale = [...ALLOWED.keys()].filter((f) => !paintsAmethyst(f));
    expect(stale, 'these no longer paint amethyst — take them out of the list').toEqual([]);
  });

  it('and no caution wears amethyst any more — the gap is paid, not grown', () => {
    // This test used to count the cautions borrowing amethyst: four, then five
    // when ModelSetup's amber violation was paid into the gap. On 2026-09-21 the
    // gap was closed the other way — a caution ink exists (§111) — so the count
    // is zero and the assertion is that it STAYS zero. The five former borrowers
    // are named so that one of them drifting back is legible as itself.
    const formerly = [
      'src/lib/views/Settings.svelte',
      'src/lib/views/Help.svelte',
      'src/lib/views/library/LyricsPane.svelte',
      'src/lib/ModelSetup.svelte',
    ];
    for (const f of formerly) expect(paintsAmethyst(f), `${f} borrows amethyst for a caution again`).toBe(false);
    expect(read('src/app.css')).toMatch(/\.b-check\.warn \.ico\{ color:var\(--v-caution2\); \}/);
    // And app.css no longer claims what it cannot support. The sentence that used
    // to sit above the four control buttons said "the rehearsal colour, and
    // nothing else uses it", in a file spending amethyst fourteen other ways.
    // The stylesheet's own claim about itself. This one cannot be checked against
    // comment-stripped source, because the claim IS a comment — the whole control
    // metrics block is one — and the replacement necessarily QUOTES the retired
    // sentence in order to explain what was wrong with it. Stripping comments would
    // make the assertion vacuous; reading them raw would flag the explanation as
    // the offence, and the cheapest way to go green on that is to delete the
    // explanation, which is the more valuable half. `workspacegrammar.test.js`
    // records the same lesson in the same words.
    //
    // So it asks the question that actually distinguishes the two: is every
    // occurrence of the retired sentence a QUOTATION? Written as a live claim it
    // has no "used to read" in front of it, and this fails.
    const css = read('src/app.css');
    const RETIRED = 'the rehearsal colour, and nothing else uses it';
    for (let i = css.indexOf(RETIRED); i !== -1; i = css.indexOf(RETIRED, i + 1)) {
      expect(
        css.slice(Math.max(0, i - 120), i),
        'the retired sentence is back as a live claim rather than as a quotation',
      ).toMatch(/used to read/);
    }
    expect(css).toMatch(/NOTHING HERE REACHES A CONGREGATION/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// AMBER — THE SWEEP THIS FILE DID NOT HAVE, AND THE ONE VIOLATION IT MISSED.
//
// Everything above holds the law at `plan.js`'s two taxonomy tables, which is
// where it was broken when this file was written, plus amethyst's boundary
// across every component. Amber — the colour with the loudest promise of the
// five — had no component sweep at all. Its only guard was the two taxonomy
// tables and four hand-written lines about `Live.svelte`.
//
// So this was live and nothing could see it: `ModelSetup.svelte` painted the
// *Download — recommended* button `class:amber={m.recommended}` — the ON AIR
// fill, `--v-amber` behind `--v-amber-ink` with an amber glow, on a download
// button in a settings panel. DESIGN_SYSTEM §1.1 says amber "survives only as
// `.r-badge.amber`, `.r-btn.amber` and `.r-stat.amber` — the on-air cases, named
// explicitly at the call site so reaching for one is a DECISION rather than a
// default". That is the right mechanism and it had no test, so the decision was
// never reviewed; a recommended download is not a congregation looking at
// anything.
//
// TWO SWEEPS, because amber reaches a component two ways and a test that saw
// only one would look exhaustive while checking half:
//
//   1. the shared CLASS (`class:amber`, `class="r-btn amber"`, `'amber'` inside
//      a class expression) — app.css paints it, so the component names no token;
//   2. the raw TOKEN (`var(--v-amber…)`) in the component's own stylesheet.
//
// Both are enumerated WITH a reason, the same shape as the amethyst map above
// and as `buttonshapes.test.js`'s kept shapes: the only thing separating a
// deliberate use from drift is somebody having said which it is. Adding an entry
// is a decision about a congregation's tally light; adding one to make a build
// green is the drift this exists to catch.
//
// WHAT THIS DOES NOT SEE, said so it is not read as more — the same boundary the
// amethyst sweep states. A component wearing the shared `.r-cbtn.endsvc`,
// `.r-badge amber` or `.r-btn.amber` class WITHOUT naming the word (the dock's
// End service is painted by `app.css` on a `data-on` attribute) is outside this
// sweep by construction. That is the right boundary — those rules carry their
// meaning at their definition, which is where `app.css` argues for them — but it
// means this is the files that reach for amber THEMSELVES, not every file on
// which amber appears.
describe('the colour law — amber means ON AIR, and every use is named', () => {
  const ROOT = resolve(__dirname, '../..');
  const read = (f) => readFileSync(resolve(ROOT, f), 'utf8');
  // ONE STRIPPER, SHARED. Three regexes here was the shape that blanked 7 KB of
  // `Stage.svelte` in `names.test.js`: a comment containing `:8032/api/*` opened a
  // block comment that ran to the next real `*/`. `codeOnly` scans left to right,
  // knows a quoted run cannot open a comment, and BLANKS rather than deletes, so
  // every offset in the result still matches the file.
  const code = (f) => codeOnly(read(f));

  /** Every .svelte under src/, derived rather than typed. */
  const components = (() => {
    const out = [];
    const walk = (dir) => {
      for (const name of readdirSync(join(ROOT, dir))) {
        const rel = `${dir}/${name}`;
        if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
        else if (name.endsWith('.svelte')) out.push(rel);
      }
    };
    walk('src');
    return out;
  })();

  /** Does this file ASK app.css for the amber variant of a shared control? */
  const wearsAmberClass = (f) =>
    /class:amber\b/.test(code(f)) || /class="[^"]*\bamber\b[^"]*"/.test(code(f)) ||
    /class=\{[^}]*'amber'[^}]*\}/.test(code(f)) || /class="[^"]*\{[^}]*'amber'[^}]*\}[^"]*"/.test(code(f));
  /** Does this file paint the token itself? */
  const paintsAmberToken = (f) => /var\(--v-amber/.test(code(f));

  it('the two scanners can each see a real instance, and neither reads a comment', () => {
    // Guards the guards, for the fourth time in this repository and for the same
    // reason: `ipc.test.js` narrowed quietly twice while looking exhaustive, and a
    // sweep matching nothing makes every assertion below pass vacuously.
    expect(components.length, 'no components found at all').toBeGreaterThan(30);
    expect(wearsAmberClass('src/lib/views/library/LiveOutputRail.svelte')).toBe(true);
    expect(paintsAmberToken('src/lib/views/Live.svelte')).toBe(true);
    // `TemplateRender.svelte` mentions amber ONLY in prose explaining what it must
    // not do. A scanner counting that would fail on a correct file, and the
    // cheapest way to go green would be deleting the explanation.
    expect(read('src/lib/TemplateRender.svelte')).toMatch(/amber/i);
    expect(paintsAmberToken('src/lib/TemplateRender.svelte')).toBe(false);
    expect(wearsAmberClass('src/lib/TemplateRender.svelte')).toBe(false);
  });

  it('every surface wearing amber is one an operator reads as ON AIR', () => {
    // WHAT EACH ENTRY MUST BE: something a congregation is looking at right now,
    // the control that owns that state, or a template's own author-chosen colour.
    // "It needed to stand out" is not on the list — that is what `--v-sel` is for,
    // and DESIGN_SYSTEM §1.1 spends four paragraphs on why the accent stopped
    // being amber.
    const ALLOWED = new Map([
      // THE TALLY ITSELF — what is on a wall, and for how long.
      ['src/lib/views/Live.svelte', 'the on-air cell, the live slide ring and the air flag'],
      ['src/App.svelte', 'the shell lamp and the on-air stopwatch'],
      ['src/lib/views/library/LiveOutputRail.svelte', 'Go live — the button that puts it there'],
      ['src/lib/views/library/VerseDeck.svelte', 'the On Air badge and its tally'],
      // THE PREACHER'S OWN PAGE — the stage alert, which is on air to one person.
      ['src/Stage.svelte', 'the stage alert and the countdown warning on a live monitor'],
      // Channels and TemplateEditor were listed here until 2026-09-21, and both
      // entries were amnesties: the only amber either painted was a CAUTION (a
      // screen taken down, a stage role unset, a font that did not load). They
      // wear `--v-caution` now and the sweep at the bottom of this file holds them.
      // A LYRIC THAT IS UP. Same promise, said on the pane that put it there.
      ['src/lib/views/library/LyricsPane.svelte', 'the section currently on the screens'],
    ]);
    const offenders = components
      .filter((f) => (wearsAmberClass(f) || paintsAmberToken(f)) && !ALLOWED.has(f))
      .map((f) => `${f} — amber means ON AIR and nothing else (DESIGN_SYSTEM §1, rule 18)`);
    expect(
      offenders,
      'if this is genuinely the tally light, add it to ALLOWED with its reason; ' +
        'if it is emphasis, --v-sel is the accent; if it is caution, see §93',
    ).toEqual([]);

    // The list may only shrink. An entry for a file that has stopped wearing amber
    // is an amnesty waiting for the next component of that name.
    const stale = [...ALLOWED.keys()].filter((f) => !wearsAmberClass(f) && !paintsAmberToken(f));
    expect(stale, 'these no longer wear amber — take them out of the list').toEqual([]);
  });

  it('the model panel does not wear it — the violation this sweep was written for', () => {
    // Named rather than left to the sweep, so the regression is legible as itself
    // rather than as one line of a list. Watched to fail: restoring
    // `class:amber={m.recommended}` fails this and the sweep above.
    const ms = code('src/lib/ModelSetup.svelte');
    expect(ms, 'the recommended download is amber again').not.toMatch(/class:amber/);
    expect(ms, 'ModelSetup paints the tally colour').not.toMatch(/var\(--v-amber/);
    // …and it still marks the recommended one. The fix is a different colour, not
    // a lost distinction: `primary` is the house accent, which is what a
    // recommended action is.
    expect(ms).toMatch(/variant=\{m\.recommended \? 'primary' : ''\}/);
  });
});

// 2026-09-21 · measured in a browser (RG-192). A HEARD claim not on any screen
// wore `--v-amber` on its border and badge while the Program pane read CLEAR;
// and `uncertain_book` — the class that put Numbers 3:16 on a wall — was pixel
// identical to a paraphrase. Amber is the tally light; a claim is a claim.
describe('the claim card does not wear the tally light, and the three methods look different', () => {
  const live = readFileSync(resolve(process.cwd(), 'src/lib/views/Live.svelte'), 'utf8');
  const rule = (sel) => {
    const i = live.indexOf(sel);
    expect(i, `${sel} not found`).toBeGreaterThan(-1);
    return live.slice(i, live.indexOf('}', i));
  };
  it('a heard claim is steel until it is on the wall', () => {
    expect(rule('.clm{')).not.toMatch(/--v-amber/);
    expect(rule('.cbadge{')).not.toMatch(/--v-amber/);
    expect(rule('.clm{')).toMatch(/--v-sel/);
  });
  it('book-uncertain has its own mark', () => {
    expect(live).toMatch(/\.clm\.ub\{[^}]*dashed/);
    expect(live).toMatch(/class:ub=\{d\.method === 'uncertain_book'\}/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// CAUTION — THE GAP §93 NAMED, PAID (RG-207, DECISIONS §111, 2026-09-21).
//
// For four months this palette published no caution ink, so every warning that
// was not a failure borrowed a promise colour and argued for it at the call site:
// five surfaces borrowed amethyst ("nothing here reaches a congregation", which
// is true of a settings page and says nothing about the warning) and three more
// borrowed amber, the tally light, on a screen an operator had taken DOWN, on a
// stage screen with no role, and on a template whose font did not load. A
// volunteer reading amber on the Outputs tab was being told a screen was on air
// by the box that said it was not.
//
// `--v-caution` is a desaturated warm ochre: warm enough to read as a warning,
// far enough from `#ffa31a` that the two never sit side by side as the same
// colour. It carries NO promise about a screen — that is the whole point of it.
describe('the colour law — caution has its own ink, and it is neither promise', () => {
  const ROOT = resolve(__dirname, '../..');
  const read = (f) => readFileSync(resolve(ROOT, f), 'utf8');
  const code = (f) => codeOnly(read(f));
  /** The CSS block of one selector, comments blanked, or '' if it is gone. */
  const rule = (f, sel) => {
    const src = code(f);
    const esc = sel.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const m = src.match(new RegExp(`${esc}\\s*\\{([^}]*)\\}`));
    return m ? m[1] : '';
  };

  it('the token exists, and it is not amber and not amethyst', () => {
    const t = read('src/tokens.css');
    const hex = (name) => (t.match(new RegExp(`${name}:(#[0-9a-f]{6})`, 'i')) || [])[1];
    expect(hex('--v-caution'), '--v-caution is not defined in tokens.css').toBeTruthy();
    expect(hex('--v-caution')).not.toBe(hex('--v-amber'));
    expect(hex('--v-caution')).not.toBe(hex('--v-amethyst'));
    for (const v of ['--v-caution-soft', '--v-caution-line', '--v-caution2']) {
      expect(t, `${v} is missing — a component would hand-write an rgba()`).toContain(`${v}:`);
    }
  });

  // EVERY CAUTION, ENUMERATED WITH WHAT IT WARNS ABOUT. The eight surfaces the
  // 2026-09-21 audit and §93 between them named. Each must paint the caution ink
  // and neither promise colour.
  const CAUTIONS = [
    ['src/app.css', '.b-check.warn .ico', 'a boot probe that answered with a warning'],
    ['src/app.css', '.b-check.warn .note', 'the same probe, its note'],
    ['src/lib/views/Settings.svelte', '.s-netwarn', 'a service is being recorded / not while the mic is live'],
    ['src/lib/views/Help.svelte', '.callout', "the help page's callout"],
    ['src/lib/views/library/LyricsPane.svelte', '.ly-warn', 'the arrangement needs checking'],
    ['src/lib/ModelSetup.svelte', '.ms-caution', 'a model that will fall behind a live sermon'],
    ['src/lib/ModelSetup.svelte', '.ms-locked', 'the service lock is holding this back'],
    ['src/lib/views/Channels.svelte', '.ch-downnow', 'a screen the operator took down'],
    ['src/lib/views/Channels.svelte', '.ch-stage-warn', 'no screen has the stage role'],
    ['src/lib/views/templates/TemplateEditor.svelte', '.te-fwarn', "a template's font did not load"],
  ];

  it.each(CAUTIONS)('%s %s — %s — wears the caution ink and no promise colour', (f, sel) => {
    const r = rule(f, sel);
    expect(r, `${sel} is gone from ${f}`).not.toBe('');
    expect(r, `${sel} does not paint --v-caution`).toMatch(/var\(--v-caution/);
    expect(r, `${sel} still paints the tally light`).not.toMatch(/var\(--v-amber/);
    expect(r, `${sel} still paints the rehearsal colour`).not.toMatch(/var\(--v-amethyst/);
  });

  it('the SLIDE badge on the transport is steel, not the tally light — the wall may be clear', () => {
    // U15 / RG-207: `.rack-mode.slide` was amber "because the plan rail is amber",
    // on a caption that is painted whenever a plan is loaded, including over a
    // wall that says CLEAR. A mode badge is the thing you are working on.
    const r = rule('src/lib/views/Live.svelte', '.rack-mode.slide');
    expect(r).not.toBe('');
    expect(r).not.toMatch(/var\(--v-amber/);
    expect(r).toMatch(/var\(--v-sel\)/);
  });

  it('the scanner sees a rule when there is one, and an empty string when there is not', () => {
    expect(rule('src/app.css', '.b-check.fail .ico')).toMatch(/--v-red/);
    expect(rule('src/app.css', '.no-such-rule-anywhere')).toBe('');
  });
});
