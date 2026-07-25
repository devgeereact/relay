import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';
import { makeLayer, isLayered, boundValue, regionsToLayers, STARTERS, formatElapsed, formatRemaining, slideRevealCss } from './layers.js';

describe('layer model', () => {
  it('makes typed layers with sane defaults and unique ids', () => {
    const a = makeLayer('text');
    const b = makeLayer('text');
    expect(a.type).toBe('text');
    expect(a.id).not.toBe(b.id);
    expect(makeLayer('background').w).toBe(100);
    expect(makeLayer('shape').type).toBe('shape');
    // A timer is a text layer bound to the countdown.
    expect(makeLayer('timer').type).toBe('text');
    expect(makeLayer('timer').bind).toBe('countdown');
  });

  it('binds text to the fired content', () => {
    const content = { text: 'For God so loved', reference: 'John 3:16', translation: 'KJV' };
    expect(boundValue({ bind: 'verse' }, content)).toBe('For God so loved');
    expect(boundValue({ bind: 'reference' }, content)).toBe('John 3:16');
    expect(boundValue({ bind: 'translation' }, content)).toBe('KJV');
    expect(boundValue({ bind: 'static', text: 'HELLO' }, content)).toBe('HELLO');
  });

  it('isLayered only when layers actually exist', () => {
    expect(isLayered({ layout: { layers: [makeLayer('text')] } })).toBe(true);
    expect(isLayered({ layout: { layers: [] } })).toBe(false);
    expect(isLayered({ layout: { regions: ['verse_text'] } })).toBe(false);
    expect(isLayered({})).toBe(false);
  });

  it('every starter produces a valid, non-empty layer stack', () => {
    for (const s of STARTERS) {
      const t = s.make();
      expect(Array.isArray(t.layout.layers)).toBe(true);
      expect(t.layout.layers.length).toBeGreaterThan(0);
      // Every layer has geometry and a type.
      for (const L of t.layout.layers) {
        expect(['text', 'media', 'shape', 'background']).toContain(L.type);
        expect(typeof L.x).toBe('number');
      }
    }
  });

  it('ships stage + confidence role profiles, theme-bound and using real bindings', () => {
    for (const key of ['stage', 'confidence']) {
      const s = STARTERS.find((x) => x.key === key);
      expect(s, `starter ${key} exists`).toBeTruthy();
      const t = s.make();
      const layers = t.layout.layers;
      // Shows the current verse and its reference, and a clock — a role monitor.
      const verse = layers.find((L) => L.bind === 'verse');
      const ref = layers.find((L) => L.bind === 'reference');
      expect(verse).toBeTruthy();
      expect(ref).toBeTruthy();
      expect(layers.some((L) => L.bind === 'clock')).toBe(true);
      // Colours bind to theme tokens, so the monitor follows its theme.
      expect(verse.color).toBe('theme:verse');
      expect(ref.color).toBe('theme:reference');
      // Opaque background (a monitor, not a keyed camera overlay).
      expect(layers.some((L) => L.type === 'background')).toBe(true);
      // A service elapsed timer — a defining stage/confidence field.
      expect(layers.some((L) => L.bind === 'elapsed')).toBe(true);
    }
  });

  it('ships a preacher view with verse, next, timer and note; and a countdown timer', () => {
    const preacher = STARTERS.find((s) => s.key === 'preacher').make().layout.layers;
    for (const bind of ['verse', 'reference', 'next', 'elapsed', 'clock', 'note']) {
      expect(preacher.some((L) => L.bind === bind), `preacher has a ${bind} layer`).toBe(true);
    }

    const timer = STARTERS.find((s) => s.key === 'timer').make().layout.layers;
    // The huge digits are a countdown-bound layer (makeLayer('timer') → text/countdown).
    const cd = timer.find((L) => L.bind === 'countdown');
    expect(cd).toBeTruthy();
    expect(cd.type).toBe('text'); // valid layer type for the renderer
    expect(cd.size).toBeGreaterThan(10); // genuinely large
    expect(cd.color).toBe('theme:verse'); // theme-aware
  });

  it('formatElapsed renders a service timer, clamping junk to 0:00', () => {
    expect(formatElapsed(0)).toBe('0:00');
    expect(formatElapsed(5_000)).toBe('0:05');
    expect(formatElapsed(65_000)).toBe('1:05');
    expect(formatElapsed(59 * 60_000 + 59_000)).toBe('59:59');
    expect(formatElapsed(60 * 60_000)).toBe('1:00:00'); // rolls to H:MM:SS at an hour
    expect(formatElapsed(3 * 3600_000 + 4 * 60_000 + 9_000)).toBe('3:04:09');
    expect(formatElapsed(-500)).toBe('0:00');
    expect(formatElapsed('nonsense')).toBe('0:00');
  });

  it('formatRemaining shows time left, and goes negative when over', () => {
    expect(formatRemaining(90_000)).toBe('1:30'); // 1:30 left
    expect(formatRemaining(0)).toBe('0:00');
    expect(formatRemaining(-90_000)).toBe('-1:30'); // 1:30 over time
    expect(formatRemaining(-1000)).toBe('-0:01');
    expect(formatRemaining(3600_000)).toBe('1:00:00');
  });

  it('slideRevealCss ramps opacity for every mode, adds transform only for slide/zoom', () => {
    // Fade: opacity only, no transform.
    expect(slideRevealCss('fade', 0.5)).toBe('opacity:0.5;');
    expect(slideRevealCss('fade', 1)).not.toContain('transform');
    // Slide rises: at t=0 it is offset, at t=1 it lands.
    expect(slideRevealCss('slide', 0)).toContain('translateY(4cqh)');
    expect(slideRevealCss('slide', 1)).toContain('translateY(0cqh)');
    // Zoom scales up into place.
    expect(slideRevealCss('zoom', 0)).toContain('scale(0.92)');
    expect(slideRevealCss('zoom', 1)).toContain('scale(1)');
    // Every mode cross-fades (opacity tracks t).
    for (const m of ['fade', 'slide', 'zoom']) expect(slideRevealCss(m, 0.3)).toContain('opacity:0.3');
  });

  it('the preacher view carries a remaining-time layer', () => {
    const preacher = STARTERS.find((s) => s.key === 'preacher').make().layout.layers;
    expect(preacher.some((L) => L.bind === 'remaining')).toBe(true);
  });

  it('converts a region template to layers faithfully', () => {
    // A lower-third region template → a band shape + text layers, no full bg.
    const band = regionsToLayers({
      layout: { regions: ['verse_text', 'reference'], lowerThird: true, align: 'center' },
      style: { accent: '#f2f2f2', verseColor: '#111' },
    });
    expect(band.layers.some((l) => l.type === 'shape')).toBe(true);
    expect(band.layers.some((l) => l.type === 'background')).toBe(false);
    expect(band.layers.filter((l) => l.type === 'text').length).toBe(2);

    // A full-screen scripture template → a background + verse + reference.
    const full = regionsToLayers({
      layout: { regions: ['verse_text', 'reference'], align: 'center' },
      style: { background: '#101010', verseColor: '#fff', accent: '#f80' },
    });
    expect(full.layers.some((l) => l.type === 'background')).toBe(true);
    expect(full.layers.filter((l) => l.type === 'text').length).toBe(2);
  });
});

// ── Layer rendering ──────────────────────────────────────────────────────────
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

describe('layer rendering', () => {
  const CONTENT = { reference: 'John 3:16', text: 'For God so loved the world', translation: 'KJV' };

  it('draws a background, a shape and bound text as positioned layers', () => {
    const t = {
      id: 1,
      name: 'L',
      layout: {
        layers: [
          makeLayer('background', { fill: '#101010' }),
          makeLayer('shape', { x: 5, y: 70, w: 90, h: 20, fill: '#000000', opacity: 0.8 }),
          makeLayer('text', { bind: 'verse', x: 8, y: 30, w: 84, h: 40, color: '#ffffff' }),
          makeLayer('text', { bind: 'reference', x: 8, y: 72, w: 84, h: 12, color: '#ffcc00' }),
        ],
      },
      style: {},
    };
    const el = mount(t, CONTENT);
    // Not the legacy region path.
    expect(el.querySelector('.slide')).toBeNull();
    expect(el.querySelector('.lbg')).toBeTruthy();
    expect(el.querySelector('.lshape')).toBeTruthy();
    const texts = [...el.querySelectorAll('.ltext .lfit')].map((n) => n.textContent.trim());
    expect(texts).toContain('For God so loved the world');
    expect(texts).toContain('John 3:16');
  });

  it('positions a layer by its percent geometry', () => {
    const t = {
      layout: { layers: [makeLayer('shape', { x: 10, y: 70, w: 80, h: 20 })] },
      style: {},
    };
    const el = mount(t, CONTENT);
    const s = el.querySelector('.lshape').getAttribute('style');
    expect(s).toMatch(/left:10%/);
    expect(s).toMatch(/top:70%/);
    expect(s).toMatch(/width:80%/);
  });

  it('a scroll layer renders a marquee run', () => {
    const t = {
      layout: { layers: [makeLayer('text', { bind: 'verse', scroll: true })] },
      style: {},
    };
    const el = mount(t, { text: 'Midweek service at 7pm' });
    expect(el.querySelector('.lfit.lscroll .lrun')).toBeTruthy();
  });

  it('a hidden layer is not drawn', () => {
    const t = {
      layout: { layers: [makeLayer('shape', { visible: false })] },
      style: {},
    };
    const el = mount(t, CONTENT);
    expect(el.querySelector('.lshape')).toBeNull();
  });

  it('legacy region templates still render the old way (back-compat)', () => {
    const t = {
      layout: { regions: ['verse_text', 'reference'], align: 'center' },
      style: { background: '#101010', verseColor: '#fff', accent: '#f80' },
    };
    const el = mount(t, CONTENT);
    expect(el.querySelector('.slide')).toBeTruthy(); // region path
    expect(el.querySelector('.lbg')).toBeNull(); // not layer path
  });
});

import { templateShows } from './layers.js';
describe('per-screen content visibility (templateShows)', () => {
  it('no shows set → shows everything', () => {
    const t = { layout: { layers: [] } };
    for (const k of ['scripture', 'song', 'media', 'announce', 'countdown'])
      expect(templateShows(t, k)).toBe(true);
  });
  it('explicit shows list → only those kinds (stage: scripture/song/timer)', () => {
    const stage = { layout: { shows: ['scripture', 'song', 'countdown'] } };
    expect(templateShows(stage, 'scripture')).toBe(true);
    expect(templateShows(stage, 'song')).toBe(true);
    expect(templateShows(stage, 'countdown')).toBe(true);
    expect(templateShows(stage, 'media')).toBe(false);
    expect(templateShows(stage, 'announce')).toBe(false);
  });
  it('legacy noMedia folds in when no explicit list', () => {
    const t = { layout: { noMedia: true } };
    expect(templateShows(t, 'media')).toBe(false);
    expect(templateShows(t, 'scripture')).toBe(true);
  });
});
