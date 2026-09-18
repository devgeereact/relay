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
      // ── THE CAUTION GAP, §93's open question ────────────────────────────────
      // These three are NOT inside the promise. They are amethyst because this
      // palette publishes no caution ink and every other colour is already a
      // promise, and each records that argument independently at its own call
      // site. They are listed so that paying the gap off is visible and adding to
      // it is not silent. `.b-check.warn` in `src/app.css` is the fourth.
      ['src/lib/views/Settings.svelte', 'CAUTION GAP — .s-netwarn, §93'],
      ['src/lib/views/Help.svelte', 'CAUTION GAP — the callout, §93'],
      ['src/lib/views/library/LyricsPane.svelte', 'CAUTION GAP — .ly-warn, §93'],
      // The fifth, and the ONE that arrived by a promise colour being given back
      // rather than by a new caution being invented. `.ms-caution` (the model that
      // will fall behind a live sermon on a machine with no acceleration) and
      // `.ms-locked` (the service lock) were and are cautions; the first was
      // painted `--v-amber`, which means ON AIR and only that, on a panel that is
      // never on air. Moving it into the gap is the gap being COUNTED correctly,
      // which is the opposite of it growing quietly — and it is what the law says
      // to do when a caution is the honest reading.
      ['src/lib/ModelSetup.svelte', 'CAUTION GAP — .ms-caution and .ms-locked, §93'],
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

  it('and the cautions are still exactly five — the gap does not grow quietly', () => {
    // The count is the assertion. `.b-check.warn` in `src/app.css` is one of them
    // and lives in the stylesheet rather than in a component, so it is named here
    // rather than in the map above.
    //
    // IT WAS FOUR AND IT IS FIVE, and the direction matters. `ModelSetup.svelte`
    // did not acquire a caution — it always had one, painted in the colour
    // reserved for ON AIR. Paying an amber violation into the amethyst gap makes
    // the gap one entry larger and the LAW one violation smaller, which is the
    // trade §93 describes. A sixth arriving because somebody wanted a warning
    // colour is the thing this count is watching for.
    const cautions = [
      'src/lib/views/Settings.svelte',
      'src/lib/views/Help.svelte',
      'src/lib/views/library/LyricsPane.svelte',
      'src/lib/ModelSetup.svelte',
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
      ['src/lib/views/Channels.svelte', 'a screen that is painting right now'],
      // THE PREACHER'S OWN PAGE — the stage alert, which is on air to one person.
      ['src/Stage.svelte', 'the stage alert and the countdown warning on a live monitor'],
      // TEMPLATE DATA, not chrome: a swatch showing a colour the operator chose.
      ['src/lib/views/templates/TemplateEditor.svelte', 'a template author picking a colour'],
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
