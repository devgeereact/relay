// THE NAME BAND THAT LOST SIX OF ITS EIGHT TEMPLATES.
//
// Operator report: "fix the Bug in the Name Band... not rendering as it should."
//
// ── WHAT IT ACTUALLY WAS: THE CLASSIFICATION ───────────────────────────────
//
// The Quick tools picker offers `$templates.filter((t) => templateKind(t) ===
// 'lower-third')`. The operator's database holds eight lower thirds — ids 3, 16,
// 17, 22, 26, 30 (seeded legacy region templates, `"lowerThird":true`) and 35, 36
// (the layered starters). Read back from that database, every one of the eight
// now carries LAYERS, because `TemplateGallery.upgradeLegacyToLayers` runs on
// mount and SAVES what `regionsToLayers` returns:
//
//     3|Lower Third         |shape:-,text:verse,text:reference
//    16|Lower Third Light   |shape:-,text:verse,text:reference
//    17|Lower Third Night   |shape:-,text:verse,text:reference
//    22|Aurora · Lower Third|shape:-,text:verse,text:reference
//    26|Ember · Lower Third |shape:-,text:verse,text:reference
//    30|Nocturne · Lower..  |shape:-,text:verse,text:reference
//    35|Lower Third · Lyric |band:-,text:verse
//    36|Lower Third · Scrip.|band:-,text:verse,text:reference
//
// `regionsToLayers` draws a band's backing as a `shape` named 'Band'. It never
// emits a `band` layer. So `kindFromLayers` finds no band, falls through to
// `bound('verse') && bound('reference')`, and answers **'scripture'** — for six
// of the eight. `templateKind` consults `layout.lowerThird` only on the REGION
// path, and the conversion has already left that path for good, even though it
// copies the flag forward (`{ ...layout, layers }`).
//
// So the picker silently drops six of eight, and the two survivors are the wrong
// two: `bands[0]` — the default selection — is "Lower Third · Lyric", which has
// NO reference layer at all. An operator fills in Name and Role, and the Role
// renders nowhere. That is the report, exactly.
//
// ── WHAT IT WAS NOT ────────────────────────────────────────────────────────
//
// Not the field mapping. A band's large line binds `verse` and its small line
// binds `reference`; the dock sends `{ reference: ltRole, text: ltName }`, so the
// NAME lands on the large line and the ROLE on the small one. That is correct,
// and the second describe below pins it so it stays correct.
//
// Not the band geometry and not the fit: `cardfit.test.js` owns both, and both
// are green.
//
// The one real defect on the `label` side is not fixable from the frontend:
// `fire_content` uses `label` BOTH as the projected small line AND as the cue's
// name in history ("The label still names the cue in history and in the plan",
// main.rs), so a name band is filed under the speaker's job title. Recorded in
// DECISIONS rather than fixed here — the Rust belongs to another agent this wave.
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { regionsToLayers, STARTERS } from './layers.js';
import { templateKind } from './templateKind.js';

/** The seeded legacy lower third, quoted from `src-tauri/src/db/templates.rs`. */
const SEEDED_BAND_LAYOUT = {
  regions: ['verse_text', 'reference'],
  align: 'center',
  lowerThird: true,
  refFirst: false,
};

const legacyBand = (name = 'Lower Third') => ({
  id: 3,
  name,
  layout: { ...SEEDED_BAND_LAYOUT },
  style: {},
});

/** What the gallery SAVED: the same template with `regionsToLayers` applied. */
const converted = (t) => ({ ...t, layout: regionsToLayers(t) });

describe('the name band picker — every lower third stays a lower third', () => {
  it('classifies the seeded legacy band as a lower third BEFORE the conversion', () => {
    // The region path has always been right about this one.
    expect(templateKind(legacyBand())).toBe('lower-third');
  });

  it('still classifies it as a lower third AFTER the gallery has converted and saved it', () => {
    // THE BUG. `upgradeLegacyToLayers` is not a preview — it writes. Every
    // lower third in the operator's database has been through it, which is why
    // the picker offers two of eight rather than eight of eight.
    const t = converted(legacyBand());
    expect(t.layout.layers.map((L) => L.type)).toEqual(['shape', 'text', 'text']);
    expect(templateKind(t)).toBe('lower-third');
  });

  it('keeps the declaration the conversion carried forward', () => {
    // `regionsToLayers` returns `{ ...layout, layers }`, so the flag survives.
    // A fix that threw the flag away instead of reading it would pass the test
    // above and lose the only durable evidence of what this template is.
    expect(converted(legacyBand()).layout.lowerThird).toBe(true);
  });

  it('classifies the layered starters as lower thirds too', () => {
    // The two the picker never lost. They answer from `ofType('band')`, a
    // different route, and must keep answering after the fix.
    for (const key of ['lower.name', 'lower.lyric', 'lower.bible']) {
      const s = STARTERS.find((x) => x.key === key);
      expect(templateKind(s.make()), key).toBe('lower-third');
    }
  });

  it('does not turn every converted template into a lower third', () => {
    // The flag is read, not assumed. A seeded SCRIPTURE template converts to the
    // same three layer types (background, verse, reference) and must stay
    // scripture — otherwise this fix empties the scripture row into the band row.
    const scripture = {
      id: 1,
      name: 'Classic Serif',
      layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
      style: {},
    };
    expect(templateKind(converted(scripture))).toBe('scripture');
  });

  it('is the filter the dock actually applies', () => {
    // The claim above is about `templateKind`; this is what ties it to the
    // control an operator uses. A fix to the derivation that the picker does not
    // read would be a fix to nothing.
    const DOCK = readFileSync(resolve(__dirname, 'Dock.svelte'), 'utf8');
    expect(DOCK).toMatch(/\$templates\.filter\(\(t\) => templateKind\(t\) === 'lower-third'\)/);
  });
});

describe('the name band — which line the name lands on', () => {
  let app = null;
  afterEach(() => {
    app?.$destroy?.();
    app = null;
    document.body.innerHTML = '';
  });

  /** Mount a template with the dock's own content shape and read the text boxes. */
  function lines(template, { name, role }) {
    const target = document.createElement('div');
    document.body.appendChild(target);
    // EXACTLY what `Dock.svelte` builds: `ltContent`, and the same values
    // `nameToProgramme` hands to `fireContent(label, text, …)`.
    const content = { reference: role, text: name, translation: null };
    app = new TemplateRender({ target, props: { template, content } });
    return [...target.querySelectorAll('.lfit')].map((el) => ({
      text: el.textContent.trim(),
      size: Number(el.dataset.base),
    }));
  }

  it('puts the NAME on the large line and the ROLE on the small one', () => {
    // The mapping was reported as backwards. It is not: `bind: 'verse'` reads
    // `content.text` (the name) and is the larger of the two, `bind: 'reference'`
    // reads `content.reference` (the role) and is the smaller.
    const got = lines(converted(legacyBand()), { name: 'Pastor Ade', role: 'Guest Speaker' });
    expect(got).toHaveLength(2);
    const big = got.reduce((a, b) => (a.size >= b.size ? a : b));
    const small = got.reduce((a, b) => (a.size < b.size ? a : b));
    expect(big.text).toBe('Pastor Ade');
    expect(small.text).toBe('Guest Speaker');
    expect(big.size).toBeGreaterThan(small.size);
  });

  it('does the same on the layered Name starter', () => {
    // Same claim, the other route into the renderer — a real `band` layer with
    // `members`, whose boxes come from `bandLayout` rather than from the layer's
    // own geometry.
    const t = STARTERS.find((x) => x.key === 'lower.name').make();
    const got = lines({ id: 99, name: 'Lower Third — Name', ...t }, { name: 'Pastor Ade', role: 'Guest Speaker' });
    const big = got.reduce((a, b) => (a.size >= b.size ? a : b));
    const small = got.reduce((a, b) => (a.size < b.size ? a : b));
    expect(big.text).toBe('Pastor Ade');
    expect(small.text).toBe('Guest Speaker');
  });
});
