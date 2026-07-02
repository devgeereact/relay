// Template shape reference + browser fallback.
//
// Templates now live in SQLite (seeded by db.rs, edited in the Templates tab)
// and are fetched by id. This module only provides the canonical shape and a
// fallback used when there's no Tauri backend (plain-browser preview at
// http://localhost:5032/output.html). See docs/SPEC.md §5.
//
// Shape: { id, name, layout: { regions, align, lowerThird, refFirst },
//          style: { font, background, accent, verseColor, verseSize, refSize, italicRef } }

export const DEFAULT_TEMPLATE = {
  id: 0,
  name: 'Classic Serif',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
  style: {
    font: 'var(--f-serif)',
    background: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)',
    accent: 'var(--amber)',
    verseColor: '#f4e4c8',
    verseSize: '4.6vw',
    refSize: '1.9vw',
    italicRef: true,
  },
};
