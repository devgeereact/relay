import { describe, it, expect } from 'vitest';
import { templateKind, kindsPresent, KIND_META, KIND_ORDER } from './templateKind.js';
import { STARTERS, regionsToLayers } from './layers.js';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Shapes taken verbatim from the seeded built-ins in db/templates.rs, so these
// pin the derivation against the templates every install actually ships with.
const classicSerif = { layout: { regions: ['verse_text', 'reference'], lowerThird: false } };
const stageMono = { layout: { regions: ['reference', 'verse_text'], refFirst: true } };
const lowerThird = { layout: { regions: ['verse_text', 'reference'], lowerThird: true } };
const worshipLyrics = { layout: { regions: ['verse_text'] } };
const lobbyWarm = { layout: { regions: ['reference', 'verse_text'] } };

const starter = (key) => {
  const s = STARTERS.find((x) => x.key === key);
  if (!s) throw new Error(`no starter ${key} — the roster changed, so has this test's claim`);
  return { name: s.label, ...s.make() };
};

describe('templateKind — the legacy region model', () => {
  it('reads a verse-with-citation as scripture', () => {
    expect(templateKind(classicSerif)).toBe('scripture');
    expect(templateKind(stageMono)).toBe('scripture');
    expect(templateKind(lobbyWarm)).toBe('scripture');
  });

  it('reads verse-text alone as a song', () => {
    // Lyrics have no reference to show.
    expect(templateKind(worshipLyrics)).toBe('song');
  });

  it('reads a lower-third band as a lower third whatever regions it carries', () => {
    // The band wins even though this template also lists reference + verse.
    expect(templateKind(lowerThird)).toBe('lower-third');
  });

  it('falls back to custom for a shape that fits no rule', () => {
    expect(templateKind({ layout: { regions: [] } })).toBe('custom');
    expect(templateKind({ layout: { regions: ['reference'] } })).toBe('custom');
  });

  it('never throws on a malformed template', () => {
    expect(templateKind(null)).toBe('custom');
    expect(templateKind({})).toBe('custom');
    expect(templateKind({ layout: null })).toBe('custom');
    expect(templateKind({ layout: { regions: 'nonsense' } })).toBe('custom');
    expect(templateKind({ layout: { layers: 'nonsense' } })).toBe('custom');
    // The scroll rule below reads `style`, which need not be an object.
    expect(templateKind({ layout: { regions: ['verse_text'] }, style: 'nonsense' })).toBe('song');
    expect(templateKind({ layout: { regions: ['verse_text'] }, style: null })).toBe('song');
  });

  // ── THE SCROLLING ANNOUNCEMENT ───────────────────────────────────────────
  // The two models disagreed about one template, and the disagreement was not
  // stable — it resolved itself, wrongly, on a visit to the Templates tab.
  const announce = (style) => ({
    layout: { regions: ['reference', 'verse_text'], align: 'center', lowerThird: false, refFirst: true },
    style,
  });

  it('reads a scrolling region template as an announcement', () => {
    expect(templateKind(announce({ scroll: true }))).toBe('announcement');
  });

  it('but NOT a scrolling template with no verse region to scroll', () => {
    // THE NARROW HALF, held so the rule cannot widen back. `regionsToLayers`
    // carries `scroll` onto the verse layer and sets `scroll: false` on the
    // reference layer, so a reference-only template converts to a stack with no
    // scrolling layer — `custom`. A region rule that accepted `reference` would
    // answer `announcement` before the conversion and `custom` after it, which is
    // the drift this whole rule exists to remove, in the other direction. Both
    // halves are asserted across the conversion rather than in isolation, because
    // agreement is the actual contract.
    const refOnly = {
      layout: { regions: ['reference'], align: 'center', lowerThird: false, refFirst: true },
      style: { scroll: true },
    };
    const converted = { ...refOnly, layout: regionsToLayers(refOnly) };
    expect(templateKind(refOnly)).toBe('custom');
    expect(templateKind(refOnly)).toBe(templateKind(converted));
    // …and the reason: nothing in the converted stack scrolls.
    expect((converted.layout.layers ?? []).some((L) => L.scroll)).toBe(false);
  });

  it('and agrees with the layer model after the conversion that SAVES', () => {
    // THE BUG THIS RULE CLOSES. `TemplateGallery.upgradeLegacyToLayers` runs on
    // mount and saves the result, and `regionsToLayers` carries
    // `scroll: !!style.scroll` onto the verse layer — where `kindFromLayers` has
    // always answered `announcement`. So before this rule the same look derived
    // `scripture` in the gallery and `announcement` once converted, and the
    // conversion made the change permanent. Watched to fail by removing the
    // scroll line from the region branch: the first expectation reverts to
    // `scripture` while the second stays `announcement`.
    const t = announce({ scroll: true });
    const converted = { ...t, layout: regionsToLayers(t) };
    expect(templateKind(t)).toBe(templateKind(converted));
    expect(templateKind(converted)).toBe('announcement');
  });

  it('but a band that scrolls is still a lower third', () => {
    // The keyed announcement in the seeded Lower Third family. `lowerThird` is
    // read first, and it has to be: the layer branch gives the same answer for
    // the converted form, so reversing the order here would split the models
    // again in the other direction.
    const t = announce({ scroll: true });
    t.layout.lowerThird = true;
    expect(templateKind(t)).toBe('lower-third');
  });

  it('and a NON-scrolling notice is still scripture, which is honest', () => {
    // A full-screen notice and a full-screen verse are the same shape — a large
    // line and a small one. `Notice Board` records this as the reason
    // `templateKind` refuses to guess, and the rule must not start guessing now
    // that a neighbouring role is derivable. It also protects `Stage Mono`,
    // which is exactly this shape and must stay scripture.
    expect(templateKind(announce({}))).toBe('scripture');
    expect(templateKind(stageMono)).toBe('scripture');
  });
});

// ── THE LAYER MODEL ────────────────────────────────────────────────────────
// This half did not exist. `templateKind` read `layout.regions` only, so every
// template built from a starter — the layer model that phases 2, 5, 6 and 12 are
// entirely made of — came back `custom`: the rail could offer no role but
// Custom for anything an operator had created, and the inspector said "Content
// type · Custom" over a SuperSource composite.
describe('templateKind — the layer model (docs/REBRAND.md §3.3)', () => {
  it('reads every starter as a role, and not one of them as Custom', () => {
    // THE TRIPWIRE. `freestyle` is the one deliberate Custom — it is a blank
    // canvas and says so. Everything else in the roster renders something with a
    // name, so a starter that lands on `custom` is a rule this file is missing,
    // not a template without a role.
    const got = Object.fromEntries(STARTERS.map((s) => [s.key, templateKind({ ...s.make() })]));
    expect(got).toEqual({
      fullscreen: 'scripture',
      lyrics: 'song',
      'lower.name': 'lower-third',
      'lower.lyric': 'lower-third',
      'lower.bible': 'lower-third',
      media: 'media',
      announcement: 'announcement',
      stage: 'stage',
      confidence: 'stage',
      preacher: 'stage',
      timer: 'timer',
      supersource: 'supersource',
      freestyle: 'custom',
    });
  });

  it('a composite is SuperSource however much else it carries', () => {
    // The region layer wins over the bars and words around it: a template
    // rendered inside a template is what makes this kind the kind it is.
    expect(templateKind(starter('supersource'))).toBe('supersource');
  });

  it('a band is a lower third, and all three variants land on the one role', () => {
    // `lower.name` and `lower.bible` are the SAME shape — a band with a verse
    // line and a reference line. Only the words an operator points at them
    // differ, so they are not guessed apart.
    for (const k of ['lower.name', 'lower.lyric', 'lower.bible']) {
      expect(templateKind(starter(k)), k).toBe('lower-third');
    }
  });

  it('a monitor-only binding makes it a stage screen, not a congregation one', () => {
    // `next`, `note`, `elapsed` and `remaining` ride to output and no
    // congregation template renders them (CLAUDE.md, Frontend shape). A template
    // that shows one is for a person on the platform or in the booth.
    for (const bind of ['next', 'next_reference', 'note', 'elapsed', 'remaining']) {
      expect(
        templateKind({ layout: { layers: [{ type: 'text', bind: 'verse' }, { type: 'text', bind }] } }),
        bind,
      ).toBe('stage');
    }
  });

  it('a crawl is an announcement even though it binds a verse and a reference', () => {
    // The ticker's Label is bound to `reference` and its Crawl to `verse`, so
    // without the scroll rule an announcement would read as scripture.
    expect(templateKind(starter('announcement'))).toBe('announcement');
  });

  it('media with words of its own is not a bare media slide', () => {
    expect(templateKind({ layout: { layers: [{ type: 'media' }] } })).toBe('media');
    expect(
      templateKind({ layout: { layers: [{ type: 'media' }, { type: 'text', bind: 'verse' }] } }),
    ).toBe('song');
  });

  it('an invisible layer cannot give a template a role', () => {
    // Hiding the band is how an operator turns a lower third into something
    // else; the rail must follow what is rendered, not what is stored.
    expect(templateKind({ layout: { layers: [{ type: 'band', visible: false }, { type: 'text', bind: 'verse' }] } })).toBe('song');
  });
});

describe('KIND_META and KIND_ORDER', () => {
  it('every role the derivation can return has display metadata and a place in the order', () => {
    for (const key of KIND_ORDER) expect(KIND_META[key], key).toBeTruthy();
    expect(Object.keys(KIND_META).sort()).toEqual([...KIND_ORDER].sort());
  });

  it('every role but Custom can be CREATED from a starter', () => {
    // THE OTHER DIRECTION. The rail offers one row per role that occurs, so a
    // role with no starter behind it is a row an operator can filter to and
    // never add to. `Songs` was exactly that: `Worship Lyrics` shipped as a
    // seeded built-in, `templateKind` has always had a rule for a verse with no
    // reference, and no starter made one.
    const creatable = new Set(STARTERS.map((s) => templateKind({ ...s.make() })));
    const missing = KIND_ORDER.filter((k) => k !== 'custom' && !creatable.has(k));
    expect(missing, 'roles the rail can show but the New menu cannot make').toEqual([]);
  });

  it('refuses pre-service, because nothing in a shape identifies one', () => {
    // docs/REBRAND.md §3.3 names ten roles. A pre-service look is a verse and a
    // citation on a warm background, which is what every scripture template is,
    // so a row for it would claim templates it cannot tell apart (DECISIONS §78).
    expect(KIND_ORDER).not.toContain('preservice');
  });
});

describe('kindsPresent', () => {
  it('lists only roles that actually occur, in display order, with counts', () => {
    const kinds = kindsPresent([classicSerif, stageMono, lobbyWarm, worshipLyrics, lowerThird]);
    expect(kinds.map((k) => k.key)).toEqual(['scripture', 'song', 'lower-third']);
    expect(kinds.find((k) => k.key === 'scripture').count).toBe(3);
    expect(kinds.find((k) => k.key === 'song').count).toBe(1);
    // No 'custom' tab when nothing is custom — an empty type tab would mislead.
    expect(kinds.some((k) => k.key === 'custom')).toBe(false);
  });

  it('counts a layer-model shelf by its real roles', () => {
    const kinds = kindsPresent([starter('supersource'), starter('stage'), starter('lower.bible'), starter('fullscreen')]);
    expect(kinds.map((k) => k.key)).toEqual(['scripture', 'lower-third', 'supersource', 'stage']);
  });

  it('is empty for an empty library', () => {
    expect(kindsPresent([])).toEqual([]);
    expect(kindsPresent(undefined)).toEqual([]);
  });
});

describe('the seeded shelf', () => {
  // WHAT A CHURCH ACTUALLY FINDS, asked of `templateKind` rather than of a
  // fixture. The per-name table — which of the forty lands on which role, and why
  // three of the eight prefixes land somewhere other than their own name — lives
  // in `shelf.test.js`, beside the file it reads. What is held HERE is the claim
  // this module exists for: a seeded row may never come back `custom`.
  //
  // The failure phase 4 closed was exactly that. `templateKind` read
  // `layout.regions` only, so every layer template answered `custom`, the gallery
  // rail could offer no row but Custom for any of them, and the inspector's
  // "Content type" said Custom over a lower third. Wave 5 made the shelf entirely
  // layer-model, which puts all forty on that branch — so if it ever regresses,
  // it regresses for everything a fresh install has.
  const SHELF = JSON.parse(
    readFileSync(resolve(__dirname, '../../src-tauri/data/shelf_templates.json'), 'utf8'),
  ).templates;

  it('has forty looks and not one of them derives Custom', () => {
    expect(SHELF).toHaveLength(40);
    const custom = SHELF.filter((t) => templateKind(t) === 'custom').map((t) => t.name);
    expect(custom, 'a seeded look has no rule and falls to Custom').toEqual([]);
  });

  it('derives a role KIND_META can name and KIND_ORDER can place', () => {
    // A role with no display metadata renders as `undefined` in the rail; a role
    // missing from the order is dropped from it silently, which is worse.
    for (const t of SHELF) {
      const k = templateKind(t);
      expect(KIND_META[k], `${t.name}: no display metadata for ${k}`).toBeTruthy();
      expect(KIND_ORDER, `${t.name}: ${k} has no place in the rail`).toContain(k);
    }
  });
});
