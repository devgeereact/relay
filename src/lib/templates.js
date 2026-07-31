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
      accent: '#ffb000',
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
      accent: '#22d3ee',
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
      accent: '#8b5cf6',
      verseColor: '#1c1224',
      verseSize: '2.6',
      refSize: '1.7',
      italicRef: false,
    },
  },
  {
    // id 4 is WORSHIP LYRICS, not Lobby Warm — this list is the kiosk/OBS
    // fallback used when the DB is unreachable, so its ids MUST match the backend
    // seed order (db/templates.rs::builtin_templates). It had four entries while
    // the backend seeded five, so a kiosk resolving template_id=4 fell back to
    // Lobby Warm and rendered a song through a scripture look. Keep the two lists
    // in the same order.
    id: 4,
    name: 'Worship Lyrics',
    layout: { regions: ['verse_text'], align: 'center', lowerThird: false, refFirst: false },
    style: {
      font: 'var(--f-body)',
      background: '#07070a',
      accent: '#ffffff',
      verseColor: '#ffffff',
      verseSize: '9',
      refSize: '2',
      italicRef: false,
    },
  },
  {
    id: 5,
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
// ── VERSION HISTORY ──────────────────────────────────────────────────────────
// A template's saved history, so an edit can be rolled back after the fact (the
// editor's undo/redo is in-memory and gone on close). Kept as a bounded, newest-
// first list of snapshots persisted in the settings KV per template — small,
// local, and needing no schema migration (the incident-scarred path, rule 25).
// The trim/dedup logic is pure so it is tested; the store owns persistence.

export const TEMPLATE_VERSIONS_MAX = 20;

/** True when two snapshots have the same SHAPE (layout + style). The name is not
 *  compared — a rename alone is not worth a version, and it would let a rename
 *  spam the history. */
export function sameTemplateShape(a, b) {
  return JSON.stringify({ l: a?.layout ?? {}, s: a?.style ?? {} }) ===
    JSON.stringify({ l: b?.layout ?? {}, s: b?.style ?? {} });
}

/**
 * Prepend a snapshot of `template` to `list`, unless it is identical in shape to
 * the newest one already there (so a no-op save, or the editor's frequent live
 * autosaves, do not fill the history with duplicates). Trims to
 * TEMPLATE_VERSIONS_MAX. Pure — returns a new newest-first list. `ts` is the
 * caller's timestamp (the store passes Date.now()).
 */
export function appendTemplateVersion(list, template, ts) {
  const arr = Array.isArray(list) ? list : [];
  if (arr.length && sameTemplateShape(arr[0], template)) return arr; // no change → no version
  const entry = {
    ts,
    name: String(template?.name ?? 'Template'),
    layout: template?.layout ?? {},
    style: template?.style ?? {},
  };
  return [entry, ...arr].slice(0, TEMPLATE_VERSIONS_MAX);
}

/** Safe-parse a persisted versions blob to a newest-first array. Junk → []. */
export function parseTemplateVersions(json) {
  if (!json) return [];
  try {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((v) => v && typeof v === 'object' && typeof v.layout === 'object');
  } catch {
    return [];
  }
}

// ── EXPORT / IMPORT ──────────────────────────────────────────────────────────
// A template is portable — a church can share a lower-third or a stage-display
// design as a small JSON file. Only the SHAPE travels (name + layout + style),
// never an `id` or the `active` console flag, so importing always CREATES a new
// template and can never collide with an existing one or silently promote itself
// onto the console. Pure serialize/parse pair (tested); the file download/picker
// is thin glue in the store.

/** Marker written into an exported template file, so import can tell a Relay
 *  template from arbitrary JSON and refuse the latter clearly. */
export const TEMPLATE_FILE_MARKER = 'relay.template/v1';

/** Serialise a template to its exported JSON text. Strips id/active — an export
 *  is a design, not an identity or a console assignment. */
export function serializeTemplate(t) {
  return JSON.stringify(
    {
      marker: TEMPLATE_FILE_MARKER,
      name: String(t?.name ?? 'Template'),
      layout: t?.layout ?? {},
      style: t?.style ?? {},
    },
    null,
    2,
  );
}

/**
 * Parse an exported template file into `{ name, layout, style }`, or throw a
 * plain-language Error the caller shows through the ONE humaniser. Rejects
 * anything that isn't a Relay template file (wrong/absent marker, or a layout
 * that is not an object) rather than importing junk that would render as a blank
 * or broken slide. Never returns an id or the active flag — the caller saves it
 * as a NEW template.
 */
export function parseImportedTemplate(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("That file isn't valid JSON — it may be corrupt or not a template file.");
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error("That file isn't a Relay template.");
  }
  if (parsed.marker !== TEMPLATE_FILE_MARKER) {
    throw new Error("That file isn't a Relay template file (wrong or missing marker).");
  }
  if (!parsed.layout || typeof parsed.layout !== 'object' || Array.isArray(parsed.layout)) {
    throw new Error('That template file has no layout to import.');
  }
  const style = parsed.style && typeof parsed.style === 'object' && !Array.isArray(parsed.style) ? parsed.style : {};
  return { name: String(parsed.name ?? 'Imported template'), layout: parsed.layout, style };
}

export function parseTemplateOverride(templateJson) {
  if (!templateJson) return null;
  try {
    const parsed = JSON.parse(templateJson);
    // JSON.parse("42") and JSON.parse("null") are both valid and both useless
    // here — the renderer expects an object and would read .style/.layout off a
    // number. Anything that isn't an object falls back to the channel template.
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    return parsed;
  } catch {
    return null;
  }
}
