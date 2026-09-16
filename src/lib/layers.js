// Layer-based template model — the ProPresenter-style editor's data.
//
// A template's `layout.layers` is an ordered array (first = back, last = front).
// Each layer is a typed, independently-styled, independently-positioned object.
// When `layout.layers` is present the renderer draws in LAYER mode; when it is
// absent it falls back to the legacy region rendering, so the built-in presets
// and themes (which are region-based) keep rendering exactly as before — one
// safe migration path, no broken shelves.
//
// Geometry is in PERCENT of the 16:9 stage (x,y = top-left corner, w,h = size),
// so a layer sits in the same place on a preview box and a 4K wall. Background
// layers ignore geometry (full frame). Text auto-fits inside its own box.
//
// Text layers BIND to the content Relay actually fires: a verse's text, its
// reference, its translation, a countdown, the clock — or a fixed string the
// operator types. Binding is what makes a layer template render live scripture
// rather than lorem ipsum.

import { migrateStyle, STYLE_DEFAULTS, bandLayout, faceOf } from './templatemodel.js';

let _seq = 0;
/** A stable-ish unique id. Not crypto — just needs to be unique within a template. */
function newId(prefix = 'l') {
  _seq += 1;
  return `${prefix}${_seq}_${Math.round(performance?.now?.() ?? 0)}`;
}

/** Text bindings → where the content comes from. */
export const BINDINGS = [
  { key: 'verse', label: 'Verse text' },
  { key: 'reference', label: 'Reference' },
  { key: 'translation', label: 'Translation' },
  { key: 'countdown', label: 'Countdown timer' },
  { key: 'clock', label: 'Clock' },
  // ROLE-MONITOR fields. These carry data that reaches OUTPUT content but is not
  // for the congregation: the verse coming up and the Stage Note.
  // A congregation template simply omits these layers; a stage/confidence
  // monitor includes them. `note` has flowed to output for ages with nothing
  // rendering it; `next` fields are populated by the fire path (resolve_fire).
  { key: 'next', label: 'Next verse text' },
  { key: 'next_reference', label: 'Next reference' },
  { key: 'note', label: 'Stage Note (monitors only)' },
  { key: 'elapsed', label: 'Service timer (elapsed)' },
  { key: 'remaining', label: 'Service timer (remaining)' },
  { key: 'static', label: 'Fixed text' },
];

export const LAYER_TYPES = [
  { type: 'text', label: 'Text', icon: 'T' },
  { type: 'band', label: 'Band (lower third)', icon: '▬' },
  { type: 'region', label: 'Slide region (composite)', icon: '▣' },
  { type: 'media', label: 'Media (image / video)', icon: '▷' },
  { type: 'shape', label: 'Shape', icon: '▢' },
  { type: 'background', label: 'Background', icon: '▦' },
  { type: 'timer', label: 'Timer / Countdown', icon: '⏱' },
];

/** Create a layer of `type` with sensible defaults, placed at a default box. */
export function makeLayer(type, over = {}) {
  const base = {
    id: newId(type[0]),
    type,
    name: '',
    visible: true,
    x: 10,
    y: 10,
    w: 80,
    h: 30,
  };
  let spec;
  switch (type) {
    case 'background':
      spec = {
        name: 'Background',
        x: 0, y: 0, w: 100, h: 100,
        fill: 'radial-gradient(130% 130% at 50% 20%, #12253f, #05080f)',
        image: null,
        opacity: 1,
        dim: 0,
      };
      break;
    case 'region':
      // A REAL RENDERED SLIDE inside its own container (docs/REBRAND.md §6).
      // `templateRef` names a BUILT-IN: a kiosk or OBS page has no database and
      // resolves ids against the bundled list, so a custom template here would
      // render one thing on the operator's wall and another in the stream.
      spec = {
        name: 'Slide region',
        x: 50, y: 8, w: 46, h: 84,
        templateRef: 1,
        radius: 1,
        outline: 0,
        outlineColor: 'theme:accent',
        plate: null,
        opacity: 1,
      };
      break;
    case 'media':
      // A MEDIA layer binds to the fired picture/video. It paints ONLY when media
      // is on screen, so a template that includes it shows media (at THIS layer's
      // z-order — put it on top to cover, lower to sit behind text), and a template
      // WITHOUT one simply never shows media on that screen. Defaults to a
      // full-frame cover fill — the "picture fills the wall" case.
      spec = {
        name: 'Media',
        x: 0, y: 0, w: 100, h: 100,
        fit: 'cover', // cover | contain
        opacity: 1,
        radius: 0,
      };
      break;
    case 'shape':
      spec = {
        name: 'Shape',
        x: 8, y: 74, w: 84, h: 18,
        fill: '#101319',
        opacity: 0.82,
        radius: 1.2,
      };
      break;
    case 'band':
      // A LOWER-THIRD BAND (docs/REBRAND.md §4). Not a shape with a helpful name:
      // it runs from `top` to the BOTTOM edge, is inset by the side safe area,
      // lifts its words off the baseline, and — the part no shape can do — it
      // NAMES the objects that live inside it, so it can give them ground before
      // they have to shrink. See `bandFit` in templatemodel.js for why that list
      // is declared rather than guessed at.
      spec = {
        name: 'Band',
        // x/y/w/h are derived from top/side (see `bandBox`) and kept in step so
        // anything that still reads a plain box — a thumbnail, an exporter — gets
        // the truth rather than the default 10/10/80/30.
        x: 6, y: 74, w: 88, h: 26,
        top: 74,
        side: 6,
        pad: 3,
        lift: 3,
        grow: 16,
        members: [],
        fill: '#101319',
        opacity: 0.9,
        radius: 1,
      };
      break;
    case 'timer':
      spec = {
        name: 'Timer',
        type: 'text',
        bind: 'countdown',
        x: 20, y: 34, w: 60, h: 32,
        font: 'var(--f-display)',
        color: '#ffffff',
        size: 12,
        align: 'center',
        valign: 'middle',
        transform: 'none',
        lineHeight: 1.1,
        letterSpacing: 0,
        shadow: 0.3,
        italic: false,
        scroll: false,
        text: '',
      };
      break;
    case 'text':
    default:
      spec = {
        name: 'Text',
        bind: 'verse',
        x: 10, y: 34, w: 80, h: 34,
        font: 'var(--f-serif)',
        color: '#f4e4c8',
        size: 5.2,
        align: 'center',
        valign: 'middle',
        transform: 'none',
        lineHeight: 1.32,
        letterSpacing: 0,
        shadow: 0.4,
        italic: false,
        scroll: false,
        text: '',
      };
      break;
  }
  return { ...base, ...spec, ...over };
}

/** Does this template use the layer model? */
export function isLayered(template) {
  return Array.isArray(template?.layout?.layers) && template.layout.layers.length > 0;
}

/**
 * Is this template KEYED — i.e. transparent, meant to composite over a live
 * camera (a lower third, a ticker), rather than paint its own full-frame
 * background? This drives blackout: a keyed channel must NOT be painted opaque
 * black (that hides the very camera it exists to caption); instead its content is
 * removed and the camera keeps going out.
 *
 * A template is keyed when nothing paints the whole frame:
 *   • layer model — no visible full-frame background layer with a real fill/image;
 *   • region model — an explicit lower third, or simply no opaque background.
 *
 * The old check was `layout.lowerThird`, which is FALSE for a layer-model lower
 * third (its band is a shape layer, not that flag) — so blackout blacked out the
 * camera and clear left the band sitting on it.
 */
export function isKeyedTemplate(template) {
  const layout = template?.layout ?? {};
  const style = template?.style ?? {};
  if (Array.isArray(layout.layers) && layout.layers.length) {
    const paintsFullFrame = layout.layers.some(
      (L) =>
        L.type === 'background' &&
        L.visible !== false &&
        (L.opacity == null || Number(L.opacity) > 0) &&
        ((L.fill && L.fill !== 'transparent') || L.image),
    );
    return !paintsFullFrame;
  }
  if (layout.lowerThird) return true;
  const hasBg = (style.background && style.background !== 'transparent') || style.bgImage;
  return !hasBg;
}

/** The content kinds a screen can be set to show/hide. */
export const CONTENT_KINDS = [
  { key: 'scripture', label: 'Scripture' },
  { key: 'song', label: 'Songs / Lyrics' },
  { key: 'media', label: 'Media' },
  { key: 'announce', label: 'Announcements' },
  { key: 'countdown', label: 'Timer / Countdown' },
];

/**
 * Does this screen (template) SHOW content of `kind`? Per-screen visibility: an
 * online wall shows everything; a stage / confidence monitor might show only
 * scripture, songs and the timer. A screen that doesn't show a kind simply
 * ignores that fire and holds what it had.
 *
 * `layout.shows` is the explicit allow-list (an array of kinds). When it's absent
 * the screen shows everything — except the legacy per-screen media opt-out
 * (`layout.noMedia`), folded in here so old templates keep working.
 */
export function templateShows(template, kind) {
  if (!kind) return true;
  const shows = template?.layout?.shows;
  if (Array.isArray(shows)) return shows.includes(kind);
  if (kind === 'media') return !template?.layout?.noMedia;
  return true;
}

/**
 * The template an output should actually render: its channel template, or a
 * content-type / cue OVERRIDE riding on the fired content — EXCEPT that a keyed
 * (transparent) channel must never be forced opaque. An opaque override on a
 * keyed channel is ignored so the lower third / ticker keeps keying over the live
 * camera; the verse still flows into the channel's own template. Opaque channels
 * take the override; a keyed override on a keyed channel is fine.
 *
 * `fallback` is the operator's CONFIGURED DEFAULT (`default_template_id`) and is
 * the last link: it answers only when nothing above it did. It is not part of
 * the ranking §29 and §70 describe — those decide between authorities that each
 * chose a look for this screen, and the default is what remains when none did.
 */
export function resolveOutputTemplate(channelTpl, override, pinned = false, fallback = null) {
  // NO TEMPLATE OF ITS OWN = this screen follows the content look (DECISIONS §70).
  // It has to be answered before the transparency law below, because
  // `isKeyedTemplate(null)` is true — a template with no background layer is keyed,
  // and an absent template has no layers at all — so a following screen would have
  // "kept its keyed template", which is nothing, and painted an empty frame.
  if (!channelTpl) return override ?? fallback ?? null;
  if (!override) return channelTpl;
  // TRANSPARENCY LAW: a keyed (lower-third) screen never goes opaque for an opaque
  // override — the camera it keys over must not be covered. Wins over everything.
  if (isKeyedTemplate(channelTpl) && !isKeyedTemplate(override)) return channelTpl;
  // A cue's DELIBERATE per-cue template choice (pinned) overrides the screen — the
  // operator picked that look for that item. A content-type DEFAULT (a "content
  // look") does NOT: the SCREEN'S OWN template is authoritative, so an operator
  // sees exactly the template they assigned to each screen. (This reverses the old
  // "content look overrides every screen" — operators found it silently replaced
  // the per-screen templates they had deliberately set. See DECISIONS §29.)
  if (pinned) return override;
  return channelTpl || override;
}

/** Format an elapsed duration (ms) as a service timer: `M:SS`, or `H:MM:SS` once
 *  it passes an hour. Negative / garbage clamps to `0:00`. Pure, so the renderer
 *  and its test share one definition. */
export function formatElapsed(ms) {
  const total = Math.max(0, Math.floor((Number(ms) || 0) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** Format a remaining duration (ms). Positive shows time left (`M:SS`); once the
 *  service runs OVER the planned length it goes negative and shows `-M:SS`, so a
 *  preacher can see they are past time. Reuses formatElapsed for the magnitude. */
/**
 * THE ONE COUNTDOWN FORMATTER. Read by the wall, the stage monitor and anything
 * else that shows the same number, so they cannot drift apart.
 *
 * `auto` shows `m:ss` and grows to `h:mm:ss` once there is an hour to show —
 * both previous copies stopped at minutes, so a 90-minute pre-service countdown
 * read `90:00`. `ms` and `hms` pin the shape for a template that wants one.
 */
export function formatCountdown(ms, mode = 'auto') {
  const total = Math.max(0, Math.round((Number(ms) || 0) / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const sec = total % 60;
  const pad = (n) => String(n).padStart(2, '0');
  if (mode === 'hms') return `${h}:${pad(m)}:${pad(sec)}`;
  if (mode === 'ms') return `${Math.floor(total / 60)}:${pad(sec)}`;
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

/** How long is left is a countdown's business; WHEN TO WORRY is this. */
export const COUNTDOWN_WARN_MS = 60_000;

/**
 * Is this countdown inside its warning window?
 *
 * The last minute — or the last tenth of a countdown shorter than ten minutes,
 * because a minute's warning on a two-minute countdown is a colour that is on for
 * half its life and therefore says nothing.
 *
 * A rule rather than a setting, deliberately: the control belongs in the Settings
 * pass, and a setting with nowhere to set it is worse than a sensible default.
 */
export function countdownWarning(remainingMs, totalMs = null) {
  const left = Number(remainingMs);
  if (!Number.isFinite(left) || left <= 0) return false;
  const span = Number(totalMs);
  const window = Number.isFinite(span) && span > 0
    ? Math.min(COUNTDOWN_WARN_MS, span / 10)
    : COUNTDOWN_WARN_MS;
  return left <= window;
}

export function formatRemaining(ms) {
  const n = Number(ms) || 0;
  return n < 0 ? `-${formatElapsed(-n)}` : formatElapsed(n);
}

/** The bound live value for a text/timer layer, given the fired content. */
export function boundValue(layer, content) {
  const c = content || {};
  switch (layer.bind) {
    case 'verse':
      return c.text || '';
    case 'reference':
      return c.reference || '';
    case 'translation':
      return c.translation || '';
    case 'next':
      return c.next_text || '';
    case 'next_reference':
      return c.next_reference || '';
    case 'note':
      return c.stage_note || '';
    case 'static':
      return layer.text || '';
    case 'countdown':
    case 'clock':
    case 'elapsed':
    case 'remaining':
      return ''; // computed live in the renderer (ticks), not from content
    default:
      return c.text || '';
  }
}

/** A friendly label for a layer in the layers panel. */
export function layerLabel(layer) {
  if (layer.name && layer.name.trim()) return layer.name.trim();
  if (layer.type === 'text' || layer.type === 'timer') {
    const b = BINDINGS.find((x) => x.key === layer.bind);
    return b ? b.label : 'Text';
  }
  if (layer.type === 'background') return 'Background';
  if (layer.type === 'media') return 'Media';
  if (layer.type === 'band') return 'Band';
  if (layer.type === 'region') return 'Slide region';
  return 'Shape';
}

/**
 * ══ WHAT A BAND CONTAINS ══
 *
 * One reader for `members`, so nothing anywhere has to know that it is an array
 * of ids that might hold a stale one. A band whose member was deleted names a
 * layer that is not there; every one of these skips it rather than rendering a
 * hole or throwing.
 */

/** The band (if any) that owns this layer. */
export function bandOf(layers, id) {
  if (!id) return null;
  return (Array.isArray(layers) ? layers : []).find(
    (L) => L && L.type === 'band' && Array.isArray(L.members) && L.members.includes(id),
  ) || null;
}

/** Is this layer inside some band? Members are drawn by their band, not by the stack. */
export function isBandMember(layers, id) {
  return !!bandOf(layers, id);
}

/** A band's members, in the order the band names them, skipping any that are gone. */
export function bandMembers(layers, band) {
  const list = Array.isArray(layers) ? layers : [];
  const ids = Array.isArray(band?.members) ? band.members : [];
  return ids.map((id) => list.find((L) => L && L.id === id)).filter(Boolean);
}

/**
 * Every layer that is not somebody's member — what the renderer and the canvas
 * draw at the top level. A member is drawn by its band, so drawing it here too
 * would paint it twice, once in the wrong place.
 */
export function topLevelLayers(layers) {
  const list = Array.isArray(layers) ? layers : [];
  return list.filter((L) => L && !isBandMember(list, L.id));
}

/**
 * WHERE EACH OBJECT IS ACTUALLY DRAWN, for every object whose drawn box is not
 * the box it stores — today that is a band and its words.
 *
 * ONE HOME, because there are two surfaces that must agree about this and they
 * are not the same code: `TemplateRender` paints the wall, and the editor's
 * canvas draws a selection handle over it. When those two disagree the handle
 * sits somewhere the words are not, which reads as a broken editor and is the
 * WYSIWYG guarantee (`TemplateRender` is THE one renderer) failing one layer
 * above the renderer.
 *
 * @param textOf  what each member currently says — a band's growth depends on
 *                the words in it, so the boxes depend on the content on screen.
 * @returns a Map of layer id → `{ x, y, w, h }`. A layer that is not in it draws
 *          at its own stored box.
 */
export function drawBoxes(layers, textOf = () => '') {
  const list = Array.isArray(layers) ? layers : [];
  const out = new Map();
  for (const L of list) {
    if (!L || L.type !== 'band') continue;
    const mem = bandMembers(list, L);
    const lay = bandLayout({
      band: L,
      members: mem.map((m) => ({ text: textOf(m), size: m.size, face: faceOf(m.font), h: m.h })),
    });
    out.set(L.id, lay.box);
    mem.forEach((m, i) => out.set(m.id, lay.members[i]));
  }
  return out;
}

// ── Starting-point templates (the "new template" chooser) ──────────────────
// Each returns a fresh `{ layout, style }` in layer mode. `style` is kept for the
// legacy renderer's sake but layer mode reads everything off the layers.

const SAMPLE_BG = 'radial-gradient(130% 130% at 50% 18%, #12253f, #05080f)';

/** Full-screen scripture: background + centred verse + reference beneath. */
function fullScreen() {
  return {
    layout: {
      layers: [
        makeLayer('background', { fill: SAMPLE_BG }),
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 8, y: 30, w: 84, h: 40, size: 5.2, color: '#eef2f8', shadow: 0.45 }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 8, y: 72, w: 84, h: 12, size: 2.5, color: '#f0b74a', align: 'center', italic: true, shadow: 0.4 }),
      ],
      align: 'center',
    },
    style: {},
  };
}

/** SONG LYRICS: the words, large, and NOTHING else.
 *
 *  THE ROLE YOU COULD FILTER TO AND NOT CREATE. `Worship Lyrics` ships as a
 *  seeded built-in and `templateKind` has always had a rule for a verse with no
 *  reference — but no starter made one, so the Templates rail offered a **Songs**
 *  row an operator could never add to. A kind you can filter to but not create
 *  is the same defect as one you can create but not filter to, in the other
 *  direction (docs/REBRAND.md §3.3; DECISIONS §78).
 *
 *  The values are the built-in's, in the layer model: sans (a lyric is scanned
 *  in a second and a half between breaths, not read like a paragraph), white on
 *  near-black for the most contrast a projector in a lit room can get, and no
 *  reference layer at all — a congregation is not singing the title. */
function songLyrics() {
  return {
    layout: {
      layers: [
        makeLayer('background', { fill: '#07070a' }),
        makeLayer('text', {
          name: 'Words', bind: 'verse', x: 6, y: 24, w: 88, h: 52,
          font: 'var(--f-body)', size: 7.6, color: '#ffffff',
          align: 'center', valign: 'middle', lineHeight: 1.24, shadow: 0.3,
        }),
      ],
      align: 'center',
    },
    style: {},
  };
}

// ── THE THREE LOWER THIRDS ─────────────────────────────────────────────────
//
// A band is keyed over a live camera, so NONE of these carries a background
// layer: the rest of the frame stays transparent (see `isKeyedTemplate`, and the
// transparency law in TemplateRender).
//
// They are three separate templates rather than one with options, which is what
// makes "editing one touches no other" a property of the model instead of a
// thing to remember.

/** The band itself, at the one geometry all three share, holding the words that
 *  were built for it.
 *
 *  MEMBERSHIP IS SET HERE, ONCE, AT BUILD TIME. A starter is the only place that
 *  knows which words belong to which band, so it says so — and every later
 *  reader (the renderer, the fit, duplicate, delete) works off that list rather
 *  than off a name, a z-order or a bounding box. `bandWith` keeps the two halves
 *  from drifting: it cannot return a band whose members are not the layers
 *  beside it. */
const bandWith = (...members) => {
  const b = makeLayer('band');
  return [{ ...b, members: members.map((m) => m.id) }, ...members];
};

/** NAME: who is speaking, and what they are. The name is the large line. */
function lowerName() {
  return {
    layout: {
      layers: bandWith(
        makeLayer('text', { name: 'Name', bind: 'verse', x: 9, y: 76, w: 82, h: 10, size: 3.2, color: '#f2f4f8', align: 'left', valign: 'middle', shadow: 0 }),
        makeLayer('text', { name: 'Role', bind: 'reference', x: 9, y: 86, w: 82, h: 5, size: 1.4, color: '#9db4ff', align: 'left', transform: 'uppercase', letterSpacing: 0.1 }),
      ),
      align: 'left',
    },
    style: {},
  };
}

/** LYRIC: the words, and NOTHING else — no reference layer at all.
 *  A song's "reference" is its title, and a title under every line reads like a
 *  slide rather than a caption. The line is given the whole band. */
function lowerLyric() {
  return {
    layout: {
      layers: bandWith(
        makeLayer('text', { name: 'Words', bind: 'verse', x: 9, y: 75, w: 82, h: 16, size: 3, color: '#f2f4f8', align: 'left', valign: 'middle', shadow: 0 }),
      ),
      align: 'left',
    },
    style: {},
  };
}

/** SCRIPTURE: the verse, with its reference beneath — right-aligned, tracked and
 *  small, so it reads as a citation rather than as a second sentence. */
function lowerBible() {
  return {
    layout: {
      layers: bandWith(
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 9, y: 76, w: 82, h: 10, size: 2.6, color: '#f2f4f8', align: 'left', valign: 'middle', shadow: 0 }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 9, y: 86, w: 82, h: 5, size: 1.5, color: '#9db4ff', align: 'right', transform: 'uppercase', letterSpacing: 0.08 }),
      ),
      align: 'left',
    },
    style: {},
  };
}

/** SuperSource: a camera half and a rendered slide half, composited (§6).
 *
 *  The camera half is what is NOT painted. Relay does not take a camera feed —
 *  NDI is parked, and a camera reaches the building through OBS or an ATEM — so
 *  a composite here is a KEYED layout and the switcher puts the picture behind
 *  the transparent half. A grey rectangle captioned "camera" would be a picture
 *  of a feature rather than the feature. */
function superSource() {
  return {
    layout: {
      layers: [
        makeLayer('shape', { name: 'Top bar', x: 0, y: 0, w: 100, h: 7, fill: '#0b0d12', opacity: 0.92, radius: 0 }),
        makeLayer('shape', { name: 'Bottom bar', x: 0, y: 93, w: 100, h: 7, fill: '#0b0d12', opacity: 0.92, radius: 0 }),
        makeLayer('region', { name: 'Word region', x: 52, y: 9, w: 44, h: 82, templateRef: 1 }),
      ],
      align: 'center',
    },
    style: {},
  };
}

/** Announcement ticker: a bottom bar + a scrolling body line. */
function announcement() {
  return {
    layout: {
      layers: [
        makeLayer('shape', { name: 'Ticker bar', x: 0, y: 88, w: 100, h: 12, fill: '#0b3330', opacity: 0.95, radius: 0 }),
        makeLayer('text', { name: 'Label', bind: 'reference', x: 1.5, y: 89, w: 20, h: 10, size: 2.6, color: '#6ee7c4', align: 'left', valign: 'middle', transform: 'uppercase', letterSpacing: 0.06 }),
        makeLayer('text', { name: 'Crawl', bind: 'verse', x: 22, y: 89, w: 77, h: 10, size: 3, color: '#eafff8', align: 'left', valign: 'middle', scroll: true }),
      ],
      align: 'left',
    },
    style: {},
  };
}

/** Full-screen media: a fired picture/video fills the wall. A screen using this
 *  shows media; screens whose templates omit the media layer never will. */
function mediaFull() {
  return { layout: { layers: [makeLayer('media', { name: 'Media', fit: 'cover' })], align: 'center' }, style: {} };
}

/** Freestyle: just a background to build on. */
function freestyle() {
  return { layout: { layers: [makeLayer('background', { fill: '#0a0a0a' })], align: 'center' }, style: {} };
}

// ── ROLE OUTPUTS ─────────────────────────────────────────────────────────────
// A "stage display" / "confidence monitor" / "preacher view" is NOT a separate
// engine — it is a normal template whose layers show role-relevant fields. These
// starters are those profiles. They are theme-aware out of the box (colours bind
// to theme tokens), and use ONLY the bindings that exist today: the current
// verse, its reference, the wall clock and fixed labels. Richer role fields —
// NEXT verse, speaker notes, elapsed/remaining service time — need new content
// plumbing through the fire path and are a deliberate follow-up, not faked here.

/** Stage display: a platform-facing monitor. Dark opaque background (it is a
 *  screen, not a keyed camera overlay), a role label, the wall clock, and the
 *  current verse + reference large enough to read from the stage. */
function stageDisplay() {
  return {
    layout: {
      layers: [
        makeLayer('background', { name: 'Screen', fill: 'theme:background', x: 0, y: 0, w: 100, h: 100 }),
        makeLayer('text', { name: 'Role label', bind: 'static', text: 'STAGE', x: 4, y: 3, w: 26, h: 7, size: 1.8, color: 'theme:accent', font: 'theme:font', align: 'left', valign: 'middle', transform: 'uppercase', letterSpacing: 0.12 }),
        makeLayer('text', { name: 'Service timer', bind: 'elapsed', x: 37, y: 3, w: 26, h: 7, size: 2.2, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Clock', bind: 'clock', x: 70, y: 3, w: 26, h: 7, size: 2.2, color: 'theme:accent', font: 'theme:font', align: 'right', valign: 'middle' }),
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 6, y: 16, w: 88, h: 46, size: 4.6, color: 'theme:verse', font: 'theme:font', align: 'left', valign: 'middle' }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 6, y: 63, w: 88, h: 7, size: 2.4, color: 'theme:reference', font: 'theme:font', align: 'left', valign: 'middle', transform: 'uppercase', letterSpacing: 0.06 }),
        makeLayer('text', { name: 'Up-next label', bind: 'static', text: 'UP NEXT', x: 6, y: 74, w: 40, h: 5, size: 1.5, color: 'theme:accent', font: 'theme:font', align: 'left', valign: 'middle', transform: 'uppercase', letterSpacing: 0.14 }),
        makeLayer('text', { name: 'Next reference', bind: 'next_reference', x: 6, y: 79, w: 88, h: 5, size: 1.8, color: 'theme:reference', font: 'theme:font', align: 'left', valign: 'middle' }),
        makeLayer('text', { name: 'Next verse', bind: 'next', x: 6, y: 84, w: 88, h: 13, size: 2.2, color: 'theme:verse', font: 'theme:font', align: 'left', valign: 'top', opacity: 0.8 }),
      ],
      align: 'left',
    },
    style: {},
  };
}

/** Confidence monitor: booth/floor-facing. Same fields as the stage display,
 *  centred and framed as the operator's "what is on screen now" confidence view. */
function confidenceMonitor() {
  return {
    layout: {
      layers: [
        makeLayer('background', { name: 'Screen', fill: 'theme:background', x: 0, y: 0, w: 100, h: 100 }),
        makeLayer('text', { name: 'Role label', bind: 'static', text: 'CONFIDENCE', x: 4, y: 4, w: 30, h: 8, size: 1.9, color: 'theme:accent', font: 'theme:font', align: 'left', valign: 'middle', transform: 'uppercase', letterSpacing: 0.12 }),
        makeLayer('text', { name: 'Service timer', bind: 'elapsed', x: 36, y: 4, w: 28, h: 8, size: 2.2, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Clock', bind: 'clock', x: 66, y: 4, w: 30, h: 8, size: 2.2, color: 'theme:accent', font: 'theme:font', align: 'right', valign: 'middle' }),
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 8, y: 20, w: 84, h: 48, size: 5.2, color: 'theme:verse', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 8, y: 69, w: 84, h: 7, size: 2.6, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Stage Note', bind: 'note', x: 8, y: 88, w: 84, h: 9, size: 2, color: 'theme:accent', font: 'theme:font', align: 'center', valign: 'middle', italic: true }),
      ],
      align: 'center',
    },
    style: {},
  };
}

/** Preacher view: a platform monitor built for the person speaking — a big
 *  centred verse, the reference, the verse coming up, the service timer + clock,
 *  and the operator's private note. Everything the preacher needs, nothing the
 *  congregation sees. Theme-aware. */
function preacherView() {
  return {
    layout: {
      layers: [
        makeLayer('background', { name: 'Screen', fill: 'theme:background', x: 0, y: 0, w: 100, h: 100 }),
        makeLayer('text', { name: 'Service timer', bind: 'elapsed', x: 4, y: 3, w: 30, h: 7, size: 2.2, color: 'theme:accent', font: 'theme:font', align: 'left', valign: 'middle' }),
        makeLayer('text', { name: 'Time remaining', bind: 'remaining', x: 37, y: 3, w: 26, h: 7, size: 2.2, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Clock', bind: 'clock', x: 66, y: 3, w: 30, h: 7, size: 2.2, color: 'theme:accent', font: 'theme:font', align: 'right', valign: 'middle' }),
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 6, y: 15, w: 88, h: 45, size: 5.4, color: 'theme:verse', font: 'theme:font', align: 'center', valign: 'middle' }),
        makeLayer('text', { name: 'Reference', bind: 'reference', x: 6, y: 61, w: 88, h: 7, size: 2.8, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle', transform: 'uppercase', letterSpacing: 0.05 }),
        makeLayer('text', { name: 'Up-next label', bind: 'static', text: 'UP NEXT', x: 6, y: 72, w: 88, h: 5, size: 1.5, color: 'theme:accent', font: 'theme:font', align: 'center', valign: 'middle', transform: 'uppercase', letterSpacing: 0.14 }),
        makeLayer('text', { name: 'Next verse', bind: 'next', x: 6, y: 77, w: 88, h: 12, size: 2.4, color: 'theme:verse', font: 'theme:font', align: 'center', valign: 'top', opacity: 0.8 }),
        makeLayer('text', { name: 'Stage Note', bind: 'note', x: 6, y: 90, w: 88, h: 8, size: 2, color: 'theme:accent', font: 'theme:font', align: 'center', valign: 'middle', italic: true }),
      ],
      align: 'center',
    },
    style: {},
  };
}

/** Countdown timer: a full-screen pre-service clock — a label, huge MM:SS, and
 *  the wall clock. The MM:SS is a timer layer bound to the fired countdown, so it
 *  ticks and shows the "begins in" label from the fired content's reference. */
function timerScreen() {
  return {
    layout: {
      layers: [
        makeLayer('background', { name: 'Screen', fill: 'theme:background', x: 0, y: 0, w: 100, h: 100 }),
        makeLayer('text', { name: 'Label', bind: 'reference', x: 8, y: 18, w: 84, h: 12, size: 3.4, color: 'theme:accent', font: 'theme:font', align: 'center', valign: 'middle', transform: 'uppercase', letterSpacing: 0.12 }),
        makeLayer('timer', { name: 'Countdown', x: 8, y: 32, w: 84, h: 48, size: 20, color: 'theme:verse', font: 'theme:font', align: 'center', valign: 'middle', shadow: 0.4 }),
        makeLayer('text', { name: 'Clock', bind: 'clock', x: 8, y: 88, w: 84, h: 8, size: 2.4, color: 'theme:reference', font: 'theme:font', align: 'center', valign: 'middle' }),
      ],
      align: 'center',
    },
    style: {},
  };
}

// A NEW TEMPLATE IS A KIND, NOT A BLANK. Starting from nothing means setting a
// dozen properties before anything renders; starting from the kind means
// adjusting two. Every starter here is a complete, working template.
//
// THE ROSTER AND THE RAIL MUST AGREE. `templateKind` derives a role from a
// template's shape and the gallery rail offers one row per role that occurs —
// so a role with no starter is a row an operator can never add to, and a starter
// whose shape derives to `custom` is a template that vanishes from its own row.
// `templateKind.test.js` holds both halves: it names the role every starter
// lands on, and `every role but Custom can be created from a starter` is what
// caught the missing lyrics one.
export const STARTERS = [
  { key: 'fullscreen', label: 'Full-Screen Scripture', make: fullScreen, hint: 'Verse centred with its reference beneath.' },
  { key: 'lyrics', label: 'Song Lyrics', make: songLyrics, hint: 'The words alone, large — no title, no reference.' },
  { key: 'lower.name', label: 'Lower Third — Name', make: lowerName, hint: 'Who is speaking, and what they are. Keyed over camera.' },
  { key: 'lower.lyric', label: 'Lower Third — Lyric', make: lowerLyric, hint: 'The words alone — no reference. Keyed over camera.' },
  { key: 'lower.bible', label: 'Lower Third — Scripture', make: lowerBible, hint: 'Verse with its reference beneath. Keyed over camera.' },
  { key: 'media', label: 'Full-Screen Media', make: mediaFull, hint: 'A picture or video fills the wall — add text over it if you like.' },
  { key: 'announcement', label: 'Announcement Ticker', make: announcement, hint: 'A scrolling crawl along the bottom.' },
  { key: 'stage', label: 'Stage Display', make: stageDisplay, hint: 'Platform monitor: current verse, reference and clock. Theme-aware.' },
  { key: 'confidence', label: 'Confidence Monitor', make: confidenceMonitor, hint: 'Booth-facing "what\'s on screen now" view with clock. Theme-aware.' },
  { key: 'preacher', label: 'Preacher View', make: preacherView, hint: 'Big centred verse, the verse coming up, the service timer and your Stage Note.' },
  { key: 'timer', label: 'Countdown Timer', make: timerScreen, hint: 'Huge MM:SS for a pre-service countdown, with a label and clock.' },
  { key: 'supersource', label: 'SuperSource', make: superSource, hint: 'Camera on one side, a rendered slide on the other. Keyed — the switcher supplies the camera.' },
  { key: 'freestyle', label: 'Freestyle', make: freestyle, hint: 'A blank canvas — add layers yourself.' },
];

// ── Legacy conversion ──────────────────────────────────────────────────────
// Turn an old region-based template into editable layers, faithfully, so an
// operator can open a preset and start moving things around. The RENDERER keeps
// its region path for un-converted templates, so this is only run on demand.
export function regionsToLayers(template) {
  const layout = template?.layout ?? {};
  // THROUGH THE MODEL, NOT AROUND IT. This read `style.font` and
  // `style.textShadow` — the whole-template keys `migrateStyle` writes onto the
  // elements and then DELETES (docs/REBRAND.md §3.1). Every template Relay seeds
  // carries a `font`, so once migration ran on the way out of the database this
  // conversion found nothing there and silently substituted the serif default:
  // `var(--f-display)` and `var(--f-body)` templates came back in the wrong
  // typeface. Not a preview, either — `TemplateGallery.upgradeLegacyToLayers`
  // runs on mount and SAVES the result, so the first visit to the Templates tab
  // after an upgrade would have re-typefaced a church's shelf, once, for good.
  //
  // `migrateStyle` is idempotent and pure, so calling it here is correct whether
  // the caller hands over a migrated template or a raw one off a disk.
  const style = migrateStyle(template?.style ?? {});
  const regions = Array.isArray(layout.regions) ? layout.regions : [];
  const band = !!layout.lowerThird;
  const refFirst = layout.refFirst || regions[0] === 'reference';
  const layers = [];

  if (band) {
    layers.push(
      makeLayer('shape', {
        name: 'Band',
        x: 4, y: 72, w: 92, h: 20,
        fill: style.accent || '#101319',
        opacity: 1,
        radius: 0.6,
      }),
    );
  } else {
    layers.push(
      makeLayer('background', {
        fill: style.background && style.background !== 'transparent' ? style.background : '#0a0a0a',
        image: style.bgImage || null,
        opacity: style.bgOpacity == null ? 1 : Number(style.bgOpacity),
        dim: Number(style.bgDim) || 0,
      }),
    );
  }

  const verseColor = style.verseColor || (band ? '#12151b' : '#f4e4c8');
  const refColor = style.refColor || (band ? style.verseColor || '#12151b' : style.accent || '#ffb000');
  const bothScripture = !band && regions.includes('verse_text') && regions.includes('reference');
  const mkVerse = (y, h) =>
    makeLayer('text', {
      name: 'Verse', bind: 'verse', x: band ? 8 : 8, y, w: band ? 84 : 84, h,
      font: style.verseFont || STYLE_DEFAULTS.verseFont, color: verseColor,
      size: Number(style.verseSize) || (band ? 2.6 : 5.2),
      align: style.verseAlign || layout.align || (band ? 'left' : 'center'), valign: 'middle',
      transform: style.verseTransform || 'none', lineHeight: Number(style.verseLineHeight) || 1.32,
      letterSpacing: Number(style.verseLetterSpacing) || 0, shadow: Number(style.verseShadow) || 0,
      italic: false, scroll: !!style.scroll,
      // The region renderer wraps a verse in “curly quotes” when a reference is
      // also shown; carry that so a converted preset reads identically.
      quote: bothScripture,
    });
  const mkRef = (y, h) =>
    makeLayer('text', {
      name: 'Reference', bind: 'reference', x: band ? 8 : 8, y, w: band ? 84 : 84, h,
      font: style.refFont || STYLE_DEFAULTS.refFont, color: refColor,
      size: Number(style.refSize) || (band ? 1.6 : 2.5),
      align: style.refAlign || layout.align || (band ? 'left' : 'center'), valign: 'middle',
      transform: style.refTransform || 'none', lineHeight: 1.2,
      letterSpacing: Number(style.refLetterSpacing) || 0, shadow: Number(style.refShadow) || 0,
      italic: !!style.italicRef, scroll: false,
    });

  const hasVerse = regions.includes('verse_text');
  const hasRef = regions.includes('reference');
  if (band) {
    if (hasVerse) layers.push(mkVerse(74, 11));
    if (hasRef) layers.push(mkRef(85, 6));
  } else if (refFirst) {
    if (hasRef) layers.push(mkRef(24, 10));
    if (hasVerse) layers.push(mkVerse(36, 52));
  } else if (hasVerse && !hasRef) {
    // Verse only (lyrics): fill most of the frame so big type stays big.
    layers.push(mkVerse(12, 76));
  } else {
    if (hasVerse) layers.push(mkVerse(20, 54));
    if (hasRef) layers.push(mkRef(76, 12));
  }

  return { ...layout, layers };
}
