// THE PICTURES RELAY SHIPS HAVE TO BE VISIBLE IN THE LIBRARY THEY SHIP INTO.
//
// Wave 5, Track I. DECISIONS §90 gives a fresh install one `media_assets` row
// per picture in the bundle. Those rows carry no file: `path` is
// `bundled:backgrounds/<file>`, a marker, and the bytes are served straight out
// of the embedded bundle.
//
// There are exactly TWO places a media row becomes a URL, and a guarantee is
// only kept on the doors you checked. `main.rs::media_url` is the one every
// output screen is sent. The other is here: `MediaLibrary.svelte` renders the
// FILE ITSELF as the thumbnail, because nothing in this codebase generates one.
// Teaching only the backend would have shipped a library whose every seeded
// picture was a broken image — firable from a plan, invisible on the shelf, with
// the pane's own "missing" state then disabling the Fire button over a file that
// was never missing.
//
// This test mounts the real pane against the real store and reads the `src`
// attribute the browser would fetch.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const BUNDLED = {
  id: 12,
  kind: 'image',
  filename: '02_Emerald_Halfton_texture_Background.jpg',
  path: 'bundled:backgrounds/02_Emerald_Halfton_texture_Background.jpg',
  created_at: '',
};
const IMPORTED = {
  id: 13,
  kind: 'image',
  filename: 'harvest.jpg',
  path: '/Users/x/Library/Application Support/com.relay.app/media/13_harvest.jpg',
  created_at: '',
};

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === 'list_media') return Promise.resolve([BUNDLED, IMPORTED]);
    if (cmd === 'local_ip') return Promise.resolve('192.168.1.9');
    return Promise.resolve([]);
  });
});

afterEach(() => {
  app?.$destroy?.();
  host?.remove?.();
});

async function mountPane() {
  const MediaLibrary = (await import('./views/library/MediaLibrary.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new MediaLibrary({ target: host, props: {} });
  // onMount → listMedia + localIp, then a render once the host is known. Both go
  // through `capture.js`'s dynamic `import('@tauri-apps/api/core')`, which a
  // microtask drain does not settle — a loop of `await Promise.resolve()` left
  // the pane reading "Loading the media library…" for ever and the assertions
  // below passing on an empty list. A real timer is what lets the module graph
  // finish, and the loop stops as soon as anything has rendered.
  for (let i = 0; i < 40 && !sources(host).length; i += 1) {
    await new Promise((r) => setTimeout(r, 0));
    await tick();
  }
  return host;
}

/** Every URL this pane hands the browser for a file, in render order. */
function sources(el) {
  return [...el.querySelectorAll('img, video, source')]
    .map((n) => n.getAttribute('src'))
    .filter(Boolean);
}

describe('the Library media pane reaches a bundled picture', () => {
  it('points a bundled row at the bundle and an imported one at /media/<id>', async () => {
    const el = await mountPane();
    const urls = sources(el);
    expect(
      urls,
      'the pane rendered no file at all — it loads the picture itself as the thumbnail, ' +
        'so an empty list here means this test is proving nothing',
    ).not.toEqual([]);
    // Asserted on the PATH, not the host. Which host the pane uses is a
    // separate pre-existing behaviour — it starts on `localhost` and adopts the
    // LAN address when `local_ip` answers — and pinning it here would make this
    // test fail for a reason that has nothing to do with what it is about.
    const path = (u) => u.replace(/^https?:\/\/[^/]+/, '');
    const paths = urls.map(path);
    expect(paths).toContain('/backgrounds/02_Emerald_Halfton_texture_Background.jpg');
    expect(paths).toContain('/media/13');
    // And the bundled row is NOT asked for by id, which is the URL that 404s.
    expect(paths).not.toContain('/media/12');
    // Every URL is the app's own HTTP server, never Vite's port, which does not
    // exist in a packaged build.
    for (const u of urls) expect(u, u).toMatch(/^http:\/\/[^/]+:8032\//);
  });

  it('names the picture Relay ships, so an operator can find it', async () => {
    const el = await mountPane();
    expect(el.textContent).toContain('02_Emerald_Halfton_texture_Background.jpg');
  });
});
