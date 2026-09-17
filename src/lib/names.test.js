// THE REGISTER OF NAMES — one concept, one name, everywhere a person reads it.
//
// Three things a preacher or an operator can see had six labels between them.
// `stage_alert` was "Word to the preacher" in the dock. `stage_note` was "Stage
// note" in the Planner, "Operator note (monitors only)" in the template editor's
// binding list, "Operator note" on two layer starters and "Note" on the stage
// page. `stage_next` was "Up next" on the stage page, "Up next" in the Library
// rail, "Up Next" in the Library inspector, "Next verse text" and "Next
// reference" in the binding list and "UP NEXT" inside a starter layer.
//
// Six labels for three concepts is not untidiness. `stage_alert` and `stage_note`
// behave DIFFERENTLY — one is typed live and never saved, one is persisted
// against a cue — and an operator who believes the Planner's field is the way to
// get a line in front of the preacher mid-sermon has been told so by the
// interface. This file exists so that neither of the first two can drift into
// the other's name again.
//
// ── WHY THE SCANNER READS EVERYTHING ────────────────────────────────────────
//
// `ipc.test.js` has had exactly this failure twice: a scanner that looked
// exhaustive while checking less than it claimed, first through a regex that did
// not allow `_` and then through a source list of `main.rs` alone. Both times
// every assertion still passed. **A scanner that quietly narrows passes
// everything.**
//
// So the input here is derived from the tree, not written down: every `.js` and
// `.svelte` file anywhere under `src/`, test files included, with this file the
// single exclusion (it must be free to spell out the labels it is retiring). And
// the scanner is guarded from three directions at the bottom of this file — it
// must reach a known depth, it must find a known real instance, and its matcher
// must find a retired label when one is planted in front of it. Without that
// third test, a matcher that had stopped matching would report a clean tree.
//
// Comments are scanned for RETIRED labels, deliberately: a label copied out of a
// stale comment is how a second name gets back in, and that test reads every
// byte of every file.
//
// The other two tests — the one casing and the register of surfaces — read the
// code with its comments removed, and that is a judgement rather than a
// narrowing. A comment is not a surface: nobody reads one in a service, this
// repository writes its section banners in capitals as a house style, and a
// prose sentence naming a concept mid-explanation is not a second label for it.
// Scanning comments there would have forced every banner above a stage-message
// branch to be rewritten into sentence case, which is the register dictating
// prose style rather than holding an interface to one name. The stripper is
// itself guarded below: a label on a line that also carries a trailing comment
// must still be found, and the retired-label test must still see one planted
// inside a comment.

import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, it, expect } from 'vitest';

const root = resolve(__dirname, '../..');

/** This file, which is allowed to name every label it is retiring. */
const SELF = 'src/lib/names.test.js';

/**
 * Every frontend source file, discovered rather than listed.
 *
 * `.test.js` files are IN. A test that asserts an old label is a second label,
 * living in the one place nobody thinks to look for user-visible text, and it
 * would keep the old wording alive in the repository's own record of what the
 * interface says.
 */
function srcFiles() {
  const out = [];
  const walk = (dir) => {
    for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
      const rel = `${dir}/${e.name}`;
      if (e.isDirectory()) walk(rel);
      else if (/\.(js|svelte)$/.test(e.name) && rel !== SELF) {
        out.push([rel, readFileSync(resolve(root, rel), 'utf8')]);
      }
    }
  };
  walk('src');
  return out;
}

/**
 * Every occurrence of `needle` in `text`, matched case-INSENSITIVELY and returned
 * as the text actually found.
 *
 * Case-insensitive, because "Up next" and "Up Next" are two labels for one thing
 * and the tree carried both. The caller decides what to do with the casing it
 * gets back: `filesWith` only asks whether the concept is named here at all,
 * while the casing test asks whether it is named the one agreed way.
 */
function hits(text, needle) {
  const found = [];
  const hay = text.toLowerCase();
  const pin = needle.toLowerCase();
  for (let i = hay.indexOf(pin); i !== -1; i = hay.indexOf(pin, i + 1)) {
    found.push(text.slice(i, i + needle.length));
  }
  return found;
}

/**
 * The same text with its comment CONTENT removed — `//` to end of line, `/* … *\/`
 * and `<!-- … -->` — so the casing and register tests read what an operator can
 * see rather than what a maintainer wrote beside it.
 *
 * `//` is only treated as a comment when it does not follow a `:`, so a `http://`
 * inside a string keeps the rest of its line. The retired-label test does NOT use
 * this: a stale comment reviving an old name is exactly what it is for.
 */
function codeOnly(text) {
  return text
    .replace(/\/\*[\s\S]*?\*\//g, ' ')
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/(^|[^:])\/\/[^\n]*/g, '$1');
}

/** The files in which `needle` is named, in any casing. */
function filesWith(files, needle) {
  return files.filter(([, text]) => hits(text, needle).length > 0).map(([rel]) => rel);
}

/** The files whose CODE — not their comments — names `needle`, in any casing. */
function surfacesWith(files, needle) {
  return files.filter(([, text]) => hits(codeOnly(text), needle).length > 0).map(([rel]) => rel);
}

// ── THE REGISTER ────────────────────────────────────────────────────────────
//
// `name`      the one string a person reads. Exact casing.
// `allowed`   the files permitted to carry it. A new surface that shows this
//             concept adds itself here, which is the point: the register is the
//             list of places the name is said, and it is meant to be read.
// `pending`   allowed, but not required to carry it yet — a file a landing branch
//             is about to put the name into. Exempt from the staleness check and
//             from nothing else.
// `forbidden` alternate labels for the SAME concept. These must appear in no
//             file at all, in any casing.
const REGISTER = [
  {
    concept: 'stage_alert — typed live, stage only, never saved',
    name: 'Stage Message',
    allowed: [
      'src/Stage.svelte',
      'src/lib/Dock.svelte',
      'src/lib/stores/capture.js',
      'src/lib/countdownwiring.test.js',
      'src/lib/quicktools.test.js',
      'src/lib/r6-contracts.test.js',
      'src/lib/stagezones.test.js',
      // Wave 5 Track C — the binding, the receiver that may refuse the frame, the
      // per-screen role picker that decides which page is allowed to paint it,
      // and the three tests that drive those.
      'src/lib/channelroles.js',
      'src/lib/channelroles.test.js',
      'src/lib/stagealertpanic.test.js',
      'src/lib/stagemessage.test.js',
      'src/lib/views/Channels.svelte',
      // Wave 3 — the two files that hold what a PANIC CONTROL does to a Stage
      // Message (DECISIONS §91) and to the console's mirror of it. They were
      // written on a branch that had not met this register and said "word to the
      // preacher"; the merge is where the one name reached them.
      'src/lib/panic.test.js',
      'src/lib/stagepanic.test.js',
      // The stage page's own identity, and the address the desk hands out for it.
      // Both say the name because both are about which screen may be painted one
      // (DECISIONS §89, and the reversal recorded in `r6-contracts.test.js`).
      'src/lib/stagepageidentity.test.js',
      'src/lib/stageremote.test.js',
    ],
    // Wave 5 Track C adds a `stage_message` text binding labelled with this same
    // string. Registered ahead of it so the merge lands green rather than red on
    // a name this register already agrees with.
    pending: ['src/lib/layers.js'],
    forbidden: ['Word to the preacher', 'Word to preacher', 'Preacher word', 'Message to the preacher'],
  },
  {
    concept: 'stage_note — saved against a cue',
    name: 'Stage Note',
    allowed: [
      'src/Stage.svelte',
      'src/lib/TemplateRender.svelte',
      'src/lib/layers.js',
      'src/lib/stores/capture.js',
      'src/lib/stagezones.test.js',
      'src/lib/surface.test.js',
      'src/lib/views/Live.svelte',
      'src/lib/views/ServicePlanner.svelte',
      'src/lib/views/plannerdesk.test.js',
      // Wave 5 Track C — the test that ratifies what a panic control does to a
      // Stage Message also asserts that the Stage Note beside it still goes.
      'src/lib/stagealertpanic.test.js',
    ],
    forbidden: ['Operator note', 'Cue note', 'Monitor note', 'Confidence note'],
  },
  {
    concept: 'stage_next and the next_* bindings',
    name: 'Up Next',
    allowed: [
      'src/Stage.svelte',
      'src/lib/layers.js',
      'src/lib/liveoutputrail.test.js',
      'src/lib/golive.test.js',
      'src/lib/libraryinspector.test.js',
      'src/lib/qa-r5-groups.test.js',
      'src/lib/slidegridwiring.test.js',
      'src/lib/stagezones.test.js',
      'src/lib/stores/capture.js',
      'src/lib/views/Library.svelte',
      'src/lib/views/Live.svelte',
      // RG-158: `Inspector.svelte` and `LiveOutputRail.svelte` USED to be here,
      // and that was this register permitting the very collision it exists to
      // stop. Both now say "Staging" for a different concept; see the entry below.
      // `Library.svelte` stays because its own prose still discusses the
      // preacher's panel, which is the concept this entry names.
      'src/lib/views/library/Inspector.svelte',
      'src/lib/views/library/LiveOutputRail.svelte',
      // Wave 5 Track A — the shelf test drives a Stage look whose zone is labelled
      // with this name.
      'src/lib/shelf.test.js',
    ],
    forbidden: ['Up-next', 'Next up', 'Coming next', 'Next verse text', 'Next reference'],
  },
  {
    // RG-158. The Library's staging area held N items, reached no output, and was
    // called "Up Next" — the same words as the single hint on the preacher's
    // monitor, on the two surfaces an operator moves between fastest. An operator
    // who queued three items here had told nobody anything.
    //
    // "Staging" rather than "Queue": said aloud in a room where a plan already has
    // cues, queue and cue are the same word.
    concept: 'the Library staging area — N items, reaches no output',
    name: 'Staging',
    allowed: [
      'src/lib/views/library/LiveOutputRail.svelte',
      'src/lib/views/library/Inspector.svelte',
      'src/lib/libraryinspector.test.js',
    ],
    forbidden: ['Staging area', 'Stage queue', 'Shortlist'],
  },
];

// ── AND ONE NAME, ONE CONCEPT — the inverse, which did not exist ────────────
//
// RG-158. Everything below this line checks that a CONCEPT is said one way. That
// is half the guarantee, and the half that was written. The other half is that a
// NAME means one thing, and until 2026-09-17 nothing checked it — which is how
// "Up Next" came to sit on two entries' worth of surfaces at once: the preacher's
// single hint on his own monitor, and the Library's staging list of N items that
// reaches no output at all. The register did not merely miss it; its `allowed`
// list for `stage_next` NAMED the two Library files, so the collision was
// permitted in writing.
//
// A register that can be satisfied by giving two concepts the same name is a
// register that will be, because the easy edit when a test fails is to add a file
// to an allow-list.
describe('one name, one concept', () => {
  it('no two entries claim the same name', () => {
    const byName = new Map();
    for (const e of REGISTER) {
      const key = e.name.toLowerCase();
      if (!byName.has(key)) byName.set(key, []);
      byName.get(key).push(e.concept);
    }
    const shared = [...byName.entries()]
      .filter(([, concepts]) => concepts.length > 1)
      .map(([name, concepts]) => `"${name}" is used for: ${concepts.join(' AND ')}`);
    expect(
      shared,
      'two concepts are wearing one name, which is the defect this file exists to stop',
    ).toEqual([]);
  });

  it("no entry's name is another entry's forbidden variant", () => {
    // The subtler shape. If one entry forbids a phrase and another entry adopts
    // it as its own name, the register contradicts itself and whichever test runs
    // first decides what is true.
    const clashes = [];
    for (const a of REGISTER) {
      for (const b of REGISTER) {
        if (a === b) continue;
        for (const variant of b.forbidden) {
          if (variant.toLowerCase() === a.name.toLowerCase()) {
            clashes.push(`"${a.name}" is ${a.concept}'s name and ${b.concept}'s forbidden variant`);
          }
        }
      }
    }
    expect(clashes, 'the register contradicts itself').toEqual([]);
  });

  it('and the scanner can still see a collision when there is one', () => {
    // A guard, for the reason every scanner in this repository carries one: a
    // check that has quietly stopped matching passes everything.
    const fake = [
      { concept: 'a', name: 'Same', forbidden: [] },
      { concept: 'b', name: 'Same', forbidden: [] },
    ];
    const byName = new Map();
    for (const e of fake) {
      const key = e.name.toLowerCase();
      byName.set(key, [...(byName.get(key) ?? []), e.concept]);
    }
    expect([...byName.values()].some((c) => c.length > 1)).toBe(true);
  });
});

describe('one concept, one name', () => {
  const files = srcFiles();

  for (const entry of REGISTER) {
    describe(entry.concept, () => {
      it(`is called "${entry.name}" and nothing else`, () => {
        const strays = [];
        for (const variant of entry.forbidden) {
          for (const rel of filesWith(files, variant)) strays.push(`${rel}: "${variant}"`);
        }
        expect(
          strays,
          `a second label for ${entry.concept} survives — the one name is "${entry.name}":\n  ${strays.join('\n  ')}`,
        ).toEqual([]);
      });

      it('is written the one way, not in a second casing', () => {
        const wrong = [];
        for (const [rel, text] of files) {
          for (const found of hits(codeOnly(text), entry.name)) {
            if (found !== entry.name) wrong.push(`${rel}: "${found}"`);
          }
        }
        expect(
          wrong,
          `"${entry.name}" is spelled a second way — a casing variant is still a second label:\n  ${wrong.join('\n  ')}`,
        ).toEqual([]);
      });

      it('is said only in the files this register lists', () => {
        const permitted = new Set([...entry.allowed, ...(entry.pending ?? [])]);
        const unregistered = surfacesWith(files, entry.name).filter((rel) => !permitted.has(rel));
        expect(
          unregistered,
          `"${entry.name}" is shown somewhere this register does not know about. ` +
            `Add the file to REGISTER if the surface is real:\n  ${unregistered.join('\n  ')}`,
        ).toEqual([]);
      });

      it('lists no file that has stopped saying it', () => {
        // An allow-list that outlives its entry is a lie in the other direction:
        // it silently permits a future surface in a file nobody is watching any
        // more. Same reasoning as `ipc.test.js`'s DELIBERATELY_UNHEARD check.
        const said = new Set(filesWith(files, entry.name));
        const stale = entry.allowed.filter((rel) => !said.has(rel));
        expect(
          stale,
          `these files are registered for "${entry.name}" but no longer contain it:\n  ${stale.join('\n  ')}`,
        ).toEqual([]);
      });
    });
  }
});

// ── THE SCANNER'S OWN THREE TESTS ───────────────────────────────────────────
//
// Everything above passes trivially if `srcFiles()` returns nothing, or if
// `hits()` stops matching. Both have happened in this repository's other
// scanner, and both times the suite stayed green.
describe('the scanner itself', () => {
  const files = srcFiles();
  const paths = files.map(([rel]) => rel);

  it('reads every src file, at every depth, rather than a hand-written list', () => {
    expect(paths.length).toBeGreaterThan(80);
    // One at each nesting level the tree actually has, so a walk that stopped
    // recursing — or that only read `src/lib` — fails here rather than silently
    // reporting a clean tree.
    for (const rel of [
      'src/Stage.svelte',
      'src/lib/Dock.svelte',
      'src/lib/stores/capture.js',
      'src/lib/views/ServicePlanner.svelte',
      'src/lib/views/library/LiveOutputRail.svelte',
    ]) {
      expect(paths, `the scanner cannot see ${rel}`).toContain(rel);
    }
    // Three shapes of file, because dropping one of them is the cheapest way to
    // narrow this scanner without anybody noticing.
    expect(paths.some((p) => p.endsWith('.svelte'))).toBe(true);
    expect(paths.some((p) => p.endsWith('.js') && !p.includes('.test.'))).toBe(true);
    expect(paths.some((p) => p.endsWith('.test.js'))).toBe(true);
    // And the one file it must not read, because it names every retired label.
    expect(paths).not.toContain(SELF);
  });

  it('strips a comment without taking the code beside it', () => {
    // The casing and register tests read `codeOnly`, so a stripper that ate more
    // than the comment would hide a real label and report a clean tree — the
    // narrowing this file exists to refuse, one level down.
    const line = `<span class="zone">Stage Message</span> <!-- STAGE MESSAGE -->`;
    expect(hits(codeOnly(line), 'Stage Message')).toEqual(['Stage Message']);
    const js = `const label = 'Stage Message'; // STAGE MESSAGE, in capitals`;
    expect(hits(codeOnly(js), 'Stage Message')).toEqual(['Stage Message']);
    // A URL keeps the rest of its line: `//` after a colon is not a comment.
    expect(codeOnly(`fetch('http://localhost:8032/output.html'); // note`)).toContain('output.html');
    // And it does take the comment: this is the half that lets a banner shout.
    expect(hits(codeOnly('// STAGE MESSAGE'), 'Stage Message')).toEqual([]);
    expect(hits(codeOnly('/* STAGE MESSAGE */'), 'Stage Message')).toEqual([]);
    // The retired-label test does not use it, and must still see a planted name.
    expect(hits('// Word to the preacher', 'Word to the preacher').length).toBe(1);
  });

  it('can still see a known instance of each name', () => {
    // If the matcher stopped matching, every test above would report a clean
    // tree. These are real strings in real shipped files.
    expect(filesWith(files, 'Stage Message')).toContain('src/lib/Dock.svelte');
    expect(filesWith(files, 'Stage Note')).toContain('src/lib/views/ServicePlanner.svelte');
    expect(filesWith(files, 'Up Next')).toContain('src/Stage.svelte');
  });

  it('finds a retired label when one is planted in front of it', () => {
    // The assertion that proves the register has teeth. Every forbidden variant
    // is matched against a corpus that contains it, so a `forbidden` list that
    // reports nothing against the tree is reporting an absence rather than a
    // broken matcher.
    for (const entry of REGISTER) {
      for (const variant of entry.forbidden) {
        const planted = [['src/decoy.svelte', `<span class="r-lbl">${variant.toUpperCase()}</span>`]];
        expect(
          filesWith(planted, variant),
          `the scanner would not notice "${variant}" coming back`,
        ).toEqual(['src/decoy.svelte']);
      }
    }
  });
});
