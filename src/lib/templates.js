// Template shape reference + browser fallback.
//
// Templates now live in SQLite (seeded by db.rs, edited in the Templates tab)
// and are fetched by id. This module only provides the canonical shape and a
// fallback used when there's no Tauri backend (plain-browser preview at
// http://localhost:5032/output.html). See docs/SPEC.md §5.
//
// Shape: { id, name, layout: { regions, align, lowerThird, refFirst },
//          style: { font, background, accent, verseColor, verseSize, refSize, italicRef } }

// Built-in templates in seed order (== DB ids 1..4). Kiosk clients have no DB
// access, so they resolve a template_id against this list; the desktop app
// fetches the (editable) DB row instead.
export const BUILTINS = [
  {
    id: 1,
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
  },
  {
    id: 2,
    name: 'Stage Mono',
    layout: { regions: ['reference', 'verse_text'], align: 'left', lowerThird: false, refFirst: true },
    style: {
      font: 'var(--f-display)',
      background: '#000000',
      accent: 'var(--teal)',
      verseColor: '#f2f5f6',
      verseSize: '5vw',
      refSize: '2vw',
      italicRef: false,
    },
  },
  {
    id: 3,
    name: 'Lower Third',
    layout: { regions: ['verse_text', 'reference'], align: 'left', lowerThird: true, refFirst: false },
    style: {
      font: 'var(--f-body)',
      background: 'transparent',
      accent: 'var(--violet)',
      verseColor: '#1c1224',
      verseSize: '2.4vw',
      refSize: '1.4vw',
      italicRef: false,
    },
  },
  {
    id: 4,
    name: 'Lobby Warm',
    layout: { regions: ['reference', 'verse_text'], align: 'center', lowerThird: false, refFirst: false },
    style: {
      font: 'var(--f-serif)',
      background: 'linear-gradient(160deg, #241419, #120a0e)',
      accent: 'var(--rose)',
      verseColor: '#f0dfe3',
      verseSize: '3.2vw',
      refSize: '1.6vw',
      italicRef: false,
    },
  },
];

export const DEFAULT_TEMPLATE = BUILTINS[0];

/** Resolve a template id to a built-in (kiosk fallback). */
export function builtinById(id) {
  return BUILTINS.find((t) => t.id === id) || DEFAULT_TEMPLATE;
}
