// Phase 12 of the rebrand (docs/REBRAND.md §6): a composite — a camera region
// and a real rendered slide beside it.
//
// Three things are worth holding here, and only one of them is about what it
// looks like:
//
//   1. The word region is ITS OWN CONTAINER. That is the entire feature: `cqw`
//      means "a share of the container's width", so a template rendered inside a
//      region sizes itself to the region exactly as it would to a screen of that
//      width. Without `container-type`, cqw inside the region resolves against
//      the whole frame and every word in the composite is roughly twice the size
//      it should be — on a stream, where nobody at the desk is watching.
//   2. A COMPOSITE MAY NOT BE ANOTHER COMPOSITE'S FILL. A template that names
//      itself would recurse until the webview died, mid-service, on a wall.
//   3. The inner template must resolve on EVERY client. A kiosk or OBS page has
//      no database; it resolves ids against the bundled built-ins. A region
//      pointing at a custom template would render one thing on the operator's
//      wall and another in the stream.
import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';
import { makeLayer, STARTERS } from './layers.js';

let host;
let app;
function mount(template, content, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content, ...props } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

const CONTENT = { reference: 'John 3:16', text: 'For God so loved the world' };

const composite = (over = {}) => ({
  id: 12,
  name: 'Composite',
  layout: {
    layers: [makeLayer('region', { name: 'Word region', x: 52, y: 9, w: 44, h: 82, templateRef: 1, ...over })],
  },
  style: {},
});

describe('a slide region', () => {
  it('renders the verse inside itself', () => {
    const el = mount(composite(), CONTENT);
    const region = el.querySelector('.lregion');
    expect(region).toBeTruthy();
    expect(region.textContent).toContain('For God so loved');
  });

  it('is its own container, so cqw inside it is a share of the REGION', () => {
    const el = mount(composite(), CONTENT);
    expect(el.querySelector('.lregion').className).toContain('lregion');
    // The rule lives in the component's stylesheet; assert the declaration is
    // there rather than the computed value, which jsdom does not resolve.
    const css = readSource();
    expect(css).toMatch(/\.lregion\s*\{[^}]*container-type:\s*inline-size/);
  });

  it('clips what does not fit, so a region cannot spill over the camera', () => {
    expect(readSource()).toMatch(/\.lregion\s*\{[^}]*overflow:\s*hidden/);
  });

  it('does not render a region inside a region', () => {
    // Depth, not a cycle check: the guard has to hold for a template that names
    // a DIFFERENT composite as well as one that names itself.
    const el = mount(composite(), CONTENT, { depth: 1 });
    expect(el.querySelector('.lregion')).toBeNull();
  });

  it('falls back to a built-in when the template it names is not one', () => {
    // A custom id would resolve on the desktop and nowhere else. `builtinById`
    // answers with the default rather than rendering an empty box, so the
    // composite still shows the verse on every client.
    const el = mount(composite({ templateRef: 4096 }), CONTENT);
    expect(el.querySelector('.lregion').textContent).toContain('For God so loved');
  });

  it('shows nothing at all when the screens are cleared', () => {
    // The region is inside the same `{#if content}` as everything else; a
    // composite that kept painting after a clear would be a verse left up.
    const el = mount(composite(), null);
    expect(el.querySelector('.lregion')).toBeNull();
  });
});

describe('the SuperSource starter', () => {
  const made = () => STARTERS.find((s) => s.key === 'supersource').make();

  it('has exactly one slide region', () => {
    const regions = made().layout.layers.filter((l) => l.type === 'region');
    expect(regions.length).toBe(1);
  });

  it('leaves the other half unpainted, because that is where the camera is', () => {
    // Relay does not take a camera feed; the switcher puts the picture behind
    // the transparent half. A background layer here would key out the camera.
    const t = made();
    expect(t.layout.layers.some((l) => l.type === 'background')).toBe(false);
    const region = t.layout.layers.find((l) => l.type === 'region');
    expect(region.x).toBeGreaterThan(20);
    expect(region.x + region.w).toBeLessThanOrEqual(100);
  });

  it('names a built-in for its inner template', () => {
    const region = made().layout.layers.find((l) => l.type === 'region');
    expect(Number.isInteger(region.templateRef)).toBe(true);
    expect(region.templateRef).toBeLessThanOrEqual(5);
  });
});

/** The component's own stylesheet, read from source — jsdom resolves no CSS. */
function readSource() {
  const { readFileSync } = require('node:fs');
  const { resolve } = require('node:path');
  return readFileSync(resolve(__dirname, 'TemplateRender.svelte'), 'utf8');
}
