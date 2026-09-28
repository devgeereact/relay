// RG-157 — A BACKGROUND CHOSEN BEFORE WAVE 5 IS STORED AS A HASHED
// `/assets/…` URL, AND THAT PATH NO LONGER EXISTS IN THE BUNDLE.
//
// `TemplateEditor`'s background picker writes `b.url` straight into a layer's
// `image`, and `b.url` came out of `import.meta.glob(..., { query: '?url' })` —
// whose built value was whatever `assetFileNames` produced, i.e.
// `/assets/<stem>-<hash>.<ext>`. Wave 5 Track I (DECISIONS §90) made the pictures
// in `src/backgrounds/` real `media_assets` rows, and a Rust seed cannot know a
// Vite content hash, so those files now emit at a stable `backgrounds/<file>`.
// Any template a person saved before that change holds the old path literally.
//
// No SEEDED template can be affected — a seeded row is written in Rust and could
// never have carried a build hash — so this is one picture, on one layer, of one
// template, and only where a person edited it. RG-157 is P3 for that reason and
// the register says plainly that *"the repair worth making is larger than the
// fault"*: a layer should reference a `media_assets` row by ID rather than a URL,
// which is a decision about the template model and not a patch.
//
// ── WHAT THIS FILE IS FOR ────────────────────────────────────────────────────
//
// RG-157's own test cell: *"Nothing asserts what an OLD stored template's `image`
// resolves to, because no fixture in this repository carries one."* This is that
// fixture, and writing it found that **the row's own impact cell was wrong**.
//
// It says: *"The layer falls back to its fill colour rather than blanking the
// screen, so a congregation sees a plain background rather than nothing."* That
// was read off the source and is not what `bgPaint` did. It was:
//
//     L.image ? `url("${L.image}") center / cover no-repeat` : L.fill || 'transparent'
//
// — an image with **no background-colour underneath it**. A 404 therefore painted
// NOTHING, not the fill: on a keyed channel the camera came through the notice,
// and on an opaque one the stage's own ground did. The designer's fill was in the
// template, was what the row promised a church, and was not in the declaration.
// So the mitigation the row rests its priority on is now real rather than assumed:
// the fill is the background COLOUR and the picture is painted over it, which is
// one shorthand and costs nothing when the picture loads.
//
// That is a fix for EVERY failed picture on a background layer, not only a stale
// one — a renamed file, a deleted asset, a CSP refusal — which is why it belongs
// in `TemplateRender` and not in a migration.
//
// ── AND WHAT IS STILL NOT FIXED, SAID PLAINLY ────────────────────────────────
//
// The stored path is still stale. Re-picking the image in the editor is still the
// whole repair, and the editor now SAYS so (it said nothing at all, which is the
// reason RG-157 was filed rather than merely noted). Relay does not rewrite the
// path: a one-off migration from `/assets/<stem>-<hash>.<ext>` to
// `backgrounds/<stem>.<ext>` would work and would be the only code in this
// repository that knows Vite's naming scheme, which is the shape of thing this
// row exists because of.
//
//   npx vitest run src/lib/staleimagepath.test.js

import { describe, it, expect, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { bundledAssetFileName, BUNDLED_BACKGROUND_DIR } from './bundledbackgrounds.js';
import { makeLayer } from './layers.js';

/** THE OLD SHAPE, exactly as a pre-wave-5 save left it in `layout.layers[].image`. */
const STALE = '/assets/emerald-halftone-a1b2c3d4.jpg';

/** A template a person saved before wave 5: one background layer, a real fill of
 *  their own, and a picture at a path the bundle no longer serves. */
const stored = () => ({
  name: 'A church’s own look',
  layout: {
    // Built through `makeLayer`, the one constructor, so the fixture is a template
    // the editor could really have saved — ids and all — rather than a hand-rolled
    // shape the renderer is not obliged to cope with.
    layers: [
      makeLayer('background', { name: 'Background', fill: '#123c2f', image: STALE, opacity: 1, dim: 0 }),
      makeLayer('text', { name: 'Verse', bind: 'verse', x: 8, y: 30, w: 84, h: 40, size: 5, color: '#ffffff' }),
    ],
    align: 'center',
  },
  style: {},
});

const CONTENT = { kind: 'scripture', reference: 'John 3:16', text: 'For God so loved the world' };

let host;
let app;
function mount(template) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content: CONTENT } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('RG-157 · a stored background path that the bundle no longer serves', () => {
  it('the path really is stale — today’s build can never emit it', () => {
    // WHAT AN OLD STORED `image` RESOLVES TO, which is the thing nothing asserted.
    // The build rule is `bundledAssetFileName`, and for a background it answers a
    // stable `backgrounds/<file>` with no hash in it, so nothing in `dist/` can
    // ever be reached by the path above. This is the fact the whole row rests on
    // and it lived only in prose.
    const emitted = bundledAssetFileName({ originalFileNames: ['src/backgrounds/emerald-halftone.jpg'] });
    expect(emitted).toBe(`${BUNDLED_BACKGROUND_DIR}/emerald-halftone.jpg`);
    expect(emitted, 'a background is hashed again, so the old path might resolve').not.toMatch(/-[0-9a-f]{8}\./);
    expect(STALE, 'the fixture is not the shape the old picker wrote').toMatch(/^\/assets\/.+-[0-9a-f]{8}\.\w+$/);
    expect(STALE.startsWith(`/${BUNDLED_BACKGROUND_DIR}/`)).toBe(false);
  });

  it('and the layer still paints the FILL under it, so a failed picture is not a blank frame', () => {
    // THE ROW'S OWN PROMISE, ASSERTED RATHER THAN ASSUMED. `background: url(...)`
    // alone has no colour, so a 404 painted nothing and let whatever was behind the
    // stage through — on a keyed channel, the camera. The designer's fill has to be
    // IN the declaration for the fallback the row describes to exist.
    const el = mount(stored());
    const bg = el.querySelector('.lbg') || el.querySelector('.layer');
    expect(bg, 'no background layer was rendered at all').toBeTruthy();
    const style = bg.getAttribute('style') || '';
    expect(style, 'the picture is not painted').toContain(STALE);
    expect(
      style,
      'the background has no colour under the picture, so a 404 paints nothing — ' +
        'RG-157 promised the fill and this is where that promise is kept',
    ).toMatch(/#123c2f/i);
  });

  it('and the words are still on the screen — a stale picture is never a blank output', () => {
    // The thing that actually matters to a congregation: the verse renders whatever
    // the background did. Asserted because the whole reason this row is P3 rather
    // than P1 is that nothing congregation-facing is lost but the picture.
    const el = mount(stored());
    expect(el.textContent).toContain('For God so loved the world');
  });

  it('the one renderer keeps the fill for EVERY image layer, not only a stale one', () => {
    // `bgPaint` is the one place a background layer's paint is decided, and a
    // picture can fail for reasons that have nothing to do with wave 5 — a renamed
    // file, a deleted `media_assets` row, a CSP refusal. Read off the source, so it
    // cannot be satisfied by a special case for this fixture's URL.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    const fn = src.slice(src.indexOf('const bgPaint ='));
    expect(fn.slice(0, 400), 'bgPaint no longer puts the fill under the image').toMatch(
      /L\.image\s*\?[^:]*L\.fill/,
    );
  });

  it('the one renderer paints the fill alone when there is no picture at all', () => {
    // The other side of the shorthand, so the fix cannot have broken the ordinary
    // case: a background layer with no image is still just its colour, with no
    // `url()` in the declaration for a browser to go and fetch.
    const t = stored();
    t.layout.layers[0].image = null;
    const el = mount(t);
    const style = (el.querySelector('.lbg') || el.querySelector('.layer')).getAttribute('style') || '';
    expect(style).toMatch(/#123c2f/i);
    expect(style, 'a layer with no picture still asks for one').not.toMatch(/url\(/);
  });
});

// THE OPERATOR'S HALF, DRIVEN RATHER THAN GREPPED.
//
// *"It is filed because the failure is silent — nothing says why the picture stopped
// painting."* That is the whole reason RG-157 is a row and not a note, and the
// surface it belongs on is the editor, because **re-picking the image is the entire
// repair**. The picker highlights the tile whose `url` matches `sel.image`, so a
// stale path highlighted nothing — indistinguishable from "no picture chosen".
//
// This block MOUNTS the editor rather than reading its source. A grep would have
// passed against a `staleImage` variable nothing rendered, which is the mistake
// `qa-inventory` exists to catch and that fourteen tests against an unimported
// component already made once in this repository.
describe('RG-157 · the editor says which picture it cannot show, and what to do', () => {
  const invoke = vi.fn();
  vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
  vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

  const settle = () => new Promise((r) => setTimeout(r, 0));
  const drain = async (n = 5) => {
    for (let i = 0; i < n; i++) await settle();
  };

  let ehost;
  let editor;
  /** Open the editor on a template whose background layer carries `image`. */
  async function openOn(image) {
    const { templates, capture } = await import('./stores/capture.js');
    const { default: TemplateEditor } = await import('./views/templates/TemplateEditor.svelte');
    const t = stored();
    t.id = 41;
    t.layout.layers[0].image = image;
    templates.set([structuredClone(t)]);
    capture.update((c) => ({ ...c, available: true }));
    ehost = document.createElement('div');
    document.body.appendChild(ehost);
    // The background layer is the one the notice belongs to, so the editor has to
    // be opened ON it — `layerId` is the door the gallery's object strip uses.
    editor = new TemplateEditor({
      target: ehost,
      props: { templateId: 41, layerId: t.layout.layers[0].id },
    });
    await drain();
    // AND THE PANEL HAS TO BE ON STYLE. The approved canvas keeps an object's
    // identity apart from its look (RG-231): Content answers *what is this*, Style
    // answers *how does it read*, and the Background controls — the picker, and so
    // the notice — live behind Style. The first version of this test asserted
    // against the Content tab, found nothing, and would have reported the notice
    // missing while it rendered perfectly one tab across.
    [...ehost.querySelectorAll('.te-scope button')]
      .find((b) => b.textContent.trim() === 'Style')
      ?.click();
    await drain(2);
    return ehost;
  }
  afterEach(() => {
    editor?.$destroy();
    editor = null;
    ehost?.remove();
    ehost = null;
    invoke.mockReset();
  });

  it('names the stored path and tells the operator to choose it again', async () => {
    const el = await openOn(STALE);
    const note = el.querySelector('.te-stale');
    expect(note, 'the editor says nothing about a picture it cannot show').toBeTruthy();
    // THE PATH ITSELF. "Something went wrong" would not let an operator tell which
    // of two pictures on two layers is the broken one.
    expect(note.textContent, 'the notice does not name the picture').toContain(STALE);
    expect(note.textContent, 'the notice does not name the repair').toMatch(/choose it again/i);
    // Announced, so a screen reader is told as well — a silent colour change is the
    // failure this row is about, one surface along.
    expect(note.getAttribute('role'), 'the notice is not announced').toBe('status');
  });

  it('and says NOTHING about a picture it has no reason to doubt', async () => {
    // THE OTHER DIRECTION, and the one that matters more. Accusing a `data:` URL or
    // an operator's own `http://` picture of being broken — when this surface cannot
    // tell from here whether it loads — is the same defect pointing the other way,
    // and it would train an operator to ignore the line.
    for (const ok of ['data:image/png;base64,iVBORw0KGgo=', 'http://localhost:8032/media/12', null]) {
      const el = await openOn(ok);
      // NOT VACUOUSLY. The notice lives inside the picker block, so "no notice"
      // and "the block never rendered" look identical from here — and a harness
      // that stopped reaching the Style tab would report this passing for ever.
      expect(
        el.querySelector('.te-bglib'),
        'the picker never rendered, so the absence below proves nothing',
      ).toBeTruthy();
      expect(
        el.querySelector('.te-stale'),
        `the editor called ${String(ok).slice(0, 24)} a picture it cannot show`,
      ).toBeNull();
      editor.$destroy();
      editor = null;
      ehost.remove();
      ehost = null;
    }
  });
});
