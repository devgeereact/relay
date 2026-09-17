// WHERE A BUNDLED BACKGROUND LANDS IN `dist/`, AND WHY IT MAY NOT BE HASHED.
//
// Wave 5, Track I. The pictures in `src/backgrounds/` become real `media_assets`
// rows on a fresh install, so a background is reachable from the Library and
// from a template's media layer rather than only from the template editor's
// build-time picker. There are thirty-three of them today and this file does not
// say so anywhere that matters: the count is read off the folder below, because
// a curated folder is expected to change and a number written down is not.
//
// A media row has to resolve to a URL an output page can actually load, and the
// seed that writes the row runs in Rust, where Vite's `[hash]` is unknowable.
// So the backgrounds — and only the backgrounds — are emitted at a stable path
// the embedded HTTP server can serve straight out of `dist/`:
// `backgrounds/<file>`. Everything else keeps the hashed name Vite gives it.
//
// The sanitisation is not cosmetic. `channels::serve_embedded` takes the request
// path off the request line and hands it to `include_dir` verbatim — it does not
// URL-decode — so a served name containing a space would arrive as `%20` and
// 404. These files are named things like `02 Emerald Halfton texture
// Background.jpg`, which is exactly the case that would have shipped broken.
import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  BUNDLED_BACKGROUND_DIR,
  BUNDLED_PREFIX,
  bundledBackgroundName,
  bundledAssetFileName,
  mediaUrl,
} from './bundledbackgrounds.js';

const folder = resolve(__dirname, '../backgrounds');
const IMAGES = /\.(jpe?g|png|webp|avif)$/i;
const sources = readdirSync(folder).filter((f) => IMAGES.test(f));

describe('bundled background asset names', () => {
  it('keeps every character the embedded HTTP server can serve without decoding', () => {
    for (const f of sources) {
      const served = bundledBackgroundName(`src/backgrounds/${f}`);
      expect(served, f).toMatch(/^[A-Za-z0-9._-]+$/);
    }
  });

  it('gives each source file its own served name', () => {
    const served = sources.map((f) => bundledBackgroundName(f));
    expect(new Set(served).size).toBe(sources.length);
  });

  it('sends a background to the stable folder and everything else to the hashed one', () => {
    expect(bundledAssetFileName({ originalFileNames: ['src/backgrounds/01-2.jpg'] })).toBe(
      `${BUNDLED_BACKGROUND_DIR}/01-2.jpg`,
    );
    expect(bundledAssetFileName({ originalFileNames: ['src/app.css'] })).toBe(
      'assets/[name]-[hash][extname]',
    );
    // No provenance at all is not a background. Rollup hands an asset emitted by
    // a plugin an empty list, and guessing from the basename would put any file
    // called `01-2.jpg` into the folder the seed treats as shipped content.
    expect(bundledAssetFileName({})).toBe('assets/[name]-[hash][extname]');
  });

  it('reads the deprecated singular field too, so a rollup that only sets that still works', () => {
    expect(bundledAssetFileName({ originalFileName: 'src/backgrounds/13.jpg' })).toBe(
      `${BUNDLED_BACKGROUND_DIR}/13.jpg`,
    );
  });

  it('is not fooled by a path that merely mentions the folder name', () => {
    expect(bundledAssetFileName({ originalFileNames: ['src/lib/backgrounds.js'] })).toBe(
      'assets/[name]-[hash][extname]',
    );
  });
});

// A GUARANTEE IS ONLY KEPT ON THE DOORS YOU CHECKED.
//
// There are exactly two places a `media_assets` row becomes a URL: `media_url`
// in `main.rs`, which every output screen is sent, and the Library's own media
// pane, which renders the file itself as the thumbnail because nothing in this
// codebase generates one. A bundled picture has no file under `/media/<id>`, so
// a rule applied to one door and not the other means the pictures Relay ships
// are firable and invisible in the library they ship into — the exact shape of
// bug CLAUDE.md records four times over.
describe('a media row becomes a URL the same way on both doors', () => {
  it('serves an imported file by id and a bundled picture out of the bundle', () => {
    expect(mediaUrl('10.0.0.5', { id: 7, path: '/Users/x/media/7_photo.jpg' })).toBe(
      'http://10.0.0.5:8032/media/7',
    );
    expect(mediaUrl('10.0.0.5', { id: 7, path: '' })).toBe('http://10.0.0.5:8032/media/7');
    expect(mediaUrl('10.0.0.5', { id: 42, path: `${BUNDLED_PREFIX}backgrounds/01-2.jpg` })).toBe(
      'http://10.0.0.5:8032/backgrounds/01-2.jpg',
    );
  });

  it('answers by id when a row arrives with no path field at all', () => {
    // An older caller, or a shape that lost the column. Falling back to the id
    // is the behaviour that shipped before bundled rows existed.
    expect(mediaUrl('127.0.0.1', { id: 3 })).toBe('http://127.0.0.1:8032/media/3');
  });

  it('agrees with the Rust side, which is the other half of the same rule', () => {
    const rust = readFileSync(resolve(__dirname, '../../src-tauri/src/main.rs'), 'utf8');
    const fn = rust.slice(rust.indexOf('fn media_url('), rust.indexOf('fn media_file_is_on_disk('));
    expect(fn, 'media_url no longer reads the bundled marker').toContain('BUNDLED_PREFIX');
    expect(fn).toContain('{ip}:8032/{rest}');
    expect(fn).toContain('{ip}:8032/media/{id}');
    // And the prefix itself is the same string on both sides.
    const starter = readFileSync(
      resolve(__dirname, '../../src-tauri/src/db/starter.rs'),
      'utf8',
    );
    expect(starter).toContain(`BUNDLED_PREFIX: &str = "${BUNDLED_PREFIX}"`);
  });
});
