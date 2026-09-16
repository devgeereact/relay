/**
 * THE STYLE LAYER THAT OUTLIVED THEMES.
 *
 * Themes were folded into templates in wave 2 (DECISIONS §27 completed): a
 * theme's every field was already a template `style` key, so a layer beneath
 * templates whose only content is a subset of the template's own keys had
 * finished being useful.
 *
 * What is here was never about themes. A layer's colour, fill or font may be a
 * TOKEN (`theme:accent`) rather than a literal, so that a stage or confidence
 * starter follows whatever template it is dropped into. The token name keeps its
 * spelling: it is written into every saved layer in every install, and renaming
 * it would need a migration to buy a nicer word.
 */

import { resolveStyle } from './templatemodel.js';

// ── LAYER STYLE TOKENS ───────────────────────────────────────────────────────
// A layer's colour / fill / font can be a LITERAL (`#ffb000`, `var(--f-serif)`)
// or a TOKEN (`theme:accent`) that resolves from the template's own style. Tokens
// are what make a LAYERED template follow the template it belongs to — recolour
// the template and every layer bound to a token moves with it, no per-layer
// editing.
//
// A token ALWAYS resolves to a literal, so a token can never emit invalid CSS
// onto the wall: a style that sets none of these falls back to the renderer's own
// defaults.

/** The bindable tokens, for the editor's "theme link" pickers. */
export const THEME_TOKENS = [
  { token: 'theme:verse', label: 'Verse colour' },
  { token: 'theme:reference', label: 'Reference colour' },
  { token: 'theme:accent', label: 'Accent' },
  { token: 'theme:background', label: 'Background' },
  { token: 'theme:font', label: 'Template typeface' },
];

const TOKEN_RESOLVERS = {
  'theme:verse': (s) => s.verseColor ?? '#f4e4c8',
  'theme:reference': (s) => s.refColor ?? s.accent ?? '#ffb000',
  'theme:accent': (s) => s.accent ?? '#ffb000',
  'theme:background': (s) => s.background ?? 'transparent',
  // THROUGH THE MODEL. This read `s.font`, the whole-template key `migrateStyle`
  // moves onto the elements and deletes (docs/REBRAND.md §3.1) — so on a migrated
  // template, every layer bound to `theme:font` (which is every text layer in the
  // stage, confidence and countdown starters) resolved to the serif default
  // instead of the typeface the template actually carries. `resolveStyle`
  // migrates on the way through and owns the default, so there is one home for
  // the answer whether the style reaching it is old or new.
  'theme:font': (s) => resolveStyle(s).verseFont,
};

/** Is `v` a recognised style token? */
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
 * Resolve any style tokens on a template's layers against that template's OWN
 * style. Returns a NEW template object (never mutates), safe to hand straight to
 * TemplateRender.
 *
 * This is `applyTheme` with the theme argument removed. The merge it performed
 * was `{ ...themeStyle, ...template.style }` — template keys winning, key by key
 * — so with no theme left to merge, the style a token resolves against is simply
 * the template's own. Nothing about a token's answer changed with the themes;
 * what changed is that there is now one place the answer can come from.
 *
 * Fast path: a template with no tokenised layer is returned essentially as-is,
 * so a literal template renders byte-for-byte the same as before.
 */
export function resolveTokens(template) {
  const base = template && typeof template === 'object' ? template : {};
  const layers = Array.isArray(base.layout?.layers) ? base.layout.layers : null;
  if (!layers || !layers.some(layerHasToken)) return { ...base };

  const style = base.style ?? {};
  return {
    ...base,
    layout: {
      ...base.layout,
      layers: layers.map((L) => {
        if (!layerHasToken(L)) return L;
        const nL = { ...L };
        if (isThemeToken(L.color)) nL.color = resolveTok(L.color, style);
        if (isThemeToken(L.fill)) nL.fill = resolveTok(L.fill, style);
        if (isThemeToken(L.font)) nL.font = resolveTok(L.font, style);
        return nL;
      }),
    },
  };
}
