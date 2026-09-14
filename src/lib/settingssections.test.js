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
    // The rendered shape of the same defect. A `disabled` switch and a "Soon" chip
    // are what the two removed rows looked like; if either idiom comes back, some
    // control is being drawn as live-looking furniture again.
    expect(MARKUP_ONLY).not.toMatch(/role="switch"[\s\S]{0,200}?disabled/);
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
