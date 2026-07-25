// THEMES — the style layer BENEATH templates (the IA spine: "Themes are CSS,
// Templates override Theme, Output renders Template").
//
// A Theme is a named bag of DEFAULT values for the exact same flat `style` keys
// TemplateRender already reads. A Template's own `style` OVERRIDES the theme,
// per key. So the effective style a screen renders is:
//
//     { ...theme.style, ...template.style }     // template wins, key by key
//
// That is the whole model. It is deliberately NOT a new renderer, a new content
// type, or an `if theme == …` branch anywhere — resolving a theme produces a
// normal `template` object that the ONE renderer (TemplateRender) draws exactly
// as it draws any other. A themed template and a hand-styled template are
// indistinguishable downstream, which is what keeps WYSIWYG and the "outputs are
// render targets of one engine" rule intact.
//
// Builtins live here (like templates.js's BUILTINS) so a kiosk/OBS client with
// no DB can still resolve a theme id. Custom themes are persisted by the desktop
// app as a JSON blob in the settings KV (see capture.js: loadThemes/saveTheme).

/**
 * The style keys a THEME is allowed to own. Everything else in a template's
 * style (per-region overrides, background image, panel, etc.) is a template
 * concern and is never set by a theme. Keeping this list explicit means a theme
 * can never smuggle in a key that silently changes an unrelated template.
 */
export const THEME_STYLE_KEYS = [
  // typography
  'font',
  'verseFont',
  'refFont',
  'verseSize',
  'refSize',
  'verseLineHeight',
  'verseLetterSpacing',
  'refLetterSpacing',
  'italicRef',
  // colour
  'accent',
  'verseColor',
  'refColor',
  'background',
  // effect + rhythm
  'verseShadow',
  'refShadow',
  'transitionMs',
  'transition',
  'refGap',
];

// Built-in themes, in seed order. A theme is intentionally a SPARSE set of
// defaults — it colours and sets type, and leaves layout/behaviour to the
// template. Ids are negative so they can never collide with a saved custom
// theme's positive id (custom ids come from Date-free incrementing, see
// capture.js), and a template's `style.themeRef` is unambiguous.
export const BUILTIN_THEMES = [
  {
    id: -1,
    name: 'Modern Dark',
    builtin: true,
    style: {
      font: 'var(--f-display)',
      accent: '#22d3ee',
      verseColor: '#ffffff',
      refColor: '#22d3ee',
      background: 'radial-gradient(120% 140% at 50% 30%, #16181d, #06070a)',
      verseSize: '6',
      refSize: '2.6',
      verseLineHeight: '1.3',
      refGap: '1.4',
      verseShadow: '0.35',
      transitionMs: '250',
      italicRef: false,
    },
  },
  {
    id: -2,
    name: 'Minimal',
    builtin: true,
    style: {
      font: 'var(--f-body)',
      accent: '#8a8a8a',
      verseColor: '#f2f2f2',
      refColor: '#b3b3b3',
      background: '#0b0b0d',
      verseSize: '5.4',
      refSize: '2.2',
      verseLineHeight: '1.36',
      refGap: '1.2',
      verseShadow: '0',
      transitionMs: '200',
      italicRef: false,
    },
  },
  {
    id: -3,
    name: 'Light',
    builtin: true,
    style: {
      font: 'var(--f-body)',
      accent: '#b4531f',
      verseColor: '#161310',
      refColor: '#b4531f',
      background: 'linear-gradient(180deg, #f6f1e8, #e9e1d2)',
      verseSize: '5.4',
      refSize: '2.4',
      verseLineHeight: '1.34',
      refGap: '1.3',
      verseShadow: '0',
      transitionMs: '250',
      italicRef: true,
    },
  },
  {
    id: -4,
    name: 'Classic',
    builtin: true,
    style: {
      font: 'var(--f-serif)',
      accent: '#ffb000',
      verseColor: '#f4e4c8',
      refColor: '#ffb000',
      background: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)',
      verseSize: '5.5',
      refSize: '2.6',
      verseLineHeight: '1.32',
      refGap: '1.4',
      verseShadow: '0.4',
      transitionMs: '300',
      italicRef: true,
    },
  },
  {
    id: -5,
    name: 'Youth',
    builtin: true,
    style: {
      font: 'var(--f-display)',
      accent: '#a855f7',
      verseColor: '#ffffff',
      refColor: '#f0abfc',
      background: 'linear-gradient(150deg, #1e1033, #2a0f45 55%, #0c0713)',
      verseSize: '6.4',
      refSize: '2.8',
      verseLineHeight: '1.26',
      refGap: '1.5',
      verseShadow: '0.5',
      transitionMs: '220',
      italicRef: false,
    },
  },
  {
    id: -6,
    name: 'Conference',
    builtin: true,
    style: {
      font: 'var(--f-head)',
      accent: '#38bdf8',
      verseColor: '#eef4f8',
      refColor: '#38bdf8',
      background: 'linear-gradient(160deg, #0f1c2b, #091320)',
      verseSize: '5.8',
      refSize: '2.5',
      verseLineHeight: '1.3',
      refGap: '1.4',
      verseShadow: '0.3',
      transitionMs: '250',
      italicRef: false,
    },
  },
  {
    id: -7,
    name: 'Wedding',
    builtin: true,
    style: {
      font: 'var(--f-serif)',
      accent: '#c99a6a',
      verseColor: '#f6ede2',
      refColor: '#c99a6a',
      background: 'linear-gradient(180deg, #241a1d, #140d10)',
      verseSize: '5.2',
      refSize: '2.4',
      verseLineHeight: '1.4',
      refGap: '1.6',
      verseShadow: '0.25',
      transitionMs: '320',
      italicRef: true,
    },
  },
  {
    id: -8,
    name: 'Livestream',
    builtin: true,
    style: {
      font: 'var(--f-display)',
      accent: '#f43f5e',
      verseColor: '#ffffff',
      refColor: '#f43f5e',
      background: '#000000',
      verseSize: '6',
      refSize: '2.6',
      verseLineHeight: '1.28',
      refGap: '1.3',
      verseShadow: '0.6',
      transitionMs: '200',
      italicRef: false,
    },
  },
];

export const DEFAULT_THEME = BUILTIN_THEMES[0];

/** Resolve a theme id (builtin or custom) against a list. Falls back to the
 *  default theme so a dangling `themeRef` never blanks a render. */
export function themeById(id, custom = []) {
  if (id == null) return null;
  return (
    BUILTIN_THEMES.find((t) => t.id === id) ||
    custom.find((t) => t && t.id === id) ||
    null
  );
}

// ── LAYER THEME TOKENS ───────────────────────────────────────────────────────
// A layer's colour / fill / font can be a LITERAL (`#ffb000`, `var(--f-serif)`)
// or a THEME TOKEN (`theme:accent`) that resolves from the applied theme. Tokens
// are what make a LAYERED template follow its theme — recolour the theme and
// every layer bound to a token moves with it, no per-layer editing.
//
// A token ALWAYS resolves to a literal, even with no theme applied, so a token
// can never emit invalid CSS onto the wall: absent a theme it falls back to the
// renderer's own defaults. Template style still wins — the effective style a
// token resolves against is {theme, …templateStyle}, template on top.

/** The bindable tokens, for the editor's "link to theme" pickers. */
export const THEME_TOKENS = [
  { token: 'theme:verse', label: 'Verse colour' },
  { token: 'theme:reference', label: 'Reference colour' },
  { token: 'theme:accent', label: 'Accent' },
  { token: 'theme:background', label: 'Background' },
  { token: 'theme:font', label: 'Theme typeface' },
];

const TOKEN_RESOLVERS = {
  'theme:verse': (s) => s.verseColor ?? '#f4e4c8',
  'theme:reference': (s) => s.refColor ?? s.accent ?? '#ffb000',
  'theme:accent': (s) => s.accent ?? '#ffb000',
  'theme:background': (s) => s.background ?? 'transparent',
  'theme:font': (s) => s.font ?? 'var(--f-serif)',
};

/** Is `v` a recognised theme token? */
export function isThemeToken(v) {
  return typeof v === 'string' && Object.prototype.hasOwnProperty.call(TOKEN_RESOLVERS, v);
}
function resolveTok(v, s) {
  return isThemeToken(v) ? TOKEN_RESOLVERS[v](s) : v;
}
function layerHasToken(L) {
  return isThemeToken(L?.color) || isThemeToken(L?.fill) || isThemeToken(L?.font);
}

/**
 * Merge a theme's defaults under a template's style, AND resolve any theme
 * tokens on the template's layers. Template keys WIN, so a template that sets
 * `verseColor` keeps it and only inherits the keys it leaves unset. Returns a
 * NEW template object (never mutates), safe to hand straight to TemplateRender.
 *
 * A null/garbage theme yields the template UNCHANGED except that layer tokens
 * still resolve (to their literal fallbacks) — a bad theme must degrade to a
 * sane look, never take the wall down (same law as parseTemplateOverride).
 *
 * Fast path: a template with no theme-style to merge AND no tokenised layer is
 * returned essentially as-is, so a literal template renders byte-for-byte the
 * same whether or not a theme is present.
 */
export function applyTheme(template, theme) {
  const base = template && typeof template === 'object' ? template : {};
  const themeStyle = {};
  if (theme && typeof theme === 'object' && theme.style) {
    for (const k of THEME_STYLE_KEYS) {
      if (theme.style[k] !== undefined) themeStyle[k] = theme.style[k];
    }
  }
  const hasThemeStyle = Object.keys(themeStyle).length > 0;
  const layers = Array.isArray(base.layout?.layers) ? base.layout.layers : null;
  const needLayers = !!layers && layers.some(layerHasToken);
  if (!hasThemeStyle && !needLayers) return { ...base };

  const out = { ...base };
  const effective = { ...themeStyle, ...(base.style ?? {}) };
  if (hasThemeStyle) out.style = effective;
  if (needLayers) {
    out.layout = {
      ...base.layout,
      layers: layers.map((L) => {
        if (!layerHasToken(L)) return L;
        const nL = { ...L };
        if (isThemeToken(L.color)) nL.color = resolveTok(L.color, effective);
        if (isThemeToken(L.fill)) nL.fill = resolveTok(L.fill, effective);
        if (isThemeToken(L.font)) nL.font = resolveTok(L.font, effective);
        return nL;
      }),
    };
  }
  return out;
}

/**
 * The theme a template inherits, if any. Stored on the template as
 * `style.themeRef` (a reserved key TemplateRender ignores) so linking a template
 * to a theme needs NO backend schema change — the template's style blob already
 * round-trips as free JSON. Returns null when the template pins no theme.
 */
export function templateThemeRef(template) {
  const ref = template?.style?.themeRef;
  return typeof ref === 'number' ? ref : null;
}

/** Resolve + apply a template's pinned theme (`style.themeRef`) in one step.
 *  No ref, or an unknown one, returns the template essentially unchanged. */
export function resolveThemed(template, custom = []) {
  const theme = themeById(templateThemeRef(template), custom);
  return applyTheme(template, theme);
}

/**
 * APPLY a theme to a template so it visibly FOLLOWS the theme: pin the theme
 * (`style.themeRef`) AND re-bind the template's layer colours/fonts to theme
 * TOKENS by role — verse text → `theme:verse`, a reference → `theme:reference`, a
 * background → `theme:background`, everything else → `theme:accent`, and every text
 * layer's font → `theme:font`. Returns a NEW template (never mutates).
 *
 * This is the step that makes themes worth having: a layered template built with
 * literal colours ignores a theme (a theme only fills keys the template left
 * unset). Applying re-tokenises those layers, so from then on recolouring the
 * theme moves the whole template. Structural layers (shapes, media) are left
 * alone — their role is the template's, not the theme's.
 */
export function applyThemeToTemplate(template, theme) {
  if (!template || typeof template !== 'object' || !theme) return template;
  const tokenFor = (L) => {
    if (L.bind === 'verse' || L.bind === 'next') return 'theme:verse';
    if (L.bind === 'reference' || L.bind === 'next_reference') return 'theme:reference';
    return 'theme:accent';
  };
  const out = { ...template, style: { ...(template.style ?? {}), themeRef: theme.id } };
  if (Array.isArray(template.layout?.layers)) {
    out.layout = {
      ...template.layout,
      layers: template.layout.layers.map((L) => {
        if (L.type === 'background') return { ...L, fill: 'theme:background' };
        if (L.type === 'text' || L.type === 'timer') return { ...L, color: tokenFor(L), font: 'theme:font' };
        return L; // shapes, media — structural, left to the template
      }),
    };
  }
  return out;
}

/**
 * Safe-parse a persisted themes blob (a JSON array). Anything that isn't a
 * well-formed array of objects with a numeric id yields [] — custom themes are a
 * convenience layer, and a corrupt blob must fall back to builtins-only, never
 * throw on boot.
 */
export function parseThemes(json) {
  if (!json) return [];
  try {
    const parsed = JSON.parse(json);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (t) => t && typeof t === 'object' && typeof t.id === 'number' && typeof t.style === 'object',
    );
  } catch {
    return [];
  }
}

// ── EXPORT / IMPORT ──────────────────────────────────────────────────────────
// A theme is portable: one church can hand another its look as a small JSON file.
// Only `name` + `style` travel — never an `id` or `builtin` flag, so importing
// always CREATES a fresh custom theme and can never overwrite a builtin or a
// theme the recipient already has. The pure serialize/parse pair lives here so it
// is tested; the file download / picker is trivial glue in the store.

/** The marker written into an exported theme file, so an import can tell a Relay
 *  theme from an arbitrary JSON blob and refuse the latter with a clear message. */
export const THEME_FILE_MARKER = 'relay.theme/v1';

/** Serialise a theme to the exported file's JSON text (pretty-printed). Strips
 *  id/builtin — an export is a look, not an identity. */
export function serializeTheme(theme) {
  return JSON.stringify(
    { marker: THEME_FILE_MARKER, name: String(theme?.name ?? 'Theme'), style: theme?.style ?? {} },
    null,
    2,
  );
}

/**
 * Parse an exported theme file back into `{ name, style }`, or throw a plain-
 * language Error the caller shows through the ONE humaniser. Rejects anything
 * that isn't a Relay theme file (wrong/absent marker, no style object) rather
 * than importing junk that would render as a blank look. Never returns an id or
 * builtin flag — the caller saves it as a NEW custom theme.
 */
export function parseImportedTheme(text) {
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error("That file isn't valid JSON — it may be corrupt or not a theme file.");
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error("That file isn't a Relay theme.");
  }
  if (parsed.marker !== THEME_FILE_MARKER) {
    throw new Error("That file isn't a Relay theme file (wrong or missing marker).");
  }
  if (!parsed.style || typeof parsed.style !== 'object' || Array.isArray(parsed.style)) {
    throw new Error('That theme file has no style to import.');
  }
  return { name: String(parsed.name ?? 'Imported theme'), style: parsed.style };
}

/** A near-empty scripture template used as the CANVAS for theme previews: it
 *  sets only layout, so whatever a preview shows comes from the THEME, not a
 *  competing template style. This is why a theme card looks like the theme. */
export const THEME_PREVIEW_TEMPLATE = {
  name: 'Theme preview',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
  style: {},
};

/** The sample scripture every theme card/preview renders, matching the template
 *  gallery's sample so the two galleries read as one system. */
export const THEME_SAMPLE_CONTENT = {
  reference: 'Psalms 23:1 · KJV',
  text: 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures.',
  translation: 'KJV',
};
