// A MEDIA SLIDE LOOKS LIKE ITS PICTURE, IN THE PLANNER TOO (RG-264).
//
// The operator: *"on the planner slide section media is not showing the
// screenshot or cover of what media is there... needs to be worked on for
// operator to be able to identify media quickly and easily"*.
//
// RG-215 put thumbnails on the running-order row and in the Add-cue results, and
// RG-225 put them on Live's grid cells. The inspector's **Slides** tab was the
// one surface neither touched, and it is the one an operator opens to check what
// a cue will actually put up: `{s.text || s.label}`, and a media slide's `text`
// is `''` by construction (`plan.js::slidesOf`), so it printed the filename in
// dim grey on a dark card. `IMG_3427.mov` tells nobody which clip that is.
//
// **Nothing pinned this markup at all**, which is why it survived two rounds of
// exactly this work: `plannermediapreview.test.js` never opens the Slides tab.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const MEDIA = [
  { id: 7, kind: 'image', filename: 'welcome.png', path: '/m/welcome.png', created_at: '' },
  { id: 9, kind: 'video', filename: 'IMG_3427.mov', path: '/m/IMG_3427.mov', created_at: '' },
];

const PLAN = { id: 1, title: 'Sunday', plan_date: '2026-09-27' };
const ITEMS = [
  {
    id: 11,
    plan_id: 1,
    cue_type: 'media',
    label: 'IMG_3427.mov',
    // `payload_json`, which is what `payloadOf` reads. Written as `payload`
    // first, the fixture handed the slide no `media_id` at all and this case
    // failed over its own setup rather than over the code.
    payload_json: JSON.stringify({ media_id: 9, kind: 'video', filename: 'IMG_3427.mov' }),
    position: 0,
    template_id: null,
    section_title: '',
    duration_sec: 0,
    timer_minutes: null,
  },
];

let app;
let host;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_plans') return [PLAN];
    if (cmd === 'plan_items') return ITEMS;
    if (cmd === 'list_media') return MEDIA;
    if (cmd === 'local_ip') return '192.168.1.50';
    if (cmd === 'list_output_channels') return [];
    if (cmd === 'load_templates') return [];
    if (cmd === 'list_announcements') return [];
    return null;
  });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

async function open() {
  const { default: ServicePlanner } = await import('./views/ServicePlanner.svelte');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new ServicePlanner({ target: host });
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  // OPEN THE PLAN FIRST. `allMedia` and `mediaHost` are loaded on that path,
  // and without them `slideThumb` correctly answers null — a fixture that
  // skipped it would have this case failing over its own setup.
  host.querySelector('.sp-railcard')?.click();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  // Open the cue, then its Slides tab.
  host.querySelector('.sp-row')?.click();
  await tick();
  const tab = [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Slides');
  tab?.click();
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  return host;
}

describe('the Slides tab shows a clip as its first frame', () => {
  it('paints the picture, not the file name', async () => {
    await open();
    const slide = host.querySelector('.sp-slide');
    expect(slide, 'no slide rendered at all').toBeTruthy();
    const thumb = slide.querySelector('.mthumb video, .mthumb img');
    expect(thumb, 'the slide still says only its filename').toBeTruthy();
    expect(thumb.getAttribute('src')).toBe(// The `#t=0.1` is the seek that makes a frame appear at all (RG-279).
      'http://192.168.1.50:8032/media/9#t=0.1');
  });

  it('a clip is a VIDEO element at metadata, which is what paints a first frame', async () => {
    // `preload="metadata"` fetches enough for one frame and never the file —
    // there is no poster column anywhere in the schema, and twenty cues must be
    // twenty small requests rather than twenty downloads.
    await open();
    const v = host.querySelector('.sp-slide .mthumb video');
    expect(v, 'a clip rendered as an image').toBeTruthy();
    expect(v.getAttribute('preload')).toBe('metadata');
  });

  it('and the URL is built by the one builder, never by hand', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve('src/lib/views/ServicePlanner.svelte'), 'utf8');
    const tab = src.slice(src.indexOf('<div class="sp-slides">'), src.indexOf('</div>', src.indexOf('<div class="sp-slides">') + 200));
    expect(tab, 'a hand-rolled media URL').not.toMatch(/:8032|\/media\//);
  });
});
