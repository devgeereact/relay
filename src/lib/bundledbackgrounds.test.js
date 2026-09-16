// WHERE A BUNDLED BACKGROUND LANDS IN `dist/`, AND WHY IT MAY NOT BE HASHED.
//
// Wave 5, Track I. The thirty-four pictures in `src/backgrounds/` become real
// `media_assets` rows on a fresh install, so a background is reachable from the
// Library and from a template's media layer rather than only from the template
// editor's build-time picker.
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
import { readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  BUNDLED_BACKGROUND_DIR,
  bundledBackgroundName,
  bundledAssetFileName,
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
