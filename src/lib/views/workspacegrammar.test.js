// THE WORKSPACE GRAMMAR — the instrument for docs/REBRAND.md §2 and §11.
//
// §2's note in the REBRAND status table says the quiet part out loud: twelve
// finished phases did not look like the prototype, because §2 — the workspace
// grammar — had no phase number and the implementation list never mentioned it.
// The shell got the chrome bar, the dock row and the status bar; the workspace
// BODIES stayed three unrelated designs.
//
// This file is what stops that happening again to the three desks. It does not
// test that a screen is pretty — nothing can. It tests the two things that made
// them diverge in the first place:
//
//   1. there is ONE definition of the layout and the type scale, and all three
//      workspaces go through it;
//   2. the decisions that were made once and then quietly re-litigated per file
//      — no pills, tokens not literals, steel blue for selection — hold.
//
// Written the way CLAUDE.md asks: each assertion fails if the defect it names is
// reintroduced. Checked by reverting each rule and watching it go red.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const read = (f) => readFileSync(resolve(__dirname, '../../..', f), 'utf8');

const FRAME = 'src/lib/views/WorkspaceFrame.svelte';
// The workspaces this pass covers. Live is deliberately absent: it is the run
// surface and has its own studio split (§2), which the frame does not own.
// Library is another agent's file and another pass.
//
// Templates and Themes joined on conversion, which is the point of keeping the
// list here rather than in three files: a desk is either in the grammar or it is
// not, and the array is where that is said. All four assertions below fired on
// the Templates/Themes conversion before it was finished — a raw 9px caption, a
// hand-rolled `.tg-pane` grid and a literal stage hex — which is what the list
// is for.
const DESKS = [
  'src/lib/views/ServicePlanner.svelte',
  'src/lib/views/Channels.svelte',
  'src/lib/views/Settings.svelte',
  'src/lib/views/templates/TemplateGallery.svelte',
  'src/lib/views/themes/ThemeGallery.svelte',
  'src/lib/views/themes/ThemeEditor.svelte',
];

describe('§2 · one workspace grammar, not three', () => {
  it('every desk lays itself out in the shared frame', () => {
    for (const f of DESKS) {
      const src = read(f);
      // `./` for a desk beside the frame, `../` for one in a subdirectory
      // (Templates and Themes each live in their own folder). The path is
      // allowed to vary; importing the ONE frame is not.
      //
      // `(?:\.\.?\/)+` rather than `\.\.?\/`: the first version of this was
      // hard-coded to `./` and failed the subdirectory desks for the wrong
      // reason — a test that reports "does not import the frame" about a file
      // that does is worse than no test, because the next person fixes the
      // import. One repeat of that is enough, so any relative depth passes and
      // only the frame itself is named.
      expect(src, `${f} does not import the frame`).toMatch(
        /import WorkspaceFrame from '(?:\.\.?\/)+WorkspaceFrame\.svelte'/,
      );
      expect(src, `${f} imports the frame but does not render it`).toMatch(/<WorkspaceFrame\b/);
    }
  });

  it('and none of them declares its own three-column body', () => {
    // The defect this replaces: the Planner's `.sp-shell`, Outputs' `.ch-shell`
    // and Settings' `.s-layout` were three separate grid declarations with three
    // different gutters (16px, 16px, 20px) for the same shape. A fourth copy is
    // how they drift apart again.
    for (const f of DESKS) {
      const grids = [...read(f).matchAll(/grid-template-columns:\s*([^;}]+)/g)].map((m) =>
        m[1].trim(),
      );
      // A table's column track is a different thing and is allowed; a RAIL ·
      // MAIN · INSPECTOR track is not, and it is recognisable: it ends in a
      // fixed inspector width after a `minmax(0,1fr)` main column.
      const bodies = grids.filter((g) => /minmax\(0,\s*1fr\)\s+\d+px\s*$/.test(g));
      expect(bodies, `${f} declares its own workspace body`).toEqual([]);
    }
  });

  it('the frame is the one place the columns are described', () => {
    const frame = read(FRAME);
    expect(frame).toMatch(/grid-template-columns:var\(--rw-cols\)/);
    // Rail · main · inspector, in that order — and every caller describes its body
    // by passing that track to the frame rather than by declaring a grid.
    //
    // SETTINGS IS TWO TRACKS, deliberately (docs/REBRAND.md §11): a rail plus one
    // reading column, capped and centred. It is the one workspace with no "thing
    // in hand" for an inspector to be about, so its third column had nothing to
    // put there and filled itself with a second copy of four rows that live in the
    // sections — including a second sentence about whether the update check has
    // ever succeeded, which is two surfaces for one rule-35 fact. The exception is
    // named here rather than loosening the count for everyone: a desk that DOES
    // have a selected object and quietly drops its inspector should still fail.
    const TWO_COLUMN = ['src/lib/views/Settings.svelte'];
    for (const f of DESKS) {
      const [, cols] = read(f).match(/columns="([^"]+)"/) ?? [];
      expect(cols, `${f} passes no column track to the frame`).toBeTruthy();
      const tracks = cols.trim().split(/\s+(?![^(]*\))/).length;
      expect(tracks, `${f}: ${cols}`).toBe(TWO_COLUMN.includes(f) ? 2 : 3);
    }
  });
});

describe('§11 · one type scale, three roles', () => {
  const frame = read(FRAME);

  it('the frame defines page title, standfirst and row', () => {
    // ROLE ONE IS AN EYEBROW NOW, not a display heading. This line used to
    // require `--v-fs-h1`, and that was the wrong half of the claim to hold: the
    // thing worth protecting is that the role is defined ONCE, here, in a token —
    // not that it is 24px. Six workspaces had drifted into two products because
    // Outputs dropped its title block while four others still opened with an H1
    // over a two-line standfirst, and on four of those the H1 was the workspace's
    // own name directly under the tab you had just pressed. It is now a mono
    // caption on the same band as the sentence and the controls, and still an
    // `<h1>` so heading navigation still lands on it.
    expect(frame, 'no page-title role').toMatch(/\.rw-h1\{[\s\S]*?font-size:var\(--v-fs-cap\)/);
    // …and it must still BE a heading. Styling it down is allowed; demoting it
    // to a <span> would take Settings' section name away from a screen reader.
    expect(frame, 'the page title is no longer a heading').toMatch(/<h1 class="rw-h1">/);
    expect(frame, 'no standfirst role').toMatch(/\.rw-lead\{[\s\S]*?font-size:var\(--v-fs-b2\)/);
    // One line on the band, with the whole sentence in its `title` — a head that
    // wraps to two lines is a head that is a different height per workspace,
    // which is the drift this whole file exists to catch.
    expect(frame, 'the standfirst may not wrap the band').toMatch(/\.rw-lead\{[\s\S]*?text-overflow:ellipsis/);
    // A row is a NAME and a VALUE — §11 is explicit, and the value half is what
    // `settingvalue.js` exists to fill honestly.
    expect(frame, 'no row name').toMatch(/\.rw-nvk\)?\{/);
    expect(frame, 'no row value').toMatch(/\.rw-nvv\)?\{/);
    // …and the footnote sits behind a hairline, so it reads as an aside rather
    // than as one more row of the list above it.
    expect(frame).toMatch(/\.rw-foot\)?\{[\s\S]*?border-top:1px solid var\(--v-line\)/);
  });

  it('the value half is mono, so a figure cannot reflow the name beside it', () => {
    // §1: "IBM Plex Mono (every figure, so a changing number never reflows the
    // row beside it)". A proportional value column shuffles the whole rail every
    // time an uptime or a client count ticks.
    const rule = frame.slice(frame.indexOf('.rw-nvv'));
    expect(rule.slice(0, 260)).toMatch(/font-family:var\(--f-mono\)/);
    expect(rule.slice(0, 260)).toMatch(/tabular-nums/);
  });

  it('no desk sets a font size in raw pixels', () => {
    // Settings alone carried eleven hand-picked sizes — 13.5px, 14px, 13px,
    // 12.5px, 12px, 11px, 10.5px, 10px, 9px, 18px, 30px — none from the scale.
    // Eleven steps chosen one control at a time is not a type scale.
    const offenders = [];
    // The WHOLE file, not just its <style> block: two of the sizes this found
    // were inline `style="font-size:12.5px"` on an empty-state, which is exactly
    // where a hand-picked size hides from a stylesheet-only scan.
    for (const f of [...DESKS, FRAME]) {
      for (const m of read(f).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)) {
        offenders.push(`${f}: ${m[0]}`);
      }
    }
    expect(offenders, 'use a --v-fs-* token — the scale is the point').toEqual([]);
  });
});

describe('§1 · the decisions that kept being re-litigated per file', () => {
  it('no pills — a pill in a control room reads as a toy', () => {
    const offenders = [];
    for (const f of [...DESKS, FRAME]) {
      for (const m of read(f).matchAll(/border-radius:\s*99px/g)) offenders.push(`${f}: ${m[0]}`);
    }
    // `--v-r-round` survives for the two shapes that are genuinely round — a
    // slider thumb and a switch — and a switch asks for the TOKEN, not the
    // literal, so this stays absolute.
    expect(offenders, 'radius is 3px; --v-r-round is for a thumb and a switch').toEqual([]);
  });

  it('selection is steel blue, and amber is still only ever ON AIR', () => {
    // A selected row is the thing you are working on. Amber means a congregation
    // is looking at something, and it is never allowed to mean anything else —
    // which is exactly the sort of rule a restyle erodes one file at a time.
    for (const f of DESKS) {
      const style = read(f).slice(read(f).lastIndexOf('<style>'));
      for (const m of style.matchAll(/\.[\w-]*\.(?:sel|on)\{([^}]*)\}/g)) {
        expect(m[1], `${f}: a selected row is painted amber`).not.toMatch(/--v-amber/);
      }
    }
  });

  it('a desk paints with tokens, never with a raw hex', () => {
    const offenders = [];
    for (const f of [...DESKS, FRAME]) {
      const style = read(f).slice(read(f).lastIndexOf('<style>'));
      for (const m of style.matchAll(/#[0-9a-fA-F]{3,8}\b/g)) offenders.push(`${f}: ${m[0]}`);
    }
    // `--v-rose,#e0526a` is a var() FALLBACK on the arrangement picker and #fff
    // is ink on a filled destructive button; both are pre-existing and named.
    expect(offenders.filter((o) => !/#e0526a|#fff\b/.test(o))).toEqual([]);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// THE TOKEN SWEEP — wave 4, agent K1. docs/REBRAND.md §1 and §12.
//
// Everything above holds SIX desks. That limit was deliberate and it was
// recorded as owed: the palette in `src/app.css` matched the prototype while
// the components ignored it, and the "Carried forward, deliberately" paragraph
// in docs/REBRAND.md said so rather than pretending otherwise.
//
// This block is the wider edge. It is in three tiers on purpose, because the
// three defects do not have the same blast radius and a single list would have
// to be the smallest of them:
//
//   1. PILLS and 2. HAND-TYPED SCALE STEPS are checked over EVERY component,
//      because after this sweep there are none left anywhere. A repo-wide
//      assertion that passes today is the cheapest one to keep true.
//   3. RAW HEX and LITERAL RADII are checked over a NAMED list, because
//      eighteen components still carry one or both for reasons that are judged
//      rather than mechanical — a wall that is really black, ink on a filled
//      destructive button, a mask channel, a radius the scale does not publish.
//      Those are listed in docs/REBRAND.md. Widening this list is the work;
//      quietly dropping a file out of it to make a build green is not.
//
// WHAT THIS STILL DOES NOT COVER, stated so nobody reads it as more than it is:
//   · off-scale font sizes are still NOT rejected, but the list is SHORTER than
//     it was, because wave 5 (M1) gave four of them a token: 15, 13, 9 and 8.5
//     are now --v-fs-{ttl,pr,fig,kind} and 53 literals converted, so tier 2
//     catches them from here on. What is left off-scale is 7, 8, 10, 10.5,
//     13.5, 16, 18, 22, 24 and 26px — about forty-eight in components and
//     thirteen in app.css. They are not in the reference type scale EITHER, so
//     converting one is still a restyle decision about that control rather
//     than a sweep, and rounding one into a neighbouring step to quieten a
//     scanner would be the worst of the three options.
//   · off-scale radii (2, 4, 7, 8, 9, 10, 11, 12, 13px) likewise. §1 asks for
//     2px and --v-r-sm is 3px; that contradiction is REBRAND's to settle.
//   · `rgba()` is not scanned at all. Seven Splash glows were the RETIRED
//     amethyst and no scanner here would have found them.
//   · template CONTENT is out of scope by design (themes.js, layers.js,
//     templates.js, TemplateRender.svelte): a colour an operator saved into a
//     slide is data, not chrome.
//   · `crash.js` is exempt from tier 2. Its whole premise is that the
//     stylesheet may not have loaded, so a token there would be a blank panel
//     at the worst possible moment.
const COMPONENTS = [
  'src/App.svelte', 'src/Output.svelte', 'src/Stage.svelte',
  'src/lib/DetectionInspector.svelte', 'src/lib/Dock.svelte', 'src/lib/FirstRun.svelte',
  'src/lib/LiveRail.svelte', 'src/lib/ModelSetup.svelte', 'src/lib/Splash.svelte',
  'src/lib/TemplatePreviewOverlay.svelte', 'src/lib/TemplateRender.svelte',
  'src/lib/boot/BootDiagnostics.svelte', 'src/lib/boot/BootSequence.svelte',
  'src/lib/boot/BootShell.svelte', 'src/lib/boot/CheckList.svelte',
  'src/lib/boot/CrashReportRecovery.svelte', 'src/lib/boot/DatabaseMigration.svelte',
  'src/lib/boot/HardwareCheck.svelte', 'src/lib/boot/PluginLoading.svelte',
  'src/lib/boot/RecoverSession.svelte', 'src/lib/boot/SafeModeStartup.svelte',
  'src/lib/boot/UpdateAvailable.svelte',
  'src/lib/ui/BrandMark.svelte', 'src/lib/ui/CameraPlate.svelte',
  'src/lib/ui/EmptyState.svelte', 'src/lib/ui/ErrorState.svelte', 'src/lib/ui/Loading.svelte',
  'src/lib/views/Channels.svelte', 'src/lib/views/Dashboard.svelte', 'src/lib/views/Help.svelte',
  'src/lib/views/Library.svelte', 'src/lib/views/Live.svelte',
  'src/lib/views/ServicePlanner.svelte', 'src/lib/views/Settings.svelte',
  'src/lib/views/Templates.svelte', 'src/lib/views/WorkspaceFrame.svelte',
  'src/lib/views/library/Announcements.svelte', 'src/lib/views/library/Arrangements.svelte',
  'src/lib/views/library/Browse.svelte', 'src/lib/views/library/Collections.svelte',
  'src/lib/views/library/History.svelte', 'src/lib/views/library/ImportReview.svelte',
  'src/lib/views/library/Inspector.svelte', 'src/lib/views/library/LiveOutputRail.svelte',
  'src/lib/views/library/LyricsPane.svelte', 'src/lib/views/library/MediaLibrary.svelte',
  'src/lib/views/library/Scripture.svelte', 'src/lib/views/library/VerseDeck.svelte',
  'src/lib/views/templates/DeskStrip.svelte', 'src/lib/views/templates/TemplateEditor.svelte',
  'src/lib/views/templates/TemplateGallery.svelte', 'src/lib/views/themes/ThemeEditor.svelte',
  'src/lib/views/themes/ThemeGallery.svelte',
];

// Swept clean of BOTH a raw hex and a literal radius, and held that way.
const SWEPT = [
  'src/App.svelte', 'src/lib/Dock.svelte', 'src/lib/LiveRail.svelte',
  'src/lib/TemplatePreviewOverlay.svelte',
  'src/lib/boot/BootDiagnostics.svelte', 'src/lib/boot/BootSequence.svelte',
  'src/lib/boot/BootShell.svelte', 'src/lib/boot/CheckList.svelte',
  'src/lib/boot/CrashReportRecovery.svelte', 'src/lib/boot/DatabaseMigration.svelte',
  'src/lib/boot/HardwareCheck.svelte', 'src/lib/boot/PluginLoading.svelte',
  'src/lib/boot/RecoverSession.svelte', 'src/lib/boot/SafeModeStartup.svelte',
  'src/lib/boot/UpdateAvailable.svelte',
  'src/lib/ui/BrandMark.svelte', 'src/lib/ui/CameraPlate.svelte',
  'src/lib/ui/EmptyState.svelte', 'src/lib/ui/Loading.svelte',
  'src/lib/views/Channels.svelte', 'src/lib/views/Dashboard.svelte',
  'src/lib/views/Settings.svelte', 'src/lib/views/Templates.svelte',
  'src/lib/views/WorkspaceFrame.svelte',
  'src/lib/views/library/Announcements.svelte', 'src/lib/views/library/Browse.svelte',
  'src/lib/views/library/Collections.svelte', 'src/lib/views/library/Inspector.svelte',
  'src/lib/views/library/LiveOutputRail.svelte', 'src/lib/views/library/LyricsPane.svelte',
  'src/lib/views/library/MediaLibrary.svelte', 'src/lib/views/library/Scripture.svelte',
  'src/lib/views/templates/DeskStrip.svelte', 'src/lib/views/themes/ThemeEditor.svelte',
];

// Swept clean of a raw hex, but still carrying a radius the scale does not
// publish. Kept as its own tier so the hex guarantee covers nine more files
// than the radius one does, rather than both collapsing to the smaller number.
const SWEPT_HEX_ONLY = [
  'src/lib/DetectionInspector.svelte', 'src/lib/ModelSetup.svelte',
  'src/lib/ui/ErrorState.svelte', 'src/lib/views/ServicePlanner.svelte',
  'src/lib/views/library/Arrangements.svelte', 'src/lib/views/library/History.svelte',
  'src/lib/views/library/ImportReview.svelte',
  'src/lib/views/templates/TemplateGallery.svelte', 'src/lib/views/themes/ThemeGallery.svelte',
];

// A comment is not a paint. Two files document a retired hex in prose
// (`--v-txt (#e8eaee)`, `This was #141417`) and a scanner that counts those
// either fails on a correct file or teaches the next person to delete the
// explanation — which is the more valuable half.
const styleOf = (src) => {
  const i = src.lastIndexOf('<style>');
  return (i === -1 ? '' : src.slice(i)).replace(/\/\*[\s\S]*?\*\//g, '');
};

describe('§1 · the token sweep — wave 4', () => {
  it('the scanner can still see the things it scans for', () => {
    // Both scanners here have a twin in this repository that quietly narrowed
    // and passed everything (ipc.test.js, twice). So: prove the comment
    // stripper does not strip code, and prove the style slicer finds a style.
    expect(styleOf('<style>a{color:#abc}/* #def */</style>')).toContain('#abc');
    expect(styleOf('<style>a{color:#abc}/* #def */</style>')).not.toContain('#def');
    expect(styleOf(read('src/lib/views/Live.svelte')).length).toBeGreaterThan(1000);
    expect(COMPONENTS.length).toBeGreaterThan(SWEPT.length);
  });

  it('no component anywhere is a pill', () => {
    // Was thirteen, in LiveRail, ModelSetup, DetectionInspector, BrandMark,
    // Arrangements, History, Help and Live. `--v-r-round` is still 99px and is
    // still correct for the shapes that are genuinely round — a slider thumb, a
    // status dot, a switch knob, a scrollbar thumb (app.css paints the shell's
    // that way) and the two-pixel bars of the brand mark. Those ask for the
    // TOKEN. The literal is what this rejects, so the round shapes survive and
    // the accidental pills cannot come back.
    const offenders = [];
    for (const f of COMPONENTS) {
      for (const m of styleOf(read(f)).matchAll(/border-radius:\s*9{2,3}px/g)) {
        offenders.push(`${f}: ${m[0]}`);
      }
    }
    expect(offenders, 'use var(--v-r-round) if the shape is really round').toEqual([]);
  });

  it('no component types a scale step as a number', () => {
    // The scale is read OUT of app.css rather than restated here, so this test
    // cannot drift from the thing it is about — and if a step's value changes,
    // every hand-typed copy of the OLD value stops being caught, which is the
    // correct behaviour: it is no longer a duplicate of anything.
    //
    // The defect: `font-size:12.5px` renders identically to `var(--v-fs-h3)`
    // until the day the scale moves, and then one heading is 1.5px out of step
    // with its twin and nobody can see why. Ninety-five of these were
    // converted; the wave-3 Settings pass had already found six.
    //
    // WAVE 5 (M1) WIDENED THIS TWICE OVER. The ladder grew four steps — 15, 13,
    // 9 and 8.5 had no token at all, which is the whole reason a hundred and
    // one literals survived wave 4's sweep: there was nothing to convert them
    // TO, and 53 of those converted the moment the tokens existed. And the scan
    // now includes `src/app.css` itself, which was the larger hole: the
    // stylesheet all 53 components share was the one file typing scale steps by
    // hand that no tier here could see, and it was doing it 43 times — a
    // scanner that holds every component to a rule the shared sheet is exempt
    // from is a scanner reporting on the smaller half of the problem.
    const css = read('src/app.css');
    const steps = new Map();
    for (const m of css.matchAll(/--v-fs-([a-z0-9]+)\s*:\s*([0-9.]+)px/g)) {
      if (!steps.has(m[2])) steps.set(m[2], m[1]);
    }
    expect(steps.size, 'no --v-fs-* scale found in app.css').toBeGreaterThan(5);
    // The four the reference names and Relay had no token for. Spelled out
    // rather than counted, so deleting one to make a build green has to be done
    // in the open.
    for (const [v, t] of [['15', 'ttl'], ['13', 'pr'], ['9', 'fig'], ['8.5', 'kind']]) {
      expect(steps.get(v), `the reference's ${v}px step has no token`).toBe(t);
    }

    const offenders = [];
    for (const f of [...COMPONENTS, 'src/app.css']) {
      // The WHOLE file: four of these were inline `style="font-size:12px"` on
      // a boot gate, which is exactly where a hand-typed size hides from a
      // stylesheet-only scan.
      for (const m of read(f).matchAll(/font-size:\s*([0-9.]+)px/g)) {
        if (steps.has(m[1])) offenders.push(`${f}: ${m[0]} is var(--v-fs-${steps.get(m[1])})`);
      }
    }
    expect(offenders).toEqual([]);
  });

  it('a swept component paints with tokens, never with a raw hex', () => {
    const offenders = [];
    for (const f of [...SWEPT, ...SWEPT_HEX_ONLY]) {
      for (const m of styleOf(read(f)).matchAll(/#[0-9a-fA-F]{3,8}\b/g)) {
        offenders.push(`${f}: ${m[0]}`);
      }
    }
    // No allowlist. That is the point of a named list rather than a glob: a
    // file is in it because it has none, and a file that needs an exception is
    // in the REBRAND paragraph instead, with the reason.
    expect(offenders).toEqual([]);
  });

  it('and a swept component asks the scale for its corners', () => {
    const offenders = [];
    for (const f of SWEPT) {
      for (const m of styleOf(read(f)).matchAll(/border-radius:\s*[0-9.]+px/g)) {
        offenders.push(`${f}: ${m[0]}`);
      }
    }
    expect(offenders, 'use --v-r-sm / -lg / -2xl / -round').toEqual([]);
  });

  it('--f-mono is declared once, and it is the figure face §1 asks for', () => {
    // It was declared TWICE in app.css, in two :root blocks. The legacy one said
    // 'JetBrains Mono' and was dead — same specificity, and the design-system
    // block is later in the file, so 'IBM Plex Mono' won. Verified by resolving
    // it in a real DOM before removing it, not by reading the cascade.
    //
    // Nothing was broken, which is why it mattered: a reader looking up the
    // console's figure face found the wrong answer first, and reordering the two
    // blocks would have silently changed every clock, confidence and latency in
    // the app.
    const css = read('src/app.css');
    const decls = [...css.matchAll(/--f-mono\s*:\s*([^;]+);/g)].map((m) => m[1].trim());
    expect(decls, 'two declarations is one too many').toHaveLength(1);
    expect(decls[0]).toMatch(/IBM Plex Mono/);
    // JetBrains Mono must stay in the bundle regardless: templates offer it BY
    // NAME, and that is a choice an operator saved into a slide.
    expect(read('src/lib/fonts.js')).toMatch(/jetbrains-mono/);
    expect(read('src/lib/views/templates/TemplateEditor.svelte')).toMatch(/'JetBrains Mono'/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// THE CONTROL METRICS — wave 5, agent M1. docs/REBRAND.md §1.
//
// The reference names one height per control, and the reason is a COLUMN
// rather than a control: a settings rail stacks a select, an input, a switch
// and a button in one list, and four heights picked one control at a time make
// that column step in and out by two pixels a row. It is the same class of
// defect as the range input's 2px UA margin — invisible to anyone reading the
// stylesheet, plainly wrong the moment somebody renders it.
//
// Two assertions, because there are two ways to lose it:
//   1. the shared rule drifts — checked against the reference table;
//   2. a component overrides the shared rule in its own file, which is how
//      `.r-select` came to render at 24px, 26px, 28px and 30px in four places
//      at once. Four of those were removed to write this.
//
// WHAT THIS DOES NOT COVER, and the limit is real: it reads the STYLESHEET, not
// a rendered box. A height declared at 26px and then collapsed by a flex parent,
// a padding that pushes past it, a font that does not load — none of those are
// visible here. Only a browser can answer what a control ACTUALLY measures, and
// the lead owns the headless one. A green run here means the numbers agree, not
// that the column lines up.
//
// Nor does it reject a literal height in general. A panel, a thumbnail, a bar
// and a waveform all have heights that belong to nothing but themselves, and a
// scanner that guessed would fail on legitimate code, get weakened, and take
// the real assertions with it — CLAUDE.md's own reason for leaving twenty-five
// of its forty-three rules untested.
describe('§1 · the control metrics', () => {
  const css = read('src/app.css');
  // The rule's body, by selector, with comments stripped so a retired value
  // documented in prose cannot satisfy or break an assertion.
  const ruleFor = (sel) => {
    const bare = css.replace(/\/\*[\s\S]*?\*\//g, '');
    const i = bare.indexOf(sel + '{');
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return bare.slice(i, bare.indexOf('}', i));
  };

  it('the scanner can still find a rule and still ignore a comment', () => {
    // Same guard as the token sweep above, for the same reason: both scanners
    // in this file have a twin in this repository that quietly narrowed and
    // passed everything.
    expect(ruleFor('.r-btn')).toMatch(/height:26px/);
    expect(ruleFor('.r-btn')).not.toMatch(/WHAT MOVED/);
  });

  it('every shared control is the height the reference gives it', () => {
    const TABLE = [
      ['.topbar-v', 34], // chrome bar
      ['.footer-v', 26], // status bar
      ['.r-btn', 26], // button
      ['.r-btn.sm', 22], // small button — the plan rail's foot is three of these
      ['.r-input, .r-select', 26],
      ['.ws-tab', 26], // workspace tab
      ['.r-pill', 26],
      ['.r-cbtn', 34], // control button
    ];
    for (const [sel, h] of TABLE) {
      expect(ruleFor(sel), `${sel} is not ${h}px`).toMatch(new RegExp(`height:${h}px`));
    }
    // The control button keeps a FLOOR as well as a height: the Controls dock
    // card is 178px and a panic control may never be scrolled out of reach
    // (CLAUDE.md rule 36), so `Dock.svelte` relaxes it to `height:auto` and the
    // floor is what survives. That is the reference's "min 32px when stretched",
    // and it is the one override the next assertion allows.
    expect(ruleFor('.r-cbtn')).toMatch(/min-height:32px/);

    // A switch and a colour well are one box, 38x21, so a mixed column lines up
    // on ONE right edge rather than stepping in and out by two pixels a row.
    for (const sel of ['.r-switch', 'input[type=color]']) {
      const r = ruleFor(sel);
      expect(r, `${sel} is not 38px wide`).toMatch(/width:38px/);
      expect(r, `${sel} is not 21px tall`).toMatch(/height:21px/);
    }

    // The slider is three numbers and they are coupled: an 18px BOX so the
    // pointer target is real, a 3px BAR so it reads as a track, and a 13px
    // thumb. Styling the input itself as the track left a 4px target, and a
    // near-miss on a live console lands on whatever is underneath.
    expect(ruleFor('.r-range, input[type=range]')).toMatch(/height:18px/);
    expect(css).toMatch(/slider-runnable-track[\s\S]{0,140}height:3px/);
    expect(css).toMatch(/slider-thumb\{[\s\S]{0,200}width:13px; height:13px/);
  });

  it('and no component overrides a shared control height in its own file', () => {
    // `.r-select` rendered at four heights in four files while app.css declared
    // a fifth. Nobody could see it, because each file was internally consistent
    // and the drift only exists in the column where two of them meet.
    const SHARED = ['r-btn', 'r-input', 'r-select', 'r-cbtn', 'r-switch', 'r-range', 'r-pill', 'ws-tab'];
    // Dock.svelte, named with its reason — see the floor above. An exemption
    // that has to be spelled out here is one somebody has to argue for.
    const ALLOWED = new Map([
      ['src/lib/Dock.svelte', /:global\(\.r-cbtn\) \{ height: auto; min-height: 32px; \}/],
    ]);
    const offenders = [];
    for (const f of COMPONENTS) {
      for (const m of styleOf(read(f)).matchAll(/[^{}]*\{[^}]*\}/g)) {
        if (!SHARED.some((c) => m[0].includes('.' + c))) continue;
        const body = m[0].slice(m[0].indexOf('{'));
        if (!/(^|[^-\w])height:\s*[0-9]/.test(body)) continue;
        const ok = ALLOWED.get(f);
        if (ok && ok.test(m[0].replace(/\s+/g, ' '))) continue;
        offenders.push(`${f}: ${m[0].replace(/\s+/g, ' ').trim()}`);
      }
    }
    expect(offenders, 'the shared control owns its height — override width and padding only').toEqual([]);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// ONE BUTTON, EVERYWHERE — wave 3, agent B1. docs/REBRAND.md §1.
//
// The metrics tier above holds the HEIGHTS. It was written against a real
// defect and it holds, and it is also why the remaining drift stayed invisible:
// every number in that table agreed while the app still rendered twenty-odd
// distinct button shapes per workspace, because a shape is not a height. It is
// height AND radius AND size AND weight AND fill AND edge AND colour AND
// FAMILY, and the last four were never checked anywhere.
//
// Four defects, each of which survived a green run of everything above:
//
//   1. `.r-btn` was the ONLY shared control with no background and a
//      transparent border. `.r-input`, `.r-select` and `.r-cbtn` all open
//      `--v-surf3` + `--v-500`. Thirteen buttons ship with no variant on them
//      and therefore no fill and no edge at all.
//   2. `ghost` drew its hairline from `--v-line2`, everything else from
//      `--v-500`. Those two buttons touch — Cancel beside Save — in every
//      dialog foot in the product.
//   3. `.r-btn.danger` drew `rgba(239,68,68,.5)`, which is in no token in this
//      repository. `.r-cbtn.danger` drew `var(--v-red-line)`, which is
//      `rgba(244,81,91,.5)`. One variant name, two reds, one stylesheet.
//   4. A `<button>` does not inherit its font. 128 of the 366 in the tree were
//      never given one, so the UA drew them `400 13.333px Arial` — a different
//      TYPEFACE at a size the scale does not contain, beside buttons that got
//      it right.
//
// Each assertion below was watched to fail with its own defect put back.
describe('§1 · one button, everywhere', () => {
  const css = read('src/app.css');
  const bare = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const ruleFor = (sel) => {
    const i = bare.indexOf(sel + '{');
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return bare.slice(i, bare.indexOf('}', i));
  };

  it('the scanner reads declarations and not the prose about them', () => {
    // Same guard the two scanners above carry, for the same reason: both of
    // this repository's other stylesheet scanners quietly narrowed and passed
    // everything. The comment on `.r-btn.danger` QUOTES the retired literal, so
    // a scanner that did not strip comments would report the defect present and
    // the defect fixed at the same time, whichever way round the code was.
    expect(ruleFor('.r-btn')).toMatch(/background:var\(--v-surf3\)/);
    expect(ruleFor('.r-btn.danger')).not.toMatch(/239,68,68/);
    expect(css).toMatch(/239,68,68/); // ...it is still there, in the comment.
  });

  it('the ordinary button draws the house surface, like every other shared control', () => {
    // The defect: `.r-btn` had no `background` and `border:1px solid transparent`.
    // Stated as an agreement rather than as three literals, because the point is
    // that these four controls are ONE column, not that surf3 is the colour.
    for (const sel of ['.r-btn', '.r-input, .r-select', '.r-cbtn']) {
      const r = ruleFor(sel);
      expect(r, `${sel} has no house fill`).toMatch(/background:var\(--v-surf3\)/);
      expect(r, `${sel} has no house edge`).toMatch(/border:1px solid var\(--v-500\)/);
    }
    // The transparent button still exists — it is now a NAMED variant rather
    // than what you get by forgetting to pick one.
    expect(ruleFor('.r-btn.quiet')).toMatch(/background:transparent/);
  });

  it('one edge — no button variant swaps the hairline for a different one', () => {
    // The defect: `.r-btn.ghost{ border-color:var(--v-line2) }`.
    // `--v-line2` is a DIVIDER token (rgba(255,255,255,.13)); it is right on a
    // rule between two rows and wrong on a control that sits beside a control.
    for (const sel of ['.r-btn.ghost', '.r-btn.primary', '.r-btn.amber', '.r-btn.danger', '.r-btn.quiet']) {
      expect(ruleFor(sel), `${sel} draws a second hairline`).not.toMatch(/border-color:var\(--v-line2\)/);
    }
  });

  it('danger is ONE red, and it comes from the token', () => {
    // The defect: two reds under one variant name. Asserted as agreement
    // between the two controls, which is the property that was actually broken
    // — a literal-hunting scanner would also condemn `.r-cbtn.black`, whose
    // absolute black IS the claim it makes, and would then be weakened.
    const btn = ruleFor('.r-btn.danger');
    const cbtn = ruleFor('.r-cbtn.danger');
    expect(btn).toMatch(/border-color:var\(--v-red-line\)/);
    expect(cbtn).toMatch(/border-color:var\(--v-red-line\)/);
    for (const r of [btn, cbtn]) expect(r, 'an untokenised red').not.toMatch(/rgba\(\s*\d/);
  });

  it('a press is pointer-down, and reduced motion still gets a press', () => {
    // The defect: `.r-btn:active{transform:scale(.98)}` unconditional, with no
    // `reduce` branch — so an operator who asked the OS for no animation got
    // movement from every button in the product, and `.r-cbtn` still would have
    // without the explicit `transform:none` below.
    const motion = bare.slice(bare.lastIndexOf('@media (prefers-reduced-motion: no-preference){'));
    expect(motion).toMatch(/\.r-btn:active:not\(:disabled\)\{ transform:scale\(\.98\); \}/);
    const reduce = bare.slice(bare.lastIndexOf('@media (prefers-reduced-motion: reduce){'));
    // Switched OFF, not merely un-restated: a media query cannot unset a
    // declaration made outside it, and `.r-cbtn`'s scale is made outside it.
    expect(reduce).toMatch(/\.r-btn:active/);
    expect(reduce).toMatch(/\.r-cbtn:active/);
    expect(reduce).toMatch(/transform:none/);
    // ...and it is a cut, not an absence. A press an operator cannot perceive
    // is a button they cannot tell from a dead one.
    expect(reduce).toMatch(/filter:brightness/);
  });

  it('every button is given a font, by a floor that cannot outrank a choice', () => {
    // The defect: nothing declared a font for a bare `<button>`, and a button
    // does not inherit one. This is the rail row that measured 13.33px.
    const floor = ruleFor('button, [role="button"]');
    expect(floor).toMatch(/font-family:var\(--f-body\)/);
    expect(floor).toMatch(/font-size:var\(--v-fs-b2\)/);
    // It must stay an ELEMENT selector. Promote it to a class and it starts
    // overriding the components that DID declare a font — `Stage.svelte`'s 18px
    // phone buttons are sized for a preacher holding the device at arm's length,
    // and a floor that could flatten those is a worse bug than the one it fixes.
    expect(bare).not.toMatch(/\.r-btn, ?\[role="button"\]\{/);
    // `font:inherit` is the tidier-looking line and the wrong one: it takes the
    // SIZE of whatever pane the button sits in, so one control renders at three
    // sizes in three places. The floor names its step.
    expect(floor).not.toMatch(/font:\s*inherit/);
  });

  it('the transport uses the shared button rather than redrawing it', () => {
    // Live's `.rk` was 28px with a `--v-line2` edge and `--v-fs-cap` type — the
    // pair an operator touches most on the run surface, three steps off the
    // shared control at once. What is left of the rule is width and flex, which
    // is the shape a legitimate override has.
    const live = read('src/lib/views/Live.svelte');
    const css = styleOf(live);
    expect(live).toMatch(/class="r-btn rk wide" title="Previous/);
    expect(live).toMatch(/class="r-btn rk wide" title="Next/);
    expect(css.match(/\.rk\{[^}]*\}/), '.rk draws its own box again').toBeNull();
    // AND the rule that was ACTUALLY drawing them. `.wide` declared a complete
    // second button — 32px, surf2, a line2 hairline — under a name that reads
    // as a layout utility, at equal specificity to `.rk` and two hundred lines
    // below it, so it won every property the two shared. The file said 28px,
    // the browser drew 32, and both rules were correct on their own terms.
    // A test that only watched `.rk` would have called this fixed while the
    // transport still measured 32.
    // ANCHORED. Written as /\.wide\{/ first, which matched the `.wide{` INSIDE
    // `.rk.wide{width:100%; flex:0 0 auto}` two hundred lines above — a rule
    // that legitimately carries no box — so the assertion read a clean body and
    // passed with the skin restored. Watched to fail only after this anchor.
    const wide = css.match(/(?:^|[\s,};])\.wide\{([^}]*)\}/);
    for (const p of ['height', 'background', 'border', 'font-size', 'font-family']) {
      expect(wide?.[1] ?? '', `.wide is a button skin again (${p})`)
        .not.toMatch(new RegExp(`(^|[;\\s])${p}\\s*:`));
    }
  });
});
