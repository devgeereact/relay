// A theme must reach PIXELS, not just merge in a pure function. These mount the
// real TemplateRender with a theme and read the computed styles it applies —
// proving the `theme` prop actually paints, and that a template overrides it.
//
// The regression this guards: wiring themes as data but forgetting to feed the
// prop through, so a theme changes nothing on the wall while the editor claims
// it does.

import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';
import { BUILTIN_THEMES } from './themes.js';

let host;
let app;
function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

// A near-empty scripture template: it sets NO colours, so whatever shows must
// come from the theme.
const bareTemplate = () => ({
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: {},
});
const verseEl = (el) => el.querySelector('.verse');
const CONTENT = { reference: 'John 3:16', text: 'For God so loved the world' };

// Computed colours come back as rgb(...) — compare on that, not the hex.
const rgb = (el) => getComputedStyle(el).color;

describe('theme reaches the rendered pixels', () => {
  it('a theme colours a template that sets no verse colour', () => {
    const theme = { style: { verseColor: '#ff8800' } };
    const el = mount({ template: bareTemplate(), theme, content: CONTENT });
    expect(rgb(verseEl(el))).toBe('rgb(255, 136, 0)');
  });

  it('the template WINS over the theme, key by key', () => {
    const theme = { style: { verseColor: '#ff8800' } };
    const t = { layout: { regions: ['verse_text', 'reference'] }, style: { verseColor: '#00ff00' } };
    const el = mount({ template: t, theme, content: CONTENT });
    expect(rgb(verseEl(el))).toBe('rgb(0, 255, 0)'); // template's green, not theme's orange
  });

  it('no theme prop = renders exactly as before (renderer default)', () => {
    // Default verseColor in TemplateRender is #f4e4c8 when nothing sets it.
    const el = mount({ template: bareTemplate(), content: CONTENT });
    expect(rgb(verseEl(el))).toBe('rgb(244, 228, 200)');
  });

  it('a template pinning a BUILTIN theme (style.themeRef) renders themed with NO prop', () => {
    // This is what makes every console preview / gallery card themed for builtins
    // without any per-surface wiring: TemplateRender resolves style.themeRef itself.
    const modernDark = BUILTIN_THEMES.find((t) => t.name === 'Modern Dark'); // verseColor #ffffff
    const t = { layout: { regions: ['verse_text', 'reference'] }, style: { themeRef: modernDark.id } };
    const el = mount({ template: t, content: CONTENT });
    expect(rgb(verseEl(el))).toBe('rgb(255, 255, 255)');
  });

  it('an explicit theme prop overrides the pinned themeRef', () => {
    const t = { layout: { regions: ['verse_text', 'reference'] }, style: { themeRef: -1 } };
    const el = mount({ template: t, theme: { style: { verseColor: '#ff0000' } }, content: CONTENT });
    expect(rgb(verseEl(el))).toBe('rgb(255, 0, 0)'); // prop wins over themeRef
  });

  it('a built-in theme applies its real accent/verse colour', () => {
    const classic = BUILTIN_THEMES.find((t) => t.name === 'Classic');
    const el = mount({ template: bareTemplate(), theme: classic, content: CONTENT });
    // Classic verseColor #f4e4c8
    expect(rgb(verseEl(el))).toBe('rgb(244, 228, 200)');
  });
});

describe('layered template — a token layer follows the theme onto pixels', () => {
  const layered = (color) => ({
    layout: {
      layers: [
        { id: 'v', type: 'text', bind: 'verse', x: 5, y: 30, w: 90, h: 40, size: 5, color },
      ],
    },
    style: {},
  });
  const lfit = (el) => el.querySelector('.lfit');

  it('resolves a colour token to the theme colour on the rendered layer', () => {
    const el = mount({ template: layered('theme:accent'), theme: { style: { accent: '#0088ff' } }, content: CONTENT });
    expect(rgb(lfit(el))).toBe('rgb(0, 136, 255)');
  });

  it('with no theme, the token falls back to a literal (never renders the raw token)', () => {
    const el = mount({ template: layered('theme:verse'), content: CONTENT });
    expect(rgb(lfit(el))).toBe('rgb(244, 228, 200)'); // verse fallback #f4e4c8
  });
});
