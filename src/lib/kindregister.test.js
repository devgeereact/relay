// RG-160 — THE RAIL OFFERS AN ANNOUNCEMENTS ROW THAT NOTHING ON THE SHELF CAN
// FILL, AND NO INSTRUMENT SAID SO.
//
// `KIND_ORDER` names nine roles. `kindsPresent` returns one rail row per role that
// OCCURS in a list, so the gallery's rail on a fresh install reads
// `All 40 · Scripture 9 · Songs 6 · Lower Thirds 5 · SuperSource 5 · Stage 5 ·
// Media 5 · Screen Countdown 5` — and there is no Announcements row, although the
// shelf ships a five-template family called `Announce · *` and `CONTENT_KINDS`
// offers **Announcements** as a content look on four surfaces. (RG-160's evidence
// cell quotes that rail with the countdown's old label; `names.test.js` holds the
// one name, so it is written here as **Screen Countdown** — and that test caught
// this comment quoting the row verbatim, which is the instrument working.)
//
// Before wave 5 the seeded `Classic · Announcement` DID derive `announcement`
// (RG-140 records it), so the wave removed the only rows that filled the bucket
// while adding five templates named for it. Nothing reported that: `shelf.test.js`
// records the per-name table and argues the split in prose, and
// `templateKind.test.js` asserts only that no seeded row derives `custom`.
//
// ── THE DECISION, AND WHY IT IS NOT "GIVE THE ANNOUNCE FAMILY A SIGNAL" ──────
//
// DECISIONS §129, taken with this file. RG-160 offered two ways out — give the
// Announce family a signal `templateKind` can read, or retire `announcement` from
// `KIND_META` and `KIND_ORDER` — and **both are wrong, for reasons that are
// measured below rather than argued.**
//
//   · THERE IS NO SIGNAL TO GIVE. A static full-screen notice is shaped exactly
//     like a full-screen verse: a title line and a body. The obvious candidate was
//     ORDER — a notice puts its title FIRST, a verse puts its citation LAST — and
//     `Scripture · Column` kills it: reference at `y13`, verse at `y26`, which is
//     `Announce · Board`'s geometry exactly. The case below asserts that collision
//     rather than trusting this paragraph, so the day somebody reaches for the
//     ref-first idea again the reason it fails is a failing test and not a memory.
//     `templateKind`'s own header already refuses `pre-service` for this exact
//     reason; deriving `announcement` from a static notice is the same guess.
//   · AND `announcement` IS NOT UNFILLABLE. It is derived by a **scrolling text
//     layer**, and two real paths produce one: the `Announcement Ticker` starter
//     an operator creates a template from, and a legacy region-model crawl once
//     `regionsToLayers` has converted it. Retiring the role would leave the
//     starter producing a template whose role the rail cannot name.
//
// So `announcement` stays, derivation stays shape-only (DECISIONS §78), and what
// changes is that the empty bucket stops being invisible. This file is the
// instrument RG-160's own test cell asks for — *"every kind in `KIND_ORDER` is
// derived by at least one seeded row, or is recorded by name as deliberately
// unfillable"* — with one strengthening: a kind no seeded row derives must still
// have a real PATH, and the path is named. A rail row with no path at all is the
// third state RG-160 says must not stand.
//
//   npx vitest run src/lib/kindregister.test.js

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { templateKind, kindsPresent, KIND_META, KIND_ORDER } from './templateKind.js';
import { STARTERS, regionsToLayers } from './layers.js';

const SHELF = JSON.parse(
  readFileSync(resolve(__dirname, '../../src-tauri/data/shelf_templates.json'), 'utf8'),
).templates;

const byName = (n) => SHELF.find((t) => t.name === n);
const starter = (key) => {
  const s = STARTERS.find((x) => x.key === key);
  expect(s, `no starter keyed '${key}'`).toBeTruthy();
  return s.make();
};

/** Which roles the SHELF derives, and which it does not. */
const FROM_SHELF = new Set(SHELF.map(templateKind));
/** Which roles a STARTER derives — the other real create path (a new template). */
const FROM_STARTERS = new Set(STARTERS.map((s) => templateKind(s.make())));

/**
 * ROLES NO SEEDED ROW DERIVES, each with the path that does and the reason the
 * shelf does not. Recorded by name, which is what RG-160 asks for instead of an
 * empty rail row nobody can account for.
 */
const NOT_ON_THE_SHELF = {
  announcement:
    'A scrolling text layer derives it. The five `Scroll · *` crawls carry one and are ' +
    'claimed first by the more specific band rule (a band is a lower third whatever it ' +
    'holds); the five `Announce · *` looks are STATIC notices, and a static notice is ' +
    'shape-identical to a verse — see the collision case below. Path: the ' +
    '`Announcement Ticker` starter, and any legacy region crawl once converted.',
  custom:
    'Nothing seeded is shapeless, and that is the point of `templateKind.test.js`. Path: ' +
    'the `Freestyle` starter, which is a background and nothing else — a blank canvas ' +
    'genuinely has no role yet.',
};

describe('RG-160 · every rail row has a path to it, and the empty ones are named', () => {
  it('the rail offers no role that nothing in the product can produce', () => {
    // THE THIRD STATE RG-160 SAYS MUST NOT STAND: a named bucket, templates named
    // for it, and no path between them. A role is reachable if the shelf derives it
    // or a starter does — those are the two ways a template comes into existence.
    const orphans = KIND_ORDER.filter((k) => !FROM_SHELF.has(k) && !FROM_STARTERS.has(k));
    expect(
      orphans,
      `KIND_ORDER offers ${orphans.join(', ')} and nothing seeded or startable derives it — ` +
        'either give the role a path or retire it from KIND_META and KIND_ORDER',
    ).toEqual([]);
  });

  it('and the roles the SHELF cannot fill are exactly the ones written down', () => {
    // Both directions, because each catches the opposite drift. A role that leaves
    // the shelf silently is the RG-160 defect itself; a note left behind for a role
    // the shelf now DOES derive is a stale exemption, which reads like a gap and is
    // how a register stops being believed.
    const missing = KIND_ORDER.filter((k) => !FROM_SHELF.has(k)).sort();
    expect(missing, 'a role left the shelf without a note being written for it').toEqual(
      Object.keys(NOT_ON_THE_SHELF).sort(),
    );
    for (const k of Object.keys(NOT_ON_THE_SHELF)) {
      expect(KIND_ORDER, `${k} is exempted but is not a rail row at all`).toContain(k);
      expect(NOT_ON_THE_SHELF[k].length, `${k}'s exemption gives no reason`).toBeGreaterThan(60);
    }
  });

  it('the Announcement Ticker starter is the path, and it really does derive the role', () => {
    // Named, not assumed. The role's whole claim to a place in `KIND_ORDER` is that
    // an operator can make one, and this is the making of it.
    expect(templateKind(starter('announcement'))).toBe('announcement');
    // And the OTHER path, which is what an existing install still holds: a legacy
    // region-model crawl, once `TemplateGallery` has converted it on mount.
    const legacyCrawl = {
      layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false },
      style: { scroll: true, verseColor: '#ffffff', accent: '#4fa8c9' },
    };
    expect(templateKind(legacyCrawl), 'the region model reads the same fact').toBe('announcement');
    expect(
      templateKind({ ...legacyCrawl, layout: regionsToLayers(legacyCrawl) }),
      'the conversion loses the role it had before the Templates tab was opened',
    ).toBe('announcement');
  });

  it('a static notice and a full-screen verse are the SAME shape, so no rule can tell them apart', () => {
    // THE MEASUREMENT THAT CLOSES THE "GIVE IT A SIGNAL" OPTION. The tempting rule
    // is order — a notice's TITLE comes first, a verse's CITATION comes last — and
    // one shipped scripture look already breaks it, so the rule would mislabel a
    // template of the family it was trying to protect.
    const textLayers = (t) =>
      (t.layout.layers || []).filter((L) => L.type === 'text' && L.visible !== false);
    const refFirst = (t) => {
      const ls = textLayers(t);
      const r = ls.find((L) => L.bind === 'reference');
      const v = ls.find((L) => L.bind === 'verse');
      return !!r && !!v && r.y < v.y;
    };
    const notice = byName('Announce · Board');
    const verse = byName('Scripture · Column');
    expect(notice && verse, 'the two templates this rests on are gone from the shelf').toBeTruthy();
    expect(refFirst(notice), 'the notice no longer puts its title first').toBe(true);
    expect(refFirst(verse), 'no shipped scripture look is reference-first any more').toBe(true);
    // Same shape, same answer. That is the honest state, not a miss.
    expect(templateKind(notice)).toBe(templateKind(verse));
  });

  it('every rail row the gallery can draw has display metadata', () => {
    // `kindsPresent` spreads `KIND_META[k]`, so a role in `KIND_ORDER` with no entry
    // renders a row with no label at all — a blank filter an operator can press.
    for (const k of KIND_ORDER) expect(KIND_META[k]?.many, `${k} has no plural label`).toBeTruthy();
    for (const row of kindsPresent(SHELF)) {
      expect(row.many, `a rail row for ${row.key} has no label`).toBeTruthy();
      expect(row.count, `a rail row for ${row.key} counts nothing`).toBeGreaterThan(0);
    }
    // And the rail is honest about the absence rather than drawing an empty row.
    expect(kindsPresent(SHELF).map((r) => r.key)).not.toContain('announcement');
  });
});
