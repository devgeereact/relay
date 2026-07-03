// Built-in templates in seed order (== DB ids 1..4). Kiosk/OBS clients have no
// DB access, so they resolve a template_id against this list; the desktop app
// fetches the (editable) DB row instead. Sizes are in cqw (container-query
// width %) so the same template scales identically at any output size.

export const BUILTINS = [
  {
    id: 1,
    name: 'Classic Serif',
    layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
    style: {
      font: 'var(--f-serif)',
      background: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)',
      accent: '#e8a33d',
      verseColor: '#f4e4c8',
      verseSize: '5.5',
      refSize: '2.6',
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
      accent: '#4fa8c9',
      verseColor: '#ffffff',
      verseSize: '6',
      refSize: '2.6',
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
      accent: '#b080e0',
      verseColor: '#1c1224',
      verseSize: '2.6',
      refSize: '1.7',
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
      accent: '#e27d93',
      verseColor: '#f0dfe3',
      verseSize: '4',
      refSize: '2',
      italicRef: false,
    },
  },
];

export const DEFAULT_TEMPLATE = BUILTINS[0];

/** Resolve a template id to a built-in (kiosk fallback). */
export function builtinById(id) {
  return BUILTINS.find((t) => t.id === id) || DEFAULT_TEMPLATE;
}
