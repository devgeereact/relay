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

/**
 * Accent colours for the output-monitor wall, in order.
 *
 * One list, so the Console wall and the Planner's monitor column colour the same
 * channel the same way. They previously kept private arrays that had drifted
 * (`'gold'` vs `'amber'`), so monitor #1 was a different colour depending on
 * which tab you were looking at — for the same physical screen.
 */
export const MONITOR_ACCENTS = ['amber', 'cyan', 'amethyst', 'rose'];

/** The accent for the Nth monitor. */
export function monitorAccent(i) {
  return MONITOR_ACCENTS[i % MONITOR_ACCENTS.length];
}

/**
 * The per-content-type template override carried on fired content, or null.
 *
 * Malformed JSON falls back to null (= use the channel's own template) rather
 * than throwing. A bad template must never take the output screens down in front
 * of a congregation — degrade to the default look, don't blank the wall.
 *
 * Shared by the console preview, the Planner monitors and the real output page,
 * so what the operator previews is parsed by the exact same code as what the
 * congregation sees. It used to be reimplemented in all three.
 */
export function parseTemplateOverride(templateJson) {
  if (!templateJson) return null;
  try {
    return JSON.parse(templateJson);
  } catch {
    return null;
  }
}
