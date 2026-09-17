// W6 — SETTINGS: the eleven sections, and the three things §11 asks of them.
//
// docs/REBRAND.md §11 is four sentences long and every one of them is a claim
// that can be checked against the file rather than admired in a spec:
//
//   · eleven sections, merged from sixteen (this repository had eighteen);
//   · one type scale, three roles — page title / standfirst / row;
//   · a footnote behind a hairline;
//   · a list row is a name and a VALUE, never an em dash standing in for one.
//
// And the acceptance clause adds the three that matter more than the layout:
// no setting writes a preference nothing reads (DECISIONS §69), the update line
// still cannot say "up to date" when no check has run (rule 35), and no control
// appears twice.
//
// Why a source scan rather than a mounted component: Settings imports the whole
// store, the boot ladder, ModelSetup, History and Dashboard, and the questions
// below are about the SHAPE of the file — which sections exist, which classes
// exist, which handler is bound twice — none of which a render answers better.
// The one question a scan genuinely cannot answer is the rendered height of a
// section, and that is marked NOT TESTED in the review note rather than faked
// here with a proxy that would pass whatever the page looked like.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
// A real import, not `require`. This package is `"type": "module"` and CI runs
// this suite on Node 20, 22 and 24 precisely because runtime differences have
// bitten here before.
import { CONTENT_KINDS } from './layers.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

const SRC = read('src/lib/views/Settings.svelte');
const APPCSS = read('src/app.css');
const SCRIPT = SRC.slice(0, SRC.indexOf('</script>'));
const MARKUP = SRC.slice(SRC.indexOf('</script>'), SRC.indexOf('<style>'));
const STYLE = SRC.slice(SRC.indexOf('<style>'));

// COMMENTS ARE NOT THE SURFACE. This file explains, at length, the controls it
// no longer has — "up to date", `resetAllSettings`, Theme — because the reason a
// control was removed is worth more than the removal. A scan that reads those
// sentences reports the removed control as still present, which is a false
// finding about a fix, and the fix is to scan what renders rather than to stop
// writing the explanation down.
const strip = (s) =>
  s
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '');
const CODE = strip(SRC);
const MARKUP_ONLY = strip(MARKUP);
/** The `<script>` block with its commentary removed — see the note above. */
const SCRIPT_ONLY = strip(SCRIPT);

/** Every `{ key: 'x', label: 'Y' }` in the SECTIONS array, in order. */
const SECTIONS_SRC = SCRIPT.slice(
  SCRIPT.indexOf('const SECTIONS = ['),
  SCRIPT.indexOf('let section ='),
);
const sections = [...SECTIONS_SRC.matchAll(/\{ key: '([a-z]+)',\s+label: '([^']+)'/g)].map((m) => ({
  key: m[1],
  label: m[2],
}));
/** Every branch of the one `{#if section === …}` chain. */
const branches = [...MARKUP_ONLY.matchAll(/section === '([a-z]+)'/g)].map((m) => m[1]);

describe('§11 · eleven sections, merged from eighteen', () => {
  it('there are exactly eleven, and they are the eleven the spec names', () => {
    expect(sections.map((s) => s.label)).toEqual([
      'General',
      'Screens & looks',
      'Audio',
      'AI & Detection',
      'Scripture & Languages',
      'Network & Integrations',
      'History & Backup',
      'Shortcuts',
      'Updates',
      'Diagnostics',
      'Privacy & Advanced',
    ]);
  });

  it('every section in the rail has a panel, and every panel has a rail entry', () => {
    // The failure this catches is a merge that renames a key on one side only:
    // the rail then offers a button that renders an empty panel, or a branch
    // exists that nothing can ever select. Both look fine in a diff.
    const keys = sections.map((s) => s.key);
    expect([...new Set(branches)].sort()).toEqual([...keys].sort());
  });

  it('the standfirst is the section’s own sentence, not the page title again', () => {
    // Role two of three. The frame renders `standfirst={activeSection.desc}`, so
    // every section must HAVE one and none may repeat its own label.
    const descs = [
      ...SECTIONS_SRC.matchAll(/\{ key: '([a-z]+)',\s+label: '([^']+)',\s+desc: '([^']+)'/g),
    ];
    expect(descs.length).toBe(11);
    for (const [, key, label, desc] of descs) {
      expect(desc.length, `${key} has no standfirst`).toBeGreaterThan(20);
      expect(desc, `${key} repeats its own label as its standfirst`).not.toBe(label);
    }
  });

  it('the three roles come from the frame — this file does not redefine them', () => {
    // A type scale defined twice is two type scales. `WorkspaceFrame.svelte` owns
    // `.rw-nv` / `.rw-nvk` / `.rw-nvv` / `.rw-nvnote` / `.rw-group` / `.rw-foot`;
    // Settings used to carry private copies of all six, plus a SECOND row grammar
    // in `.s-netrow`/`.s-netk`/`.s-netv` with different padding and a different
    // key colour, and a second footnote (`.s-tr-note`) with no hairline at all.
    for (const gone of [
      '.s-lead{',
      '.s-row{',
      '.s-rowtitle{',
      '.s-rownote{',
      '.s-netrow{',
      '.s-netk{',
      '.s-netv{',
      '.s-grouphead{',
      '.s-note{',
      '.s-tr-note{',
    ])
      expect(STYLE, `${gone} is a private copy of a role the frame already owns`).not.toContain(
        gone,
      );
    expect(MARKUP_ONLY).toMatch(/class="rw-nv"/);
    expect(MARKUP_ONLY).toMatch(/class="rw-group"/);
    expect(MARKUP_ONLY).toMatch(/class="rw-foot"/);
  });

  it('a row is a name and a value — no em dash stands in for one', () => {
    // R3-13 in `surface.test.js` forbids the `|| '—'` fallback; this is the same
    // rule stated positively for the merged page, and it also catches a bare em
    // dash typed straight into a value.
    expect(MARKUP_ONLY).not.toMatch(/\|\|\s*'—'/);
    expect(MARKUP_ONLY).not.toMatch(/class="rw-nvv">—</);
  });
});

describe('acceptance 1 · no setting writes a preference nothing reads', () => {
  it('Theme is gone, and with it the only writer of an attribute nobody read', () => {
    // It wrote `prefs.theme` and stamped `data-theme` on the document element.
    // Nothing in the application has ever read either — DECISIONS §69's defect,
    // and docs/REBRAND.md §1 is explicit that Relay is dark only, so it was not
    // "Soon" either.
    expect(CODE).not.toMatch(/dataset\.theme/);
    expect(CODE).not.toMatch(/applyTheme/);
    expect(MARKUP_ONLY).not.toMatch(/setPref\('theme'/);
  });

  it('nothing anywhere reads `data-theme`, which is why the control had to go', () => {
    // The guard on the finding rather than on the fix: if a light sheet ever
    // lands, this test fails and the control can come back with a reason.
    const files = [];
    const walk = (dir) => {
      for (const e of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
        const p = `${dir}/${e.name}`;
        if (e.isDirectory()) walk(p);
        else if (/\.(svelte|css|js)$/.test(e.name) && !e.name.endsWith('.test.js')) files.push(p);
      }
    };
    walk('src');
    const readers = files.filter((f) => /\[data-theme/.test(read(f)));
    expect(readers).toEqual([]);
  });

  it('there is no preference store left at all — the last two were removed', () => {
    // `Auto Start on Login` and `Minimize to System Tray` were the survivors, kept
    // as `disabled` switches with a "Soon" chip on the argument that saying what is
    // missing beats pretending to work. It is not a middle ground: they saved a
    // preference nothing reads (§69's defect exactly) AND they rendered as a
    // greyed copy of the live switch above them, so a column of switches on this
    // page had two rows of furniture at the bottom of it. "Soon" is a promise too,
    // and neither is on a roadmap.
    //
    // Removing them emptied `DEFAULT_PREFS`, which left the key, the loader, the
    // saver and the setter with no reader — so the whole store went. This asserts
    // the store is GONE rather than merely small: an empty object waiting for the
    // next preference is how the next one arrives without a reader.
    expect(SCRIPT_ONLY).not.toMatch(/DEFAULT_PREFS/);
    expect(SCRIPT_ONLY).not.toMatch(/const PREFS_KEY/);
    expect(CODE).not.toMatch(/\bsetPref\(/);
    expect(CODE).not.toMatch(/\bsavePrefs\b/);
    expect(CODE).not.toMatch(/\bloadPrefs\b/);
  });

  it('and nothing in `src/` opens that key any more', () => {
    const files = [];
    const walk = (dir) => {
      for (const e of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
        const p = `${dir}/${e.name}`;
        if (e.isDirectory()) walk(p);
        else if (/\.(svelte|js)$/.test(e.name) && !e.name.endsWith('.test.js')) files.push(p);
      }
    };
    walk('src');
    // Stripped, per this file's own rule: Settings still NAMES the key, in the
    // tombstone comment that says why it went. A scan that reads the explanation
    // and reports the store as still present is a false finding about a fix.
    expect(files.filter((f) => strip(read(f)).includes('relay.prefs.v1'))).toEqual([]);
  });

  it('no control on this page is shown as a switch it cannot move', () => {
    // The rendered shape of the same defect. A switch that can never be thrown
    // and a "Soon" chip are what the two removed rows looked like; if either
    // idiom comes back, some control is being drawn as live-looking furniture
    // again.
    //
    // ── WHY THIS ASKS WHAT DISABLES THE SWITCH, NOT WHETHER ──────────────────
    // It used to read `not.toMatch(/role="switch"[\s\S]{0,200}?disabled/)`:
    // any `disabled` within 200 characters of a switch failed it. That caught
    // the two dead rows, which carried a BARE `disabled` — an attribute with no
    // expression, true for the life of the build, on a control with no handler
    // behind it. It also catches `disabled={!$capture.available}`, which is a
    // different thing entirely: a live fact about this machine right now, that
    // an operator can change by attaching the engine, on a control that does
    // move the moment they do.
    //
    // Forbidding both would have kept §12 out of the two sections that need it
    // most — a switch is the instrument the spec asks for, and crash reporting
    // and latency measuring genuinely cannot be set while the backend is away.
    // So the claim is sharpened rather than dropped, and it is STRICTER than
    // the old one in the case it was written for: a switch may carry no literal
    // `disabled` at all, and any conditional one must name a live store, which
    // the two dead rows could never have satisfied.
    // `[^<]`, not `[^>]`: an arrow function in `on:click` carries a `>`, so a
    // scanner bounded by the first `>` stops in the middle of the handler and
    // reads a truncated tag. Attributes contain no `<`, so the close tag is the
    // honest boundary.
    const switches = [...MARKUP_ONLY.matchAll(/<button\b[^<]*?role="switch"[^<]*?><\/button>/g)].map(
      (m) => m[0],
    );
    // The guard on the instrument: a scanner that finds nothing passes anything.
    expect(switches.length, 'Settings renders at least one switch').toBeGreaterThan(0);
    for (const s of switches) {
      // A bare `disabled`, or one set from a literal, is furniture.
      expect(s, `a switch with a literal disable: ${s}`).not.toMatch(
        /disabled(?![-\w=])|disabled=\{(true|false)\}|disabled="/,
      );
      const cond = s.match(/disabled=\{([^}]*)\}/);
      if (cond) {
        expect(cond[1], `a switch disabled by something that is not live: ${s}`).toMatch(/\$/);
      }
      // And it must actually do something — the other half of "cannot move".
      expect(s, `a switch with no handler: ${s}`).toMatch(/on:click=/);
    }
    expect(MARKUP_ONLY).not.toMatch(/>Soon</);
    expect(MARKUP_ONLY).not.toMatch(/Not available yet/);
  });

  it('Settings defines no switch of its own — §12 is one instrument everywhere', () => {
    // `.s-toggle`/`.s-knob` were a private 38×21 switch, pixel for pixel the same
    // shape as `.r-switch` in `app.css`. A second instrument defined in a view file
    // is how "one instrument everywhere" stops being true quietly, and it is only
    // ever noticed when the two drift. `app.css` keeps the one.
    expect(STYLE).not.toMatch(/\.s-toggle\{/);
    expect(STYLE).not.toMatch(/\.s-knob\{/);
    expect(APPCSS).toMatch(/\.r-switch\{[^}]*width:38px;\s*height:21px/);
  });

  // ── E1 · §12 · ONE INSTRUMENT, AND IT IS ACTUALLY USED ────────────────────
  //
  // W6 removed Settings' PRIVATE switch and the test above holds that absence.
  // Absence is only half of §12: the page then had no switch at all, and its
  // three real binary settings — safe mode, latency measuring, crash reporting
  // — were three different text buttons instead. `Turn on`, `Stop measuring`,
  // `Turn crash reporting on`: three grammars for one question, two of which
  // showed the ACTION rather than the STATE, so the only way to read whether
  // Relay was reporting crashes was to read the label and invert it.
  //
  // §12 asks for one instrument everywhere so a mixed column lines up on one
  // right edge. These are the three that make that true.
  describe('§12 · every binary setting wears the one switch', () => {
    /** The three real on/off settings on this page, and where each one lives. */
    const BINARY = [
      // `applySafeMode`, not `setSafeMode`: the switch goes through the one door
      // that actually disarms detection and closes the screens, and the record
      // writer has exactly one caller (DECISIONS §86, `safemode.test.js`).
      { label: 'Safe mode', section: 'general', handler: 'applySafeMode' },
      { label: 'Measuring latency', section: 'diagnostics', handler: 'toggleLatency' },
      { label: 'Send crash reports', section: 'privacy', handler: 'toggleCrash' },
    ];

    for (const b of BINARY) {
      it(`${b.label} is a switch, not a sentence on a button`, () => {
        // NOT `[^>]*` for the attributes: an arrow function contains a `>`, so
        // a tag scanner written that way stops at `() =` and silently reports
        // that a switch has no handler. It ends at `></button>`, which is what
        // actually closes one of these.
        const sw = MARKUP_ONLY.match(
          new RegExp(`<button\\b[^<]*?aria-label="${b.label}"[^<]*?></button>`),
        )?.[0];
        expect(sw, `no switch labelled “${b.label}”`).toBeTruthy();
        // The shared instrument from `app.css`, never a local one.
        expect(sw).toMatch(/class="r-switch"/);
        // Announced as a switch, and its state readable without sight.
        expect(sw).toMatch(/role="switch"/);
        expect(sw).toMatch(/aria-checked=\{/);
        expect(sw).toMatch(new RegExp(`on:click=[^<]*${b.handler}`));
      });
    }

    it('and the text buttons those three replaced are gone', () => {
      // The labels themselves, so a reintroduction is caught by the words an
      // operator would read rather than by a class name.
      expect(MARKUP_ONLY).not.toMatch(/>\s*\{\$safeMode \? 'Turn off' : 'Turn on'\}/);
      expect(MARKUP_ONLY).not.toMatch(/Stop measuring/);
      expect(MARKUP_ONLY).not.toMatch(/Turn crash reporting o/);
    });

    it('no switch on this page is thrown from a local mirror of a remote fact', () => {
      // Rule 15/35 on a control rather than on a status line: a switch that
      // flips on click and only afterwards asks the backend has reported a
      // success it has not achieved. Each of the three reads the value the
      // engine returned — a derived store, or a field assigned from the reply.
      expect(SCRIPT_ONLY).toMatch(/\$: crashOn = !!crash\.enabled;/);
      expect(SCRIPT_ONLY).toMatch(/const now = await latencySetEnabled\(on\);/);
      // `safeMode` is the derived store, not a `let` in this file.
      expect(SCRIPT_ONLY).not.toMatch(/let safeMode\b/);
    });

    it('measuring says “not read yet” rather than “on” over a backend it never asked', () => {
      // The control this replaced read `lat?.enabled ?? true`, which printed the
      // word a healthy measuring pipeline shows whenever the report was null —
      // one reading over two situations, which is rule 35 exactly. The tri-state
      // keeps them apart, and the switch is not rendered at all until there is a
      // fact to throw it from.
      expect(SCRIPT_ONLY).toMatch(/\$: latMeasuring = lat \? !!lat\.enabled : null;/);
      expect(MARKUP_ONLY).not.toMatch(/lat\?\.enabled \?\? true/);
      const row = MARKUP_ONLY.slice(
        MARKUP_ONLY.indexOf('<div class="rw-nvk">Measuring</div>'),
      ).slice(0, 900);
      expect(row).toMatch(/latMeasuring === null/);
      expect(row).toMatch(/missing: 'not read yet'/);
      expect(row).toMatch(/\{#if latMeasuring !== null\}/);
    });

    it('an ACTION is still a button — the two grammars are not merged the other way', () => {
      // The opposite failure: everything becoming a switch. Resetting the
      // measurement performs something once and has no state to show, so it
      // stays a BUTTON, beside the switch rather than instead of it.
      //
      // WIDENED, and it is worth saying why rather than just doing it. The
      // assertion used to read the literal string `<button class="r-btn"`, and
      // that is not the claim in its own title: the claim is that this control is
      // an action and not a state. When the control moved to `ui/Button.svelte` --
      // which renders exactly `.r-btn` and adds the reason a disabled control owes
      // the operator -- this went red over a change that made it strictly better,
      // which is a scanner measuring the spelling instead of the thing. Both
      // spellings are accepted and the NEGATIVE half is stated explicitly, because
      // that is the half the title is about: it may not become a switch.
      const control = MARKUP_ONLY.slice(
        Math.max(0, MARKUP_ONLY.indexOf('Start a fresh measurement') - 400),
        MARKUP_ONLY.indexOf('Start a fresh measurement'),
      );
      expect(control).toMatch(/<(?:button class="r-btn"|Button)\b/);
      expect(control).toMatch(/on:click=\{resetLatency\}/);
      expect(control, 'the action became a state').not.toMatch(/r-switch|role="switch"/);
    });
  });

  it('the footnote counts the removals, and the count is the tally §69 holds', () => {
    // The page states the absence rather than listing the names. The number is the
    // only part of that sentence that can go stale, so it is the part under test:
    // nine before this pass, eleven after it.
    const foot = MARKUP_ONLY.slice(MARKUP_ONLY.indexOf('preference controls used to be'));
    expect(MARKUP_ONLY).toMatch(/Eleven preference controls used to be on this page/);
    expect(MARKUP_ONLY).not.toMatch(/Nine preference controls/);
    expect(foot).toMatch(/Auto Start on Login/);
    expect(foot).toMatch(/Minimize to System Tray/);
  });

  it('what Relay does at launch is STATED, not offered as a toggle nothing reads', () => {
    // The prototype's General section offers "Open outputs on launch" as a switch.
    // `App.svelte` already calls `autoOpenOutputs()` on mount unless safe mode is
    // on, and there is no persisted preference and no reader for one — so building
    // the switch would be §69 again. The behaviour is a row with a real value
    // instead, read from the one thing that changes it.
    const general = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf("section === 'general'"),
      MARKUP_ONLY.indexOf("section === 'screens'"),
    );
    expect(general).toMatch(/Screens at launch/);
    expect(general).toMatch(/\$safeMode \? 'held back by safe mode' : 'reopened automatically'/);
  });

  it('“Confirm before going live” does not come back because a prototype shows one', () => {
    // It was deleted in §69 for promising a confirmation step between the operator
    // and the congregation's screen that has never existed. A prototype cannot
    // promise a guard the engine does not have.
    expect(MARKUP_ONLY).not.toMatch(/Confirm [Bb]efore [Gg]oing [Ll]ive/);
  });

});

describe('acceptance 2 · the update line still reports the CHANNEL', () => {
  it('neither surface invents its own words for a check outcome', () => {
    // `describeChannel` is the one place a channel state becomes a sentence
    // (rule 35, RG-83, RG-92). It used to be called TWICE — the Updates row and
    // the Overview rail's "Check for updates" card, which is a second surface that
    // can drift from the first about whether a check ever succeeded. The rail is
    // gone, so there is now exactly ONE place on this page that turns a check
    // outcome into words, and this asserts the count rather than its presence:
    // a third caller is the failure, and so is a zero.
    const calls = [...MARKUP_ONLY.matchAll(/describeChannel\(\$updateChannel\)/g)];
    expect(calls.length).toBe(1);
    expect(MARKUP_ONLY).not.toMatch(/up to date/);
    expect(MARKUP_ONLY).not.toMatch(/latest version/);
  });

  it('a failed check is coloured as a failure, not as news', () => {
    expect(MARKUP_ONLY).toMatch(/class:s-netbad=\{\$updateChannel\.state === 'failed'\}/);
    expect(STYLE).toMatch(/\.s-netbad\{[^}]*--v-rose/);
  });

  // The mirror of the assertion above: `.s-netbad` → rose is pinned, but
  // nothing pinned `.s-netwarn` → amethyst, so recolouring it to amber (rule
  // 18's colour reserved for ON AIR, on a page that is never on air) would
  // leave every other test green.
  it('the caution class is amethyst, not the colour reserved for ON AIR', () => {
    expect(STYLE).toMatch(/\.s-netwarn\{[^}]*--v-amethyst/);
  });
});

describe('acceptance 3 · no duplicated control, and no class that styles nothing', () => {
  it('no named handler is bound to two different controls', () => {
    // `resetAllSettings` was bound twice — a rail-footer button and a Danger Zone
    // card, six inches apart, two labels, one destructive action. (It is gone
    // entirely now: with Theme deleted there was nothing left for it to restore.)
    const bound = [...MARKUP_ONLY.matchAll(/on:click=\{([A-Za-z_$][\w$]*)\}/g)].map((m) => m[1]);
    const twice = bound.filter((h, i) => bound.indexOf(h) !== i);
    expect([...new Set(twice)]).toEqual([]);
  });

  it('the reset that restored nothing is gone from both places it was offered', () => {
    expect(CODE).not.toMatch(/resetAllSettings/);
    expect(CODE).not.toMatch(/Reset to Defaults|Reset All Settings/);
  });

  it('every `class:` directive names a class something actually defines', () => {
    // The bug: the update-preflight rows carried `class:bad` and `class:warn` and
    // NEITHER class existed, here or in `app.css` — so a pre-update check that
    // FAILED was painted in exactly the same grey as one that passed, on the
    // screen whose whole job is to say whether an update is safe right now. A
    // status that looks identical when it is bad news is not a status (rule 35),
    // and this one was invisible because a `class:` directive on a class nobody
    // defines is silent in every tool the project had.
    const used = [
      ...new Set([...MARKUP_ONLY.matchAll(/class:([A-Za-z0-9_-]+)=/g)].map((m) => m[1])),
    ];
    expect(used.length).toBeGreaterThan(5);
    const undefined_ = used.filter((c) => !STYLE.includes(`.${c}`) && !APPCSS.includes(`.${c}`));
    expect(undefined_, 'a class:… that no stylesheet defines paints nothing').toEqual([]);
  });
});

describe('the readiness surface is extended, never forked', () => {
  it('Dashboard is rendered from Diagnostics — the section that asks its question', () => {
    const diag = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf("section === 'diagnostics'"),
      MARKUP_ONLY.indexOf("section === 'privacy'"),
    );
    expect(diag).toMatch(/<Dashboard \/>/);
    expect(MARKUP_ONLY.match(/<Dashboard \/>/g)).toHaveLength(1);
  });

  it('and Settings still never mentions `greet` (rule 26 — it is a counter)', () => {
    expect(SRC).not.toMatch(/\bgreet\b/);
    expect(SRC).toMatch(/<Dashboard \/>/);
  });
});

describe('Screens & looks says whether the setting above it reaches anything', () => {
  it('lists the screens and the look each one is actually wearing', () => {
    // DECISIONS §70: a screen's own template wins over a content look (§29), so
    // the content-look map only reaches screens that have none. For most of this
    // product's life every screen always had one and the map changed nothing in
    // the building, with no way to tell from this page.
    const screens = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf("section === 'screens'"),
      MARKUP_ONLY.indexOf("section === 'audio'"),
    );
    expect(screens).toMatch(/screenLook\(ch\)/);
    expect(screens).toMatch(/<div class="rw-group">Screens<\/div>/);
    // …and it is a READOUT. The screens themselves are configured in Outputs.
    expect(screens).not.toMatch(/setChannelTemplate|setChannelDisplay/);
  });

  it('the look resolver reads the model’s own answer for "no template"', () => {
    expect(SCRIPT).toMatch(/ch\.template_id == null/);
    expect(SCRIPT).toMatch(/follows the content look/);
  });

  it('and a screen list that has not loaded says which kind of nothing it has', () => {
    expect(MARKUP_ONLY).toMatch(/screensState === 'loading'/);
    expect(MARKUP_ONLY).toMatch(/screensState === 'failed'/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// §11 · A RAIL PLUS ONE READING COLUMN.
//
// The page shipped with THREE columns: the section rail, the panel, and an
// "Overview" inspector carrying Version · Environment · Licence · Uptime and
// four quick links. §11 is a rail plus one column, capped and centred, and the
// prototype has no inspector on this workspace — a settings page has no "thing
// in hand" for one to be about.
//
// The column was not merely surplus. Every row on it was a second copy of a row
// somewhere else on the same page, and one of those copies — "Check for updates"
// — had to turn a check outcome into words itself, which is a second surface
// that can disagree with the first about whether Relay has ever reached the
// update server (rule 35, RG-83, RG-92). Deleting it is a rule-35 reduction as
// much as a layout fix.
// ─────────────────────────────────────────────────────────────────────────────
describe('§11 · two columns, and the third one is not coming back', () => {
  it('the frame is given two tracks, not three', () => {
    const cols = SRC.match(/columns="([^"]+)"/);
    expect(cols, 'Settings must pass an explicit column track').toBeTruthy();
    expect(cols[1].split('minmax').length - 1, 'one flexible main column').toBe(1);
    expect(cols[1]).not.toMatch(/\d+px\s+minmax\([^)]*\)\s+\d+px/);
  });

  it('there is no inspector pane on this workspace', () => {
    expect(MARKUP_ONLY).not.toMatch(/rw-insp/);
    expect(MARKUP_ONLY).not.toMatch(/s-qlink/);
    expect(STYLE).not.toMatch(/\.s-qlink\{/);
    expect(STYLE).not.toMatch(/\.s-ocard\{/);
  });

  it('the reading column is capped and centred, not left to fill a booth monitor', () => {
    expect(STYLE).toMatch(/\.s-read\{[^}]*max-width:880px/);
    expect(STYLE).toMatch(/\.s-read\{[^}]*justify-self:center/);
    expect(MARKUP_ONLY).toMatch(/class="rw-pane s-read"/);
  });

  it('nothing the deleted column carried became unreachable', () => {
    // The point of the test: a column may only be deleted if every fact on it has
    // a home. Version and Environment are the first rows of Updates; Uptime is
    // Diagnostics'; Licence moved onto the Privacy report. The four quick links
    // pointed at three rail entries and the Help tab, all still one click away.
    const sectionOf = (key) => {
      const i = MARKUP_ONLY.indexOf(`section === '${key}'`);
      const after = [...MARKUP_ONLY.matchAll(/section === '([a-z]+)'/g)]
        .map((m) => m.index)
        .filter((x) => x > i);
      return MARKUP_ONLY.slice(i, after.length ? after[0] : MARKUP_ONLY.length);
    };
    expect(sectionOf('updates')).toMatch(/Installed version/);
    expect(sectionOf('updates')).toMatch(/>Environment</);
    expect(sectionOf('diagnostics')).toMatch(/Uptime \(this run\)/);
    expect(sectionOf('privacy')).toMatch(/>Licence</);
    // …and the one thing that is now offered once instead of twice.
    expect(MARKUP_ONLY.match(/Check for Updates/gi)).toHaveLength(1);
  });

  it('the page title names the SECTION, so the role that identifies the page can', () => {
    // It used to be the literal word "Settings" — unchanging, while the rail
    // highlighted the section, the panel's own pane header repeated the section,
    // and the standfirst described it. Three of the four said the same thing and
    // the fourth said nothing.
    expect(SRC).toMatch(/title=\{activeSection\.label\}/);
    expect(SRC).not.toMatch(/title="Settings"/);
    const panel = MARKUP_ONLY.slice(MARKUP_ONLY.indexOf('<main class='));
    expect(panel).not.toMatch(/rw-panettl">\{activeSection\.label\}/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// §11 · ROW GEOMETRY. A floor, so a column of rows keeps one rhythm and one
// right edge instead of stepping in and out by eleven pixels a row depending on
// whether that row happens to have a note under its name.
// ─────────────────────────────────────────────────────────────────────────────
describe('§11 · the row', () => {
  it('has the 46px floor the spec asks for', () => {
    expect(STYLE).toMatch(/\.s-panel :global\(\.rw-nv\)\{[^}]*min-height:46px/);
  });

  it('and every control still sits in the frame’s one right-aligned column', () => {
    // `.rw-nvctl` is the right half of the row. A control rendered outside it
    // floats to its own edge and the column stops being a column — which is what
    // the shortcut key caps used to do, from a private row grammar of their own.
    expect(STYLE).not.toMatch(/\.s-scrow\{/);
    expect(MARKUP_ONLY).toMatch(/class="s-sckeys rw-nvctl"/);
  });

  // ── E1 · ONE SCALE MEANS ONE SCALE ────────────────────────────────────────
  //
  // §11 asks for one type scale with three roles, and `WorkspaceFrame` owns the
  // roles — a claim the tests above check for Settings' MARKUP. Neither of them
  // could see the other half: `Dashboard.svelte`, which renders inside Settings
  // as the Diagnostics readiness surface, carried SIX literal font sizes
  // (13.5px, 13px, 12.5px, 11px, 10px, 9.5px). Three were a token's value typed
  // out by hand and three were steps the scale does not have, so a Dashboard row
  // heading and a Settings row heading were a pixel and a half apart by
  // accident, and an edit to the tokens would have moved one and not the other.
  //
  // A literal is not forbidden because it is ugly. It is forbidden because it
  // does not MOVE: a scale one view has opted out of is not a scale, it is a
  // coincidence with a maintenance cost, and the coincidence is invisible until
  // somebody changes a token and half the product follows.
  it('neither file E1 owns types a font size the scale does not know', () => {
    const DASH = read('src/lib/views/Dashboard.svelte');
    for (const [name, src] of [
      ['Settings.svelte', SRC],
      ['Dashboard.svelte', DASH],
    ]) {
      const style = src.slice(src.indexOf('<style>'));
      const literals = [...style.matchAll(/font-size:\s*([0-9.]+)(px|rem|em)/g)].map(
        (m) => m[1] + m[2],
      );
      expect(literals, `${name} sets a font size outside --v-fs-*`).toEqual([]);
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE SHORTCUT TABLE IS ONE TABLE.
//
// Settings carried a SECOND, hand-written copy of six shortcut rows, and it had
// already drifted from the bindings it claimed to describe: `A` (accept the top
// AI suggestion) and `D` (dismiss it) were missing entirely, `/` was missing,
// the PgUp/PgDn aliases were missing, and `?` was described as opening Help when
// it opens the cheatsheet overlay. That is this repository's own named failure —
// a guarantee kept on one door and skipped on its twin — landing on a help
// screen, which is the worst place for it: it teaches an operator something
// false, under pressure. `shortcuts.js` owns the array; the keydown handler, the
// cheatsheet, Help and `sectionkeys.js::RESERVED` all read it, and now so does
// this page.
// ─────────────────────────────────────────────────────────────────────────────
describe('the Shortcuts section reads the canonical table', () => {
  it('Settings declares no shortcut table of its own', () => {
    expect(SCRIPT).not.toMatch(/const SHORTCUTS\s*=/);
    expect(SCRIPT).toMatch(/import \{ SHORTCUTS \} from '\.\.\/shortcuts\.js'/);
  });

  it('and renders that array, split by whether the key is always live', () => {
    // The split is the claim `always` makes: those three fire from the global
    // handler and survive a crashed view (rule 15, DECISIONS §20); the rest work
    // only where the surface registered the action. A list that ran the two
    // together would tell an operator that `A` is as reliable as `Esc`.
    const sc = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf("section === 'shortcuts'"),
      MARKUP_ONLY.indexOf("section === 'updates'"),
    );
    expect(sc).toMatch(/SHORTCUTS\.filter\(\(s\) => s\.always\)/);
    expect(sc).toMatch(/SHORTCUTS\.filter\(\(s\) => !s\.always\)/);
  });

  it('so the keys the old copy omitted are on the page by construction', () => {
    // Asserted against the SOURCE OF TRUTH rather than against Settings' markup:
    // the page now renders whatever that array holds, so the question worth asking
    // is whether the array still holds the keys whose absence was the bug. If `A`
    // or `D` is ever dropped from `shortcuts.js` this fails there, which is where
    // that decision would be made.
    const table = read('src/lib/shortcuts.js');
    const flat = [...table.matchAll(/\{ keys: \[([^\]]+)\]/g)].map((m) => m[1]).join(' ');
    for (const k of ["'A'", "'D'", "'/'", "'PgDn'", "'PgUp'", "'Esc'", "'B'", "'?'"])
      expect(flat, `${k} is missing from the canonical table`).toContain(k);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE SETUP WALK-THROUGH IS GUARDED DURING A RECORDED SERVICE.
//
// One click on this button sets `session.setupDone = false`, which mounts
// FirstRun full-screen over a live console. From inside it, `stopMicTest()` and
// `chooseDevice()` each stop the LIVE microphone, and "Try it" fires John 3:16
// to the congregation's screens — under a label that says none of that. The
// service lock could not reach it on its own: `restartSetup` is a session
// write, so `servicelock::guard` is never consulted, which is why the guard has
// to be on the button itself, reading the same live fact the "Unlock for this
// service" control six rows below it already reads. CLAUDE.md rule 44 names
// this exact sentence — its Escape half is fixed and pinned; this is the other
// half.
// ─────────────────────────────────────────────────────────────────────────────
describe('the setup walk-through is held back while a service is recording', () => {
  const history = MARKUP_ONLY.slice(
    MARKUP_ONLY.indexOf("section === 'history'"),
    MARKUP_ONLY.indexOf("section === 'shortcuts'"),
  );

  it('the button is disabled by a live fact that AGREES with the unlock control below it', () => {
    const btn = history.match(
      /<button\b[^<]*?on:click=\{restartSetup\}[^<]*?>Run the setup walk-through<\/button>/,
    )?.[0];
    expect(btn, 'no button calling restartSetup was found').toBeTruthy();
    // A bare/literal disable is furniture, same rule as the switches above.
    expect(btn).not.toMatch(/disabled(?![-\w=])|disabled=\{(true|false)\}|disabled="/);
    const cond = btn.match(/disabled=\{([^}]*)\}/)?.[1];
    expect(cond, 'the button must be conditionally disabled').toBeTruthy();
    const terms = cond.split('||').map((s) => s.trim());

    // AGREEMENT, not a remembered literal: the "Service lock" block below reads
    // its own `{#if …}` condition to decide whether to show "Unlock for this
    // service" at all. A guard that names a different or narrower fact than
    // that control is worse than no guard, so this asks the source for the
    // real condition rather than hard-coding a string that could drift from it.
    const serviceLockBlock = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('<div class="rw-group">Service lock</div>'),
      MARKUP_ONLY.indexOf('Unlock for this service'),
    );
    const lockCond = serviceLockBlock.match(/\{#if ([^}]*)\}/)?.[1]?.trim();
    expect(lockCond, 'could not find the Service lock block\'s own condition').toBeTruthy();
    expect(terms, 'disagrees with the unlock control it sits above').toContain(lockCond);

    // idle() (updater.js) deliberately reads more than one fact: a service can be
    // recording with the mic momentarily stopped, and a rehearsal can have the mic
    // live with no service lock armed at all — either one stopping the
    // walk-through's own microphone out from under an operator. Its lock half is
    // now two terms rather than one, for the reason this guard gained a third:
    // lifting the lock does not end the service. The button must cover the mic.
    expect(terms).toContain('$capture.capturing');
  });

  it('a disabled control carries its reason, in amethyst — never amber', () => {
    // Scoped to the {#if}…{/if} block itself, not a fixed character window —
    // a window wide enough to catch the warning text is also wide enough to
    // catch an unrelated control's amber a few hundred characters later.
    //
    // The block is found through the BUTTON'S OWN condition, not a remembered
    // literal. This line held the guard's exact three-term string and broke the
    // moment a third fact was added to it — the test above already makes the point
    // that a hard-coded copy of a condition drifts from the condition, and this one
    // was the copy. The reason that MATTERS is that a guard whose explanation sits
    // under a different condition can be disabled with nothing said.
    const guardCond = history
      .match(/<button\b[^<]*?on:click=\{restartSetup\}[^<]*?>/)?.[0]
      ?.match(/disabled=\{([^}]*)\}/)?.[1];
    expect(guardCond, 'no disabled condition on the walk-through button').toBeTruthy();
    const ifAt = history.indexOf(`{#if ${guardCond}}`);
    expect(
      ifAt,
      `the explanation does not sit under the button's own condition (${guardCond})`,
    ).toBeGreaterThanOrEqual(0);
    const closeAt = history.indexOf('{/if}', ifAt);
    const block = history.slice(ifAt, closeAt + '{/if}'.length);
    expect(block).toMatch(/class="rw-foot s-netwarn"/);
    expect(block).toMatch(/microphone is live|service is being recorded/i);
    expect(block).not.toMatch(/var\(--v-amber\)/);
    expect(block).not.toMatch(/class="[^"]*\bs-netbad\b/);
  });

  it('the Service lock label six rows below states the same fact in the same colour', () => {
    // One condition — a service is recording — said twice on this page, and it
    // used to be two different colours: amethyst here, amber (ON AIR — reserved,
    // and this page is never on air) at "Service lock". Both must read the same.
    const serviceLockBlock = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('<div class="rw-group">Service lock</div>'),
      MARKUP_ONLY.indexOf('Unlock for this service'),
    );
    expect(serviceLockBlock).toMatch(/A service is being recorded\./);
    expect(serviceLockBlock).not.toMatch(/var\(--v-amber\)/);
    expect(serviceLockBlock).toMatch(/class="s-netwarn"/);
  });
});

// The gap Important 1 (fix round 1) closed: every guard above reads MARKUP_ONLY,
// which is correct for the row (RG-133 records that exemption deliberately) but
// left the SCRIPT half of the same bug uncovered. `doCheckUpdates` composing its
// own "You're on the latest version." ternary instead of calling `describeChannel`
// is the Task 3 defect verbatim, and it lives entirely inside `<script>` — so a
// scanner that only ever reads MARKUP_ONLY would watch it come back and stay green.
describe('the update BUTTON goes through describeChannel too — not only the row', () => {
  it('doCheckUpdates calls describeChannel rather than composing its own sentence', () => {
    const fn = SCRIPT_ONLY.slice(
      SCRIPT_ONLY.indexOf('async function doCheckUpdates'),
      SCRIPT_ONLY.indexOf('checking = false;', SCRIPT_ONLY.indexOf('async function doCheckUpdates')),
    );
    expect(fn, 'doCheckUpdates was not found').toBeTruthy();
    expect(fn).toMatch(/describeChannel\(get\(updateChannel\)\)/);
  });

  it('and the script never re-invents the sentence describeChannel already owns', () => {
    // Comments are stripped by `strip()`, so this is the live code only — the
    // explanatory comment beside the fix is allowed to use these words; a
    // literal fallback string in the handler is not.
    expect(SCRIPT_ONLY).not.toMatch(/latest version/i);
    expect(SCRIPT_ONLY).not.toMatch(/up to date/i);
  });
});

// ── THE FIVE SMALLER FINDINGS (2026-09-15) ──────────────────────────────────
//
// Five controls on this page that did not say what had happened, plus a private
// copy of a map that has a store. A source scan is the right instrument for the
// structural claims below — which class, which handler, which region the markup
// carries — and it is an APPROXIMATION for the DSN ones, which are claims about
// behaviour over time.
//
// The first version of this paragraph said "nothing in this repository mounts
// Settings, and a fixture that did would be a fixture of everything". THAT WAS
// FALSE. `readstates.test.js` mounts `views/Settings.svelte`, presses through to
// a section and asserts on rendered text, in about four lines — and a false
// statement in a test file about what can be tested is the exact mechanism that
// kept the `stopCapture` comment alive. The behavioural half of F-8 lives there,
// under "Settings → the Sentry DSN": typing without saving, saving, and a save
// the backend does not honour.
describe('a Settings control says which of its outcomes happened', () => {
  it('F-3 · Detect speakers reads WHICH failure it was, not a bare false', () => {
    const fn = SCRIPT_ONLY.slice(
      SCRIPT_ONLY.indexOf('async function detectSpeakers'),
      SCRIPT_ONLY.indexOf('function pickOutput'),
    );
    expect(fn, 'detectSpeakers was not found').toBeTruthy();
    // The three situations that used to render identically.
    expect(fn).toMatch(/\.ok\b/);
    expect(fn).toMatch(/'denied'/);
    expect(fn).toMatch(/'no-input'/);
  });

  it('F-3 · and a refusal carries the way to reverse it', () => {
    // A refusal that only says "refused" leaves an operator with a dead button
    // and no next action. The OS path is the whole point of distinguishing it.
    const fn = SCRIPT_ONLY.slice(
      SCRIPT_ONLY.indexOf('async function detectSpeakers'),
      SCRIPT_ONLY.indexOf('function pickOutput'),
    );
    expect(fn).toMatch(/Privacy & Security/);
    expect(fn).toMatch(/Microphone/);
  });

  it('F-3 · the outcome is rendered, and in a live region', () => {
    expect(MARKUP_ONLY).toMatch(/\{#if outMsg\}[\s\S]{0,200}role="status"/);
  });

  it('F-9 · no inline amber survives anywhere in the markup', () => {
    // Amber means ON AIR and nothing else (rule 18). Settings is never on air,
    // and this file's own style block says "Rose, never amber" three times while
    // the demo-content edited count carried `style="color:var(--v-amber)"`.
    expect(
      MARKUP_ONLY,
      'An inline amber in Settings. Amber means ON AIR; use .s-netbad (rose, a ' +
        'failure) or .s-netwarn (amethyst, a caution).',
    ).not.toMatch(/--v-amber/);
  });

  it('F-9 · the demo edited count wears the caution class instead', () => {
    const demoBlock = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('<div class="rw-group">Demo content</div>'),
      MARKUP_ONLY.indexOf('<div class="rw-group">Service lock</div>'),
    );
    expect(demoBlock).toMatch(/been changed since/);
    expect(demoBlock).toMatch(/class="s-netwarn"/);
  });

  it('F-10 · both results that were announced to nobody now have live regions', () => {
    expect(MARKUP_ONLY).toMatch(/\{#if updateMsg\}[\s\S]{0,120}role="status"/);
    expect(MARKUP_ONLY).toMatch(/\{#if crashMsg\}[\s\S]{0,200}role="status"/);
  });

  it('F-8 · the Sentry DSN has a commit path of its own', () => {
    // `setCrashReporting` had exactly one caller — the switch — so with crash
    // reporting already on, editing the address wrote only a local object and
    // the next re-read put the old one back. The one control in Relay that
    // decides where data leaves this machine.
    const calls = SCRIPT_ONLY.match(/setCrashReporting\(/g) ?? [];
    expect(
      calls.length,
      'setCrashReporting has one caller again. The DSN can be edited with no way ' +
        'to commit it while the switch is already on.',
    ).toBeGreaterThan(1);
    expect(SCRIPT_ONLY).toMatch(/async function saveDsn/);
    expect(MARKUP_ONLY).toMatch(/on:click=\{saveDsn\}/);
  });

  it('F-8 · the switch commits the SAVED address, never the bound field', () => {
    // Turning reporting on is the operator's yes to WHETHER, not to WHERE. With
    // `crash.dsn` here, a half-typed address the page was calling "not saved yet"
    // became the live destination one click later — and `set_crash_reporting`
    // calls `telemetry::enable` on it in the same breath. The behavioural half is
    // in `readstates.test.js`; this is the one-line version that a reader of
    // `toggleCrash` will see.
    const fn = SCRIPT_ONLY.slice(
      SCRIPT_ONLY.indexOf('async function toggleCrash'),
      SCRIPT_ONLY.indexOf('async function saveDsn'),
    );
    expect(fn, 'toggleCrash was not found').toBeTruthy();
    expect(fn).toMatch(/setCrashReporting\(enabled, savedDsn\)/);
    // SCOPED TO THE CALL. This read `not.toMatch(/crash\.dsn/)` over the whole
    // function, which also forbade READING the bound field — and the function has
    // to read it, to put the operator's half-typed draft back after `acceptCrash`
    // has overwritten it with the saved address. What must never happen is
    // `crash.dsn` reaching the command, so that is what is asserted; `savedDsn` is
    // written in exactly one place (the test below), so it cannot be forged into
    // carrying the draft either.
    expect(fn).not.toMatch(/setCrashReporting\([^)]*crash\.dsn/);
    // …and the draft is restored AFTER the call, never merged into it.
    const call = fn.indexOf('setCrashReporting(');
    const restore = fn.indexOf('crash = { ...crash, dsn: draft }');
    expect(restore, 'the half-typed draft is discarded by a switch flip').toBeGreaterThan(-1);
    expect(restore).toBeGreaterThan(call);
  });

  it('F-8 · and does not commit on blur or on every keystroke', () => {
    // The failure mode of the option NOT taken: blur fires on any focus change,
    // so a half-typed or mis-pasted address would become the live destination
    // with no moment at which the operator said so — and reports already sent to
    // the wrong endpoint cannot be recalled.
    const field = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('id="crash-dsn"'),
      MARKUP_ONLY.indexOf('</div>', MARKUP_ONLY.indexOf('id="crash-dsn"')),
    );
    expect(field).toMatch(/bind:value=\{crash\.dsn\}/);
    expect(field).not.toMatch(/on:blur|on:change|on:input/);
  });

  it('F-8 · an unsaved address says so rather than looking committed', () => {
    // The Save button's own failure mode is an edit that is never saved. That one
    // can be made visible, which is why it was chosen over the silent one.
    expect(SCRIPT_ONLY).toMatch(/dsnDirty\s*=/);
    // `{#if}` or `{:else if}` — the unsaved line now sits behind the "could not
    // read the setting" branch, because an address the page could not read is not
    // an address with unsaved edits. Either spelling renders it; a literal `{#if}`
    // here would have failed on a correct page.
    expect(MARKUP_ONLY).toMatch(/\{(?:#if|:else if) dsnDirty\}/);
  });

  it('F-8 · savedDsn is only ever written from what the backend returned', () => {
    // Rule 15 on the smallest possible control: if the local copy were written
    // from what was ASKED for, a refused save would leave the field looking
    // committed while the engine reported somewhere else.
    // The declaration is not a write; every assignment after it must be one.
    const writes = SCRIPT_ONLY.match(/(?<!let )savedDsn\s*=/g) ?? [];
    expect(writes.length, 'savedDsn is written in more than one place').toBe(1);
    expect(SCRIPT_ONLY).toMatch(/function acceptCrash\(landed\)[\s\S]{0,160}savedDsn = landed/);
  });
});

describe('Settings keeps no private copy of a thing that has a store', () => {
  it('reads the canonical content kinds rather than a list of its own', () => {
    // A four-entry private list that predated the timer: the Countdown look could
    // be set in the Templates gallery and was invisible here.
    expect(SCRIPT_ONLY).toMatch(/contentTypes = CONTENT_KINDS/);
    expect(
      SCRIPT_ONLY,
      'Settings has grown its own content-kind list again.',
      // The SECTION list at the top of the file legitimately has a `scripture`
      // key — it is a Settings section, not a content kind. Anchor on the pairing
      // that only a content-look list has.
    ).not.toMatch(/key: 'song',\s*label:/);
  });

  it('and Settings, the gallery and the editor all agree about every kind', () => {
    const gallery = read('src/lib/views/templates/TemplateGallery.svelte');
    const editor = read('src/lib/views/templates/TemplateEditor.svelte');
    expect(CONTENT_KINDS.map((k) => k.key)).toContain('countdown');
    for (const src of [SRC, gallery, editor]) {
      expect(src).toMatch(/CONTENT_KINDS/);
    }
  });

  it('subscribes to the one store instead of refilling a local map', () => {
    expect(MARKUP_ONLY).toMatch(/\$contentTemplates\[ct\.key\]/);
    expect(
      SCRIPT_ONLY,
      'Settings holds a private ctMap again. `contentTemplates` is the one store ' +
        'and three surfaces used to keep private copies that silently disagreed.',
    ).not.toMatch(/\bctMap\b/);
    expect(SCRIPT_ONLY).toMatch(/loadContentTemplates\(\)/);
  });
});

describe('the offline model install goes through the one humaniser', () => {
  // Stripped, for the same reason the Settings scan is: the comment beside the
  // fix quotes the expression it replaced, and a scanner that reads comments
  // reports a fixed defect as still present.
  const MODEL = strip(read('src/lib/ModelSetup.svelte'));

  it('F-5 · installFound renders humanError, not a raw Rust string', () => {
    // Its six siblings on this surface already do. The defect is latent —
    // `install_from_file`'s own refusals are written for a volunteer — but the
    // command returns Result<String, String>, not the typed { kind, message },
    // so nothing constrains the next string it grows.
    const fn = MODEL.slice(MODEL.indexOf('async function installFound'), MODEL.indexOf('async function get('));
    expect(fn, 'installFound was not found').toBeTruthy();
    expect(fn).toMatch(/installMsg = humanError\(e\)/);
    expect(fn).not.toMatch(/e\?\.message \?\? String\(e\)/);
  });
});

// ── THE UPDATE STATUS VALUE IS PROSE, AND PROSE IN A FIGURE COLUMN EATS THE ──
//    NAME BESIDE IT.
//
// Task 3 grew `describeChannel`'s `failed` answer from 33 characters to 112, to
// carry the guidance the button used to hold on its own. That is the right
// sentence in the wrong cell: `.rw-nvv` is mono, right-aligned and sits in the
// `auto` half of `minmax(0,1fr) auto` (`views/WorkspaceFrame.svelte`), so the
// value takes its max-content width and the name is squeezed into whatever is
// left. MEASURED in a browser at an 878px row: "Update status" occupied 772px
// beside "up to date" and 98.8px beside the failure sentence. Through `.s-nvp` —
// the class this file already defines for exactly this case, and already uses on
// the Privacy report — the name gets 562.3px back and the value is capped at
// 46ch. A failure is still rose, because `.s-netbad` is declared after `.s-nvp`
// at equal specificity.
describe('the Update status row does not let its own sentence eat the name', () => {
  it('the value is a sentence cell, not a figure cell', () => {
    const row = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('<span class="rw-nvk">Update status</span>'),
      MARKUP_ONLY.indexOf('</div>', MARKUP_ONLY.indexOf('<span class="rw-nvk">Update status</span>')),
    );
    expect(row, 'the Update status row was not found').toBeTruthy();
    expect(row).toMatch(/class="s-nvp"/);
    // `.s-nvp` REPLACES `.rw-nvv` rather than joining it — keeping both leaves
    // the sentence in mono, which is the half of the defect that is about
    // reading rather than about width.
    expect(row).not.toMatch(/rw-nvv/);
    // And the failure is still rose.
    expect(row).toMatch(/class:s-netbad=/);
  });

  it('and so does the Last attempt row directly beneath it', () => {
    // The same defect at the same moment on the same screen: an arbitrary backend
    // string in the figure cell, rendered only when the channel has failed. The
    // measured page had it reading "Cannot read properties of undefined (reading
    // 'invoke')" — 51 characters of diagnostic squeezing a two-word name.
    const row = MARKUP_ONLY.slice(
      MARKUP_ONLY.indexOf('<span class="rw-nvk">Last attempt</span>'),
      MARKUP_ONLY.indexOf('</div>', MARKUP_ONLY.indexOf('<span class="rw-nvk">Last attempt</span>')),
    );
    expect(row, 'the Last attempt row was not found').toBeTruthy();
    expect(row).toMatch(/class="s-nvp"/);
    expect(row).not.toMatch(/rw-nvv/);
  });

  it('.s-netbad is declared after .s-nvp, so a failure still wins the colour', () => {
    // Equal specificity, same stylesheet: source order decides. Reordering them
    // would paint a failed update channel in the dim body colour.
    expect(STYLE.indexOf('.s-netbad{')).toBeGreaterThan(STYLE.indexOf('.s-nvp{'));
  });
});
