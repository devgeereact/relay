// The template STYLE properties the editor exposes must actually render — a
// control that changes nothing is a lie the operator eventually trusts. These
// mount the real TemplateRender and read the computed styles it applies.

import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';

let host;
let app;
function mount(template, content) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

const scripture = (style = {}) => ({
  id: 1,
  name: 'T',
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: { verseColor: '#ffffff', accent: '#ffb000', background: '#101010', ...style },
});
const verseEl = (el) => el.querySelector('.verse');
const refEl = (el) => el.querySelector('.reference');
const bgEl = (el) => el.querySelector('.bglayer');
const CONTENT = { reference: 'John 3:16', text: 'For God so loved the world' };

describe('background opacity', () => {
  it('dims the background layer, not the text', () => {
    const el = mount(scripture({ bgOpacity: 0.4 }), CONTENT);
    expect(getComputedStyle(bgEl(el)).opacity).toBe('0.4');
    // The text sits on its own layer and is unaffected.
    expect(getComputedStyle(verseEl(el)).opacity).toBe('1');
  });
  it('defaults to fully opaque when unset', () => {
    const el = mount(scripture(), CONTENT);
    expect(getComputedStyle(bgEl(el)).opacity).toBe('1');
  });
});

describe('capitalization', () => {
  it('applies text-transform to the verse and reference independently', () => {
    const el = mount(scripture({ verseTransform: 'uppercase', refTransform: 'lowercase' }), CONTENT);
    expect(getComputedStyle(verseEl(el)).textTransform).toBe('uppercase');
    expect(getComputedStyle(refEl(el)).textTransform).toBe('lowercase');
  });
});

describe('line height and letter spacing', () => {
  it('sets the verse line-height', () => {
    const el = mount(scripture({ verseLineHeight: 1.7 }), CONTENT);
    // jsdom returns the raw number for unitless line-height.
    expect(getComputedStyle(verseEl(el)).lineHeight).toMatch(/1\.7/);
  });
  it('sets letter-spacing in em', () => {
    const el = mount(scripture({ verseLetterSpacing: 0.1 }), CONTENT);
    expect(getComputedStyle(verseEl(el)).letterSpacing).toBe('0.1em');
  });
});

describe('text shadow', () => {
  it('is off by default and on when a strength is set', () => {
    const off = mount(scripture(), CONTENT);
    // jsdom renders `text-shadow:none` as a fully-transparent shadow.
    expect(getComputedStyle(verseEl(off)).textShadow).toMatch(/none|rgba\(0, 0, 0, 0\)/);
    app.$destroy(); host.remove();
    const on = mount(scripture({ textShadow: 0.8 }), CONTENT);
    // A real shadow carries a visible (non-zero-alpha) black.
    expect(getComputedStyle(verseEl(on)).textShadow).toMatch(/rgba\(0,\s?0,\s?0,\s?0\.\d/);
  });

  it('is per region — the verse and reference shadows are independent', () => {
    // verseShadow on, refShadow off: only the verse gets one.
    const el = mount(scripture({ verseShadow: 0.8, refShadow: 0 }), CONTENT);
    expect(getComputedStyle(verseEl(el)).textShadow).toMatch(/rgba\(0,\s?0,\s?0,\s?0\.\d/);
    expect(getComputedStyle(refEl(el)).textShadow).toMatch(/none|rgba\(0, 0, 0, 0\)/);
  });
});

describe('per-region font', () => {
  it('lets the verse and reference each carry their own font', () => {
    const el = mount(scripture({ verseFont: 'Georgia', refFont: 'Arial' }), CONTENT);
    expect(getComputedStyle(verseEl(el)).fontFamily).toMatch(/Georgia/);
    expect(getComputedStyle(refEl(el)).fontFamily).toMatch(/Arial/);
  });
  it('falls back to the base font when a region font is unset', () => {
    const el = mount(scripture({ font: 'Palatino' }), CONTENT);
    expect(getComputedStyle(verseEl(el)).fontFamily).toMatch(/Palatino/);
    expect(getComputedStyle(refEl(el)).fontFamily).toMatch(/Palatino/);
  });
});

describe('reference gap', () => {
  it('sets the space between the verse and reference', () => {
    const el = mount(scripture({ refGap: 5 }), CONTENT);
    // cqw margin — jsdom keeps the raw declaration.
    expect(refEl(el).getAttribute('style')).toMatch(/margin-top:5cqw/);
  });
});

describe('bright-background readability tools', () => {
  it('renders a dim scrim only when bgDim is set', () => {
    const none = mount(scripture(), CONTENT);
    expect(none.querySelector('.dimlayer')).toBeNull();
    app.$destroy(); host.remove();
    const dim = mount(scripture({ bgDim: 0.5 }), CONTENT);
    expect(getComputedStyle(dim.querySelector('.dimlayer')).opacity).toBe('0.5');
  });

  it('draws a text panel behind the words with a tweakable colour + opacity', () => {
    const el = mount(scripture({ textPanel: true, panelColor: '#000000', panelOpacity: 0.6 }), CONTENT);
    const c = el.querySelector('.content.panel');
    expect(c).toBeTruthy();
    expect(getComputedStyle(c).background).toMatch(/rgba\(0,\s?0,\s?0,\s?0\.6/);
  });

  it('has no panel by default', () => {
    const el = mount(scripture(), CONTENT);
    expect(el.querySelector('.content.panel')).toBeNull();
  });
});

describe('per-region text colour', () => {
  it('uses an explicit refColor when set, overriding the accent', () => {
    const el = mount(scripture({ refColor: '#00ff00', accent: '#ffb000' }), CONTENT);
    expect(getComputedStyle(refEl(el)).color).toBe('rgb(0, 255, 0)');
  });
  it('falls back to the accent for the reference when no refColor', () => {
    const el = mount(scripture({ accent: '#ff0000' }), CONTENT);
    expect(getComputedStyle(refEl(el)).color).toBe('rgb(255, 0, 0)');
  });
});

describe('announcement scroll (footer ticker)', () => {
  it('renders a bottom footer ticker with a scrolling run when enabled', () => {
    const el = mount(scripture({ scroll: true }), { reference: 'NOTICE', text: 'Midweek service at 7pm' });
    expect(el.querySelector('.ticker')).toBeTruthy();
    expect(el.querySelector('.ticker .ticker-run')).toBeTruthy();
    // No centred .content verse in ticker mode.
    expect(el.querySelector('.content .verse')).toBeNull();
  });
  it('renders a normal centred verse when scroll is off', () => {
    const el = mount(scripture(), CONTENT);
    expect(el.querySelector('.ticker')).toBeNull();
    expect(el.querySelector('.content .verse')).toBeTruthy();
  });
  it('drops the ticker label when the reference region is turned off', () => {
    // The bug: the ticker showed the reference even with "Show reference" off.
    const el = mount(
      { id: 1, name: 'T', layout: { regions: ['verse_text'], align: 'center' }, style: { scroll: true, verseColor: '#fff', accent: '#f00', background: '#101010' } },
      { reference: 'NOTICE', text: 'Midweek service' },
    );
    expect(el.querySelector('.ticker')).toBeTruthy();
    expect(el.querySelector('.ticker-label')).toBeNull();
  });
});

describe('font fallback', () => {
  it('appends a generic fallback to a bare family name', () => {
    const el = mount(scripture({ font: 'Didot' }), CONTENT);
    const ff = getComputedStyle(el.querySelector('.content')).fontFamily;
    // An uninstalled "Didot" degrades to the computer default, not something random.
    expect(ff).toMatch(/Didot/);
    expect(ff).toMatch(/system-ui|sans-serif/);
  });
  it('leaves a CSS variable font untouched (it carries its own fallback)', () => {
    const el = mount(scripture({ font: 'var(--f-serif)' }), CONTENT);
    const ff = getComputedStyle(el.querySelector('.content')).fontFamily;
    expect(ff).toMatch(/--f-serif/);
  });
});
