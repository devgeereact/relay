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

  it('the localStorage preferences that remain are only the two §69 kept', () => {
    // `Auto Start on Login` and `Minimize to System Tray` stay, disabled and
    // tagged, because they say what is missing rather than pretending to work.
    // Anything else in this object is a preference with no reader.
    const defaults = SCRIPT.slice(
      SCRIPT.indexOf('const DEFAULT_PREFS'),
      SCRIPT.indexOf('};', SCRIPT.indexOf('const DEFAULT_PREFS')),
    );
    const keys = [...defaults.matchAll(/^\s{4}(\w+):/gm)].map((m) => m[1]);
    expect(keys.sort()).toEqual(['autoStart', 'minimizeTray']);
  });

  it('and no other file opens that key, so the store is exactly as small as it looks', () => {
    const files = [];
    const walk = (dir) => {
      for (const e of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
        const p = `${dir}/${e.name}`;
        if (e.isDirectory()) walk(p);
        else if (/\.(svelte|js)$/.test(e.name) && !e.name.endsWith('.test.js')) files.push(p);
      }
    };
    walk('src');
    const users = files.filter((f) => read(f).includes('relay.prefs.v1'));
    expect(users).toEqual(['src/lib/views/Settings.svelte']);
  });
});

describe('acceptance 2 · the update line still reports the CHANNEL', () => {
  it('neither surface invents its own words for a check outcome', () => {
    // `describeChannel` is the one place a channel state becomes a sentence
    // (rule 35, RG-83, RG-92). The Updates row and the Overview rail's quick
    // link both call it; the restructure must not have left a third.
    const calls = [...MARKUP_ONLY.matchAll(/describeChannel\(\$updateChannel\)/g)];
    expect(calls.length).toBe(2);
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
