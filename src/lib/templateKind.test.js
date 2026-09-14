import { describe, it, expect } from 'vitest';
import { templateKind, kindsPresent, KIND_META, KIND_ORDER } from './templateKind.js';
import { STARTERS } from './layers.js';

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
