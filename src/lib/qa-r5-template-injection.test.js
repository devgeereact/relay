// R5 · A TEMPLATE FILE IS UNTRUSTED INPUT, AND IT REACHES THE WALL.
//
// `TemplateRender.svelte` is the ONE renderer for the fullscreen output and the
// editor preview, so anything that reaches it reaches a wall in front of people.
// Two facts meet here:
//
//   1. `templates.js::parseImportedTemplate` validates SHAPE only — a marker, a
//      `layout` object, a `style` object. Not one value inside either is checked.
//      `importTemplateFromFile` then saves the result as a real template.
//
//   2. `TemplateRender` interpolates those values raw into `style="…"`:
//        bgPaint(L)   → `url("${L.image}")`
//        boxStyle(L)  → `left:${L.x}%; top:${L.y}%; …`
//        the text row → `color:{L.color}; font-family:{fontFamOf(L.font)}; …`
//
// Svelte escapes `"` in an attribute, so a template cannot break OUT of the style
// attribute and inject markup — there is no `{@html}` anywhere in the file, and
// verse/reference text is correctly escaped. Good. But it does not need to break
// out: everything above is already inside a style attribute, so appending `;` and
// another declaration is enough.
//
// The two consequences that matter for this product, in order:
//
//   OFFLINE.  `url(http://…)` in a background layer makes the OUTPUT WINDOW fetch
//             from the internet on every render. Relay's first constraint is that
//             rendering works with zero internet; the shipped CSP explicitly
//             allows `img-src … http:`. A template pack shared between churches
//             ("here's our look") becomes a per-fire beacon and a blank background
//             the day the wifi is out.
//
//   THE WALL. A colour value of `red; position:fixed; inset:0; background:#000`
//             is a full-frame blackout the operator cannot see coming, applied by
//             a file they double-clicked in the Templates tab.
//
// Threat model, stated honestly: this is not a remote attacker. The LAN hub is
// broadcast-only, so nobody can push a template over the network. It is an
// UNTRUSTED FILE, and files are how templates are designed to be shared
// (`exportTemplate` / `importTemplateFromFile` exist for exactly that).
//
// These tests assert the CLEAN behaviour and are therefore RED today. They pass
// the moment imported templates are value-validated.

import { describe, it, expect, afterEach } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { parseImportedTemplate, TEMPLATE_FILE_MARKER } from './templates.js';

let host;
let app;
function render(template, content) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

/** A `.relaytemplate.json` as a church would receive it — a plausible file. */
const hostileFile = JSON.stringify({
  marker: TEMPLATE_FILE_MARKER,
  name: 'Sunday Look',
  style: { accent: '#ffb000' },
  layout: {
    layers: [
      {
        id: 'bg',
        type: 'background',
        x: 0,
        y: 0,
        w: 100,
        h: 100,
        image: 'http://tracker.example/beacon.png',
      },
      {
        id: 'v',
        type: 'text',
        bind: 'verse',
        x: 5,
        y: 40,
        w: 90,
        h: 30,
        size: 4,
        color: 'red; position:fixed; inset:0; background:#000; z-index:9999',
      },
    ],
  },
});

const verse = {
  reference: 'John 3:16',
  text: 'For God so loved the world',
  translation: 'KJV',
};

describe('an imported template file cannot reach the wall unchecked', () => {
  it('is sanitised AT THE IMPORT BOUNDARY — values, not just shape', () => {
    // FIXED 2026-08-14 (P1-9). The importer validated the SHAPE — a marker, a
    // layout object, a style object — and not one value inside either.
    const t = parseImportedTemplate(hostileFile);
    // The beacon is gone…
    expect(t.layout.layers[0].image).toBe('');
    // …and so is the declaration-escape that turns a colour field into a
    // full-frame blackout (`red; position:fixed; inset:0; background:#000`).
    expect(t.layout.layers[1].color).not.toContain('position:fixed');
  });

  it('keeps the values a template legitimately needs', () => {
    // A safety fix that breaks ordinary templates has overshot. Embedded images
    // are the normal case for a shared look — they are how a template travels
    // without a folder of files — and bundled asset paths must survive too.
    const ok = parseImportedTemplate(
      JSON.stringify({
        marker: JSON.parse(hostileFile).marker,
        name: 'Fine',
        layout: {
          layers: [
            { image: 'data:image/png;base64,iVBORw0KGgo=' },
            { image: 'asset://backgrounds/dawn.jpg' },
            { color: '#ffb000', align: 'center', font: 'var(--f-serif)' },
          ],
        },
        style: { accent: 'rgba(255, 176, 0, 0.5)' },
      }),
    );
    expect(ok.layout.layers[0].image).toContain('data:image/png');
    expect(ok.layout.layers[1].image).toBe('asset://backgrounds/dawn.jpg');
    expect(ok.layout.layers[2].color).toBe('#ffb000');
    expect(ok.style.accent).toBe('rgba(255, 176, 0, 0.5)');
  });

  it('never puts a remote URL into the rendered output (offline-first)', () => {
    const t = parseImportedTemplate(hostileFile);
    const el = render(t, verse);
    const styles = [...el.querySelectorAll('[style]')].map((n) => n.getAttribute('style'));
    const remote = styles.filter((s) => /url\(["']?https?:/i.test(s));
    expect(
      remote,
      'a background layer from an imported file put an http:// url into the ' +
        'rendered style, so the output window fetches from the internet on every ' +
        'render. Relay renders offline or it does not render.',
    ).toEqual([]);
  });

  // SUSPECTED, and labelled as such: jsdom is not a faithful CSS oracle.
  //
  // The same unvalidated `color` value that carries `; position:fixed; inset:0;
  // background:#000` is dropped by jsdom's inline-style parser, so this cannot be
  // observed at layer B — but the identical mechanism IS observed one test up,
  // where `background:url("…")` from an unvalidated `image` field survives into
  // the DOM. What can be asserted here without a browser is that the value is
  // interpolated raw, with no filter between the file and the style attribute.
  //
  // Confirming the visual consequence needs a real webview: layer E, and the
  // manual step is in the report's BLOCKED list.
  it('the renderer stays dumb ON PURPOSE, and the boundary carries the rule', async () => {
    // `TemplateRender` still interpolates values straight into style attributes,
    // and that is the DESIGN, not a residual defect. R5's own recommendation was
    // to validate at the import boundary rather than in the renderer, for two
    // reasons that both still hold: the renderer has five call sites and will grow
    // a sixth, and it runs on the hot fire path where this repo already has a hard
    // rule about template-JSON cost.
    //
    // What this test holds is that the boundary is the ONLY thing standing there,
    // so nobody removes it thinking the renderer will catch it.
    const src = await readFile(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');
    expect(src).toContain('background:{bgPaint(L)}'); // still raw, still fine

    const importer = await readFile(resolve(process.cwd(), 'src/lib/templates.js'), 'utf8');
    expect(
      /sanitiseLayout|sanitiseStyleValues/.test(importer),
      'parseImportedTemplate is the only filter between a shared template file and ' +
        'a congregation wall. If it goes, nothing downstream is checking.',
    ).toBe(true);
  });

  it('DOES correctly escape verse text — no markup injection (the half that works)', () => {
    // Worth pinning: there is no {@html} in TemplateRender, so a hostile verse or
    // song title is inert. This is the guarantee that must not regress.
    const el = render(
      {
        layout: {
          layers: [
            { id: 'v', type: 'text', bind: 'verse', x: 0, y: 0, w: 100, h: 100, size: 4 },
          ],
        },
        style: {},
      },
      { ...verse, text: '<img src=x onerror="globalThis.__pwned=1">' },
    );
    expect(el.querySelector('img[onerror]')).toBe(null);
    expect(globalThis.__pwned).toBeUndefined();
    expect(el.textContent).toContain('<img src=x');
  });
});
