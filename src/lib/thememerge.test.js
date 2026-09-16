import { describe, it, expect } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolveTokens } from './styletokens.js';
import { MOVED_TABS } from './session.js';

// ONE MODEL, NOT TWO.
//
// A theme had no field a template does not have: THEME_STYLE_KEYS was a subset
// of the same flat `style` keys, there was no themes table and no Rust struct.
// What it did have was a second gallery, a second editor, a second store, a
// second export format and a hub frame of its own — five surfaces that could
// disagree with the template model about what a screen wears.
//
// A LAYER COLOUR BOUND TO A TOKEN STILL RESOLVES. That is the half of themes.js
// that was never about themes: `theme:accent` on a layer resolved against the
// merged style, and with no theme in the merge that is the template's own style.
// Deleting the resolver with the desk would have silently blanked every layer in
// the stage, confidence and countdown starters.
describe('themes are folded into templates', () => {
  it('leaves no theme desk behind', () => {
    expect(existsSync('src/lib/views/themes/ThemeGallery.svelte')).toBe(false);
    expect(existsSync('src/lib/views/themes/ThemeEditor.svelte')).toBe(false);
  });

  it('still sends a stored session naming the old desk somewhere valid', () => {
    // The desk is gone; a laptop that was left on it is not. Without this the
    // operator lands nowhere and concludes the whole workspace was deleted.
    expect(MOVED_TABS.themes).toBe('templates');
  });

  it('resolves a layer token against the template own style', () => {
    const t = {
      style: { accent: '#ffb000', verseColor: '#eee' },
      layout: { layers: [{ id: 'a', type: 'text', color: 'theme:accent', fill: 'theme:background' }] },
    };
    const out = resolveTokens(t);
    expect(out.layout.layers[0].color).toBe('#ffb000');
    expect(out.layout.layers[0].fill).toBe('transparent');
  });

  it('and reads the template, rather than answering from the fallbacks', () => {
    // THE CASE ABOVE CANNOT TELL THE TWO APART, and it was written that way in
    // the plan. `#ffb000` IS the `theme:accent` fallback and `transparent` IS the
    // `theme:background` fallback, so a resolver that ignored the template
    // entirely and answered from its own defaults passes it — verified by
    // replacing the style it reads with `{}` and watching all four assertions
    // stay green. That is the exact defect this task could have shipped: the
    // desk deleted, the resolver kept, and every layer in the stage, confidence
    // and countdown starters quietly painting the default instead of the
    // template's colours. So the values here are ones no fallback produces.
    const out = resolveTokens({
      style: { accent: '#00ff88', background: '#112233', refColor: '#abcdef', verseColor: '#fedcba' },
      layout: {
        layers: [
          { id: 'a', type: 'text', color: 'theme:accent' },
          { id: 'b', type: 'background', fill: 'theme:background' },
          { id: 'c', type: 'text', color: 'theme:reference' },
          { id: 'd', type: 'text', color: 'theme:verse' },
        ],
      },
    });
    expect(out.layout.layers.map((L) => L.color ?? L.fill)).toEqual([
      '#00ff88',
      '#112233',
      '#abcdef',
      '#fedcba',
    ]);
  });

  it('leaves no themeRef resolution in the render path', () => {
    const render = readFileSync('src/lib/TemplateRender.svelte', 'utf8');
    const output = readFileSync('src/Output.svelte', 'utf8');
    expect(render).not.toMatch(/themeRef/);
    expect(output).not.toMatch(/themeRef|resolveThemed/);
  });

  it('leaves no themes frame on the hub', () => {
    const ch = readFileSync('src-tauri/src/channels.rs', 'utf8');
    expect(ch).not.toMatch(/fn set_themes|"kind":"themes"/);
  });
});
