// What KIND of template this is — derived from what it actually renders, never
// stored. Relay's template model has no `content_type` column; a template is a
// layout + a style, and its kind is a fact about that shape. Deriving it (rather
// than adding a field an operator must set, and every existing row must be
// back-filled with) keeps the gallery's type filter honest: the tab reflects what
// the template genuinely is, and can never disagree with the template.
//
// The rules, in order:
//   - a lower-third BAND is a lower third, whatever regions it carries;
//   - reference + verse together is scripture (a verse with its citation);
//   - verse text ALONE, no reference, is a song/lyric slide (lyrics have no
//     "John 3:16" to show);
//   - anything else is custom.
//
// Announcement and Countdown are deliberately NOT derived: nothing in the layout
// distinguishes an announcement template from a plain verse-text one (both are
// just `verse_text`), and countdown is content-driven, not a template shape. A
// tab for a kind that cannot be told apart from another would be a lie.

/** @returns one of 'lower-third' | 'scripture' | 'song' | 'custom' */
export function templateKind(t) {
  const layout = t?.layout ?? {};
  const regions = Array.isArray(layout.regions) ? layout.regions : [];
  const has = (r) => regions.includes(r);

  if (layout.lowerThird) return 'lower-third';
  if (has('reference') && has('verse_text')) return 'scripture';
  if (has('verse_text') && !has('reference')) return 'song';
  return 'custom';
}

/** Display metadata for a kind: singular for the inspector, plural for a tab. */
export const KIND_META = {
  scripture: { one: 'Scripture', many: 'Scripture' },
  song: { one: 'Song', many: 'Songs' },
  'lower-third': { one: 'Lower Third', many: 'Lower Thirds' },
  custom: { one: 'Custom', many: 'Custom' },
};

/** The kinds present in a template list, in a stable display order, with counts. */
export function kindsPresent(list) {
  const order = ['scripture', 'song', 'lower-third', 'custom'];
  const counts = {};
  for (const t of list ?? []) {
    const k = templateKind(t);
    counts[k] = (counts[k] ?? 0) + 1;
  }
  return order.filter((k) => counts[k]).map((k) => ({ key: k, count: counts[k], ...KIND_META[k] }));
}
