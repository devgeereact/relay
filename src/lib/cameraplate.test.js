// THE CAMERA PLATE — a preview affordance, and the boundary it may not cross.
//
// A KEYED template (a lower third) paints a band and leaves the rest of the
// frame transparent, because the rest of the frame is a camera the switcher
// supplies. Relay never takes that feed. So on any surface that previews a
// template against NOTHING, a lower third renders as near-black type on
// transparency — an empty dark rectangle. The seeded `Lower Third` built-in is
// exactly that: `background: transparent` with `verseColor: #1c1224`, legible
// over a camera and invisible over nothing.
//
// Outputs solved it and had the markup written out twice, four hundred lines
// apart. It is ONE component now (`ui/CameraPlate.svelte`), used by Outputs'
// screen cards, Outputs' inspector preview, the Templates gallery card and the
// Templates inspector preview.
//
// THE BOUNDARY IS THE HALF THAT MATTERS. The output page is transparent so a
// keyed template keys out for OBS/ATEM; that transparency is the feature. A
// plate behind a real output would paint over the camera it exists to caption.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { isKeyedTemplate } from './layers.js';

const read = (f) => readFileSync(f, 'utf8');

const invoke = vi.fn(async (cmd) =>
  cmd === 'list_templates' || cmd === 'list_output_channels' ? [] : null,
);
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./views/templates/TemplateGallery.svelte');
const { templates } = await import('./stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i++) await settle(); };

// The seeded built-ins, verbatim from `db/templates.rs` — the keyed one and an
// opaque one, so the test holds both sides of the rule rather than only the
// case that prompted it.
const LOWER_THIRD = {
  id: 3,
  name: 'Lower Third',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: true },
  style: { font: 'var(--f-body)', background: 'transparent', accent: '#b080e0', verseColor: '#1c1224', verseSize: '2.6' },
};
const CLASSIC = {
  id: 1,
  name: 'Classic Serif',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false },
  style: { font: 'var(--f-serif)', background: '#101018', verseColor: '#f4e4c8', verseSize: '5.5' },
};

let host;
let cmp;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateGallery({ target: host });
  return cmp;
}
const unmount = () => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; };

const cardFor = (name) =>
  [...host.querySelectorAll('.tg-card')].find((c) => c.querySelector('.tg-name').textContent.trim() === name);

describe('the plate goes behind a template that is a band over something', () => {
  beforeEach(() => {
    invoke.mockClear();
    invoke.mockImplementation(async (cmd) =>
      cmd === 'list_templates' ? structuredClone([CLASSIC, LOWER_THIRD]) : cmd === 'list_output_channels' ? [] : null,
    );
    templates.set(structuredClone([CLASSIC, LOWER_THIRD]));
  });
  afterEach(unmount);

  it('the decision is `isKeyedTemplate`, the one helper both workspaces already share', () => {
    // Not a name check, not a `lowerThird` flag: a layer-model lower third has
    // no such flag (that is why `isKeyedTemplate` exists), and the question is
    // always "does anything paint the whole frame".
    expect(isKeyedTemplate(LOWER_THIRD)).toBe(true);
    expect(isKeyedTemplate(CLASSIC)).toBe(false);
  });

  it('a keyed template gets one on its CARD, and an opaque one does not', async () => {
    mount();
    await drain();
    expect(cardFor('Lower Third').querySelectorAll('.cp').length, 'the lower third').toBe(1);
    expect(cardFor('Classic Serif').querySelectorAll('.cp').length, 'the opaque one').toBe(0);
  });

  it('and on the INSPECTOR preview, which had the same empty rectangle', async () => {
    mount();
    await drain();
    cardFor('Lower Third').click();
    await drain();
    expect(host.querySelector('.tg-preview .cp'), 'keyed').toBeTruthy();

    cardFor('Classic Serif').click();
    await drain();
    expect(host.querySelector('.tg-preview .cp'), 'opaque').toBeFalsy();
  });

  it('is LABELLED, because Relay sends no video', async () => {
    // An unlabelled picture on these surfaces could be mistaken for something
    // Relay is sending. The word is what keeps the plate from being a claim.
    mount();
    await drain();
    expect(cardFor('Lower Third').querySelector('.cp').textContent.trim()).toBe('camera');
  });

  it('and it is decoration — a screen reader is not told about a camera nobody sent', async () => {
    mount();
    await drain();
    expect(cardFor('Lower Third').querySelector('.cp').getAttribute('aria-hidden')).toBe('true');
  });
});

describe('the plate may never reach a real output', () => {
  // THE HALF THAT CARRIES A GUARANTEE. The output page is transparent so a keyed
  // template keys out for OBS/ATEM. A plate there would paint over the camera the
  // band exists to caption — the same failure as blacking out a keyed channel,
  // which `isKeyedTemplate` exists to prevent.
  const OUTPUT_SURFACES = [
    'src/Output.svelte',
    'src/Stage.svelte',
    'src/lib/TemplateRender.svelte',
    'src/lib/TemplatePreviewOverlay.svelte',
  ];

  for (const f of OUTPUT_SURFACES) {
    it(`${f.split('/').pop()} renders no plate`, () => {
      expect(read(f)).not.toMatch(/CameraPlate/);
    });
  }

  it('only preview surfaces import it, and there is exactly ONE of it', () => {
    // A second copy is how a fix lands on one preview and not the other — which
    // is what this component was lifted out of: the markup and its CSS existed
    // twice inside Channels.svelte, four hundred lines apart.
    const importers = [
      'src/lib/views/Channels.svelte',
      'src/lib/views/templates/TemplateGallery.svelte',
    ];
    for (const f of importers) {
      expect(read(f), f).toMatch(/import CameraPlate from '.*ui\/CameraPlate\.svelte'/);
    }
    // Nothing hand-rolls the picture any more.
    for (const f of importers) {
      expect(read(f), `${f} must not still carry its own plate markup`).not.toMatch(/class="ch-plate"/);
    }
  });
});
