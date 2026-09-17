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

/**
 * Names a promise token, INCLUDING its variants — `--v-amber-soft`, `--v-amber2`,
 * `--v-cyan-line`. A plain substring match is what is wanted: a variant of a
 * promise colour is still that promise, and no non-promise token in `app.css`
 * begins with one of these names.
 */
function promiseIn(value) {
  const v = String(value || '');
  for (const [token, means] of Object.entries(PROMISE)) {
    if (v.includes(token)) return `${token} (${means})`;
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
    const src = readFileSync(resolve(__dirname, './plan.js'), 'utf8');
    const body = src.slice(src.indexOf('export function slideAccent'));
    const fn = body.slice(0, body.indexOf('\n}') + 2);
    const offenders = Object.keys(PROMISE).filter((t) => fn.includes(`var(${t}`));
    expect(offenders).toEqual([]);
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
// AMETHYST'S BOUNDARY — DECISIONS §92.
//
// Everything above holds the law at the two `plan.js` tables that broke it. This
// block holds the one colour whose RULE was self-contradicting rather than whose
// call sites were: `src/app.css` said "Rehearse amethyst — the rehearsal colour,
// and nothing else uses it", in a file that spends amethyst on about fourteen
// other surfaces.
//
// §92 settles it by reading those fourteen instead of counting them. Amethyst's
// promise is "NOTHING HERE REACHES A CONGREGATION"; rehearsal is the case it was
// named for and not the whole of it, because safe mode ("outputs disabled") and
// the launch sequence (no console, no output window, nothing on any wall) are the
// same fact said where the word "rehearsal" does not fit.
//
// What §92 deliberately leaves OPEN is caution. Four surfaces spend amethyst on a
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
  const code = (f) =>
    read(f)
      .replace(/<!--[\s\S]*?-->/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/^\s*\/\/.*$/gm, '');

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

  it('and every surface that does paint it is one of the five kinds §92 allows', () => {
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
      // ── THE CAUTION GAP, §92's open question ────────────────────────────────
      // These three are NOT inside the promise. They are amethyst because this
      // palette publishes no caution ink and every other colour is already a
      // promise, and each records that argument independently at its own call
      // site. They are listed so that paying the gap off is visible and adding to
      // it is not silent. `.b-check.warn` in `src/app.css` is the fourth.
      ['src/lib/views/Settings.svelte', 'CAUTION GAP — .s-netwarn, §92'],
      ['src/lib/views/Help.svelte', 'CAUTION GAP — the callout, §92'],
      ['src/lib/views/library/LyricsPane.svelte', 'CAUTION GAP — .ly-warn, §92'],
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
      'amethyst means "nothing here reaches a congregation" — say which kind this is in DECISIONS §92, or use --v-sel',
    ).toEqual([]);

    // The list may only shrink. An entry for a file that has stopped painting
    // amethyst is an amnesty waiting for the next component of that name.
    const stale = [...ALLOWED.keys()].filter((f) => !paintsAmethyst(f));
    expect(stale, 'these no longer paint amethyst — take them out of the list').toEqual([]);
  });

  it('and the four cautions are still exactly four — the gap does not grow quietly', () => {
    // The count is the assertion. `.b-check.warn` in `src/app.css` is the fourth
    // and lives in the stylesheet rather than in a component, so it is named here
    // rather than in the map above.
    const cautions = [
      'src/lib/views/Settings.svelte',
      'src/lib/views/Help.svelte',
      'src/lib/views/library/LyricsPane.svelte',
    ];
    for (const f of cautions) expect(paintsAmethyst(f), `${f}`).toBe(true);
    expect(read('src/app.css')).toMatch(/\.b-check\.warn \.ico\{ color:var\(--v-amethyst2\); \}/);
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
