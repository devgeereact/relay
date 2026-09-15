// What ROLE this template is FOR — derived from what it actually renders, never
// stored. Relay's template model has no `role` column; a template is a layout +
// a style, and its role is a fact about that shape. Deriving it (rather than
// adding a field an operator must set, and every existing row must be
// back-filled with) keeps the gallery's rail honest: a row reflects what the
// template genuinely is, and can never disagree with the template.
//
// This is docs/REBRAND.md §3.3's register (`LOOKS`), and the rule that governs it
// is the one phase 4 closed: **a role may only appear once something renders it.**
// A role offered before its renderer exists is a control that saves a setting
// nothing reads. Which of the spec's ten are here, and which are refused, is
// recorded in DECISIONS §78.
//
// ── TWO MODELS, ONE ANSWER ──────────────────────────────────────────────────
// A template is either a LAYER stack (`layout.layers` — everything phases 2, 5,
// 6 and 12 build, and every starter in `layers.js`) or a legacy REGION list
// (`layout.regions` — the five seeded built-ins). This file was written for the
// region model alone and was never taught the other one, so every template an
// operator created from a starter — all twelve, the three lower thirds and the
// SuperSource composite included — came back `custom`. The rail could name a
// role it had a renderer for and still never show it, and the inspector's
// "Content type" row said Custom over a lower third. Both models are read here
// now, and `every_starter_lands_on_a_role_that_is_not_custom` is the tripwire:
// add a starter without a rule and the test names it.
//
// The rules for a LAYER stack, in order — most specific first, because several
// are true at once (a stage display has a clock AND a verse AND a reference):
//   - a `region` layer is a COMPOSITE — a template rendered inside a template,
//     which is what SuperSource is (§6);
//   - a `band` layer is a lower third, whatever it holds (§4);
//   - a MONITOR-ONLY binding (`next`, `next_reference`, `note`, `elapsed`,
//     `remaining`) means this screen is for a person on the platform or in the
//     booth, never a congregation — CLAUDE.md says so of those exact fields;
//   - a layer bound to `countdown` is a countdown screen. The rule asks about
//     the BINDING, not about `type: 'timer'`, because there is no such layer at
//     runtime: `makeLayer('timer')` normalises to a TEXT layer bound to
//     `countdown`, so a rule written against the type name reads perfectly and
//     matches nothing. Found by the starter tripwire below, which is what it is
//     for;
//   - a text layer that SCROLLS is a ticker, i.e. an announcement crawl;
//   - a `media` layer with no words of its own is a media slide;
//   - verse + reference together is scripture (a verse with its citation);
//   - verse ALONE, no reference, is a song/lyric slide (lyrics have no
//     "John 3:16" to show);
//   - anything else is custom.
//
// ── WHAT IS DELIBERATELY NOT DERIVED ────────────────────────────────────────
// **Pre-service.** §3.3 asks for it and nothing in a template's shape tells a
// pre-service look from an ordinary scripture one — "Lobby Warm" is a verse and
// a citation on a warm gradient, which is what every other scripture template
// is. A row for it would claim templates it cannot identify.
//
// **The three lower thirds separately.** §3.3 names `lower.name`, `lower.lyric`
// and `lower.bible`. A band holding one bound line and a band holding a verse
// with its citation are genuinely different shapes — but `lower.name` and
// `lower.bible` are not: both are a band with a verse line and a reference line,
// and the only thing separating "Ade Ogunlana / GUEST SPEAKER" from a verse and
// its citation is which words the operator points at them. They collapse to one
// `lower-third` role rather than being guessed apart by alignment, which is a
// style choice an operator may change at any time.

/** The bindings that only ever appear on a screen a congregation does not see. */
const MONITOR_BINDS = new Set(['next', 'next_reference', 'note', 'elapsed', 'remaining']);

const visible = (L) => L && L.visible !== false;

/** @returns a role key — see `KIND_META` for the full set. */
export function templateKind(t) {
  const layout = t?.layout ?? {};
  const layers = Array.isArray(layout.layers) ? layout.layers : [];
  if (layers.length) {
    const fromLayers = kindFromLayers(layers);
    // A DECLARATION SURVIVES A CONVERSION THAT LOST THE BAND.
    //
    // `regionsToLayers` draws a legacy lower third's backing bar as a `shape`
    // named 'Band'; it does not emit a `band` layer. `kindFromLayers` therefore
    // finds no band, falls through to "a verse line and a reference line", and
    // answers `scripture` — while `layout.lowerThird`, copied forward by that
    // same conversion (`{ ...layout, layers }`), sat right there saying
    // otherwise. Reading the shape and ignoring the declaration is how six of
    // the operator's eight lower thirds silently left the Quick tools picker:
    // `upgradeLegacyToLayers` runs on mount and SAVES, so every seeded band in
    // the database has been through it. `nameband.test.js` has the read.
    //
    // The declaration is read, never assumed: a template that has NOT declared
    // itself keeps whatever its layers say, and layers that positively name a
    // richer shape win — a `band` already agrees, and a `region` is a composite,
    // which is a claim about structure that a flag on the old model cannot make.
    if (layout.lowerThird && fromLayers !== 'supersource') return 'lower-third';
    return fromLayers;
  }

  const regions = Array.isArray(layout.regions) ? layout.regions : [];
  const has = (r) => regions.includes(r);
  if (layout.lowerThird) return 'lower-third';
  if (has('reference') && has('verse_text')) return 'scripture';
  if (has('verse_text') && !has('reference')) return 'song';
  return 'custom';
}

function kindFromLayers(all) {
  const layers = all.filter(visible);
  const ofType = (ty) => layers.some((L) => L.type === ty);
  const bound = (b) => layers.some((L) => L.type === 'text' && L.bind === b);

  if (ofType('region')) return 'supersource';
  if (ofType('band')) return 'lower-third';
  if (layers.some((L) => L.type === 'text' && MONITOR_BINDS.has(L.bind))) return 'stage';
  if (ofType('timer') || bound('countdown')) return 'timer';
  if (layers.some((L) => L.type === 'text' && L.scroll)) return 'announcement';

  const hasVerse = bound('verse');
  if (ofType('media') && !hasVerse) return 'media';
  if (hasVerse && bound('reference')) return 'scripture';
  if (hasVerse) return 'song';
  return 'custom';
}

/** Display metadata for a role: singular for the inspector, plural for a rail
 *  row, and `tag` for the card — the prototype's right-hand mono label. */
export const KIND_META = {
  scripture: { one: 'Scripture', many: 'Scripture', tag: 'SCRIPTURE' },
  song: { one: 'Song', many: 'Songs', tag: 'LYRICS' },
  'lower-third': { one: 'Lower Third', many: 'Lower Thirds', tag: 'LOWER THIRD' },
  supersource: { one: 'SuperSource', many: 'SuperSource', tag: 'SUPERSOURCE' },
  stage: { one: 'Stage Monitor', many: 'Stage', tag: 'STAGE MONITOR' },
  announcement: { one: 'Announcement', many: 'Announcements', tag: 'ANNOUNCEMENT' },
  media: { one: 'Media', many: 'Media', tag: 'MEDIA' },
  timer: { one: 'Timer', many: 'Timer / Countdown', tag: 'TIMER' },
  custom: { one: 'Custom', many: 'Custom', tag: 'CUSTOM' },
};

/** Display order for the rail. Mirrors the prototype's, minus the role this
 *  repository refuses to guess (pre-service) and plus the one it has that the
 *  spec's list omits (timer). */
export const KIND_ORDER = [
  'scripture',
  'song',
  'lower-third',
  'supersource',
  'stage',
  'announcement',
  'media',
  'timer',
  'custom',
];

/** The roles present in a template list, in a stable display order, with counts. */
export function kindsPresent(list) {
  const counts = {};
  for (const t of list ?? []) {
    const k = templateKind(t);
    counts[k] = (counts[k] ?? 0) + 1;
  }
  return KIND_ORDER.filter((k) => counts[k]).map((k) => ({ key: k, count: counts[k], ...KIND_META[k] }));
}
