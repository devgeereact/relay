// A MEDIA CUE IS A PICTURE IN LIVE'S SLIDE GRID (RG-275).
//
// The operator, after the build shipped: *"the snapshot of the media still not
// showing on the planner slides on the live workspace"*.
//
// RG-225 added `mediaId`/`mediaKind` to every cell and taught `cellContent` to
// hand `TemplateRender` a `media_url` — and its test reads `Live.svelte` as TEXT
// and calls `planCells` directly. **Nothing has ever mounted the grid and looked
// at a cell.** That is the gap this file closes, and it is the same gap that let
// a `ReferenceError` ship a fortnight ago: a component nothing renders is not
// covered, however green its tests.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const Live = (await import('./views/Live.svelte')).default;
const { BUILTINS } = await import('./templates.js');
const sess = await import('./session.js');

const settle = (ms = 120) => new Promise((r) => setTimeout(r, ms));

const CHANNEL = { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 1, role: 'main' };
const PLAN = { id: 1, title: 'Sunday morning', plan_date: '2026-09-27' };
const CLIP = { id: 9, kind: 'video', filename: 'IMG_3427.mov', path: '/m/IMG_3427.mov', created_at: '' };
const PHOTO = { id: 7, kind: 'image', filename: 'welcome.png', path: '/m/welcome.png', created_at: '' };

const cue = (over = {}) => ({
  id: 11,
  plan_id: 1,
  cue_type: 'media',
  label: 'IMG_3427.mov',
  payload_json: JSON.stringify({ media_id: 9, kind: 'video', filename: 'IMG_3427.mov' }),
  position: 0,
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  ...over,
});

let host;
let app;
let items = [];

beforeEach(() => {
  items = [cue()];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
    if (cmd === 'list_templates') return Promise.resolve([BUILTINS[0]]);
    if (cmd === 'list_plans') return Promise.resolve([PLAN]);
    if (cmd === 'plan_items') return Promise.resolve(items);
    if (cmd === 'list_media') return Promise.resolve([PHOTO, CLIP]);
    if (cmd === 'local_ip') return Promise.resolve('192.168.1.50');
    if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
    if (cmd === 'get_sensitivity') return Promise.resolve(50);
    return Promise.resolve(null);
  });
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.channelHealth.set({});
  sess.setSession({ activeTab: 'live', planId: 1 });
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  sess.clearSession?.();
});

async function open() {
  app = new Live({ target: host, props: {} });
  await settle();
  await tick();
  await settle();
  await tick();
  return host;
}

describe('the slide grid paints a media cue', () => {
  it('renders the clip’s own frame in the cell, not just its name', async () => {
    await open();
    const cells = [...host.querySelectorAll('.sg-cell')];
    expect(cells.length, 'the grid staged nothing at all').toBeGreaterThan(0);
    const media = cells.find((c) => c.textContent.includes('IMG_3427.mov'));
    expect(media, 'no media cell in the grid').toBeTruthy();
    const pic = media.querySelector('video, img');
    expect(pic, 'the media cell is still only its filename').toBeTruthy();
    // THE FRAGMENT IS THE PICTURE (RG-279). `preload="metadata"` sizes the
    // element and paints nothing, so a cell without `#t=` is a black box with a
    // play icon on it - which is exactly what the operator reported twice.
    expect(pic.getAttribute('src')).toBe('http://192.168.1.50:8032/media/9#t=0.1');
  });

  it('a still picture paints too', async () => {
    items = [cue({ id: 12, label: 'welcome.png', payload_json: JSON.stringify({ media_id: 7, kind: 'image', filename: 'welcome.png' }) })];
    await open();
    const cell = [...host.querySelectorAll('.sg-cell')].find((c) => c.textContent.includes('welcome.png'));
    expect(cell?.querySelector('img'), 'a picture cue drew no picture').toBeTruthy();
  });

  it('and a deleted asset falls back to the words rather than a broken frame', async () => {
    items = [cue({ payload_json: JSON.stringify({ media_id: 404, kind: 'video' }) })];
    await open();
    const cell = [...host.querySelectorAll('.sg-cell')].find((c) => c.textContent.includes('IMG_3427.mov'));
    expect(cell, 'the cue vanished with its asset').toBeTruthy();
    expect(cell.querySelector('video, img'), 'a frame was drawn for an asset that is gone').toBeNull();
  });
});

// ── AND WHEN THERE IS NO PICTURE, IT SAYS WHY (RG-275) ──────────────────────
//
// The operator reported *"the snapshot of the media still not showing"* against
// a build where the grid renders pictures correctly — which is provable above.
// So the remaining question is always about the DATA, and three different
// answers were one silent outcome: the asset was deleted, the cue was saved
// without an id, or Relay could not read the library at all. Each is a
// different thing to do next.
describe('a media cell with no picture says which kind of nothing it is', () => {
  it('names a missing file when the library loaded and the asset is gone', async () => {
    items = [cue({ payload_json: JSON.stringify({ media_id: 404, kind: 'video' }) })];
    await open();
    expect(host.textContent).toContain('file missing');
  });

  it('names an empty library, which is a different problem entirely', async () => {
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
      if (cmd === 'list_templates') return Promise.resolve([BUILTINS[0]]);
      if (cmd === 'list_plans') return Promise.resolve([PLAN]);
      if (cmd === 'plan_items') return Promise.resolve(items);
      if (cmd === 'list_media') return Promise.resolve([]);
      if (cmd === 'local_ip') return Promise.resolve('192.168.1.50');
      if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      return Promise.resolve(null);
    });
    await open();
    expect(host.textContent).toContain('no media library');
  });

  it('names a cue that was never given a file', async () => {
    items = [cue({ payload_json: JSON.stringify({ kind: 'video' }) })];
    await open();
    expect(host.textContent).toContain('no file chosen');
  });

  it('and says none of it when the picture is there', async () => {
    await open();
    for (const noise of ['file missing', 'no media library', 'no file chosen']) {
      expect(host.textContent, `a working cell complained about "${noise}"`).not.toContain(noise);
    }
  });
});
