// THE ONE RULE THAT DECIDES WHERE A BUNDLED BACKGROUND LANDS IN `dist/`.
//
// Read by `vite.config.js` at build time. It lives here, in a module with no
// build-tool imports of its own, so it can be tested directly — the alternative
// is a rule that only ever runs inside a bundler and is checked by rebuilding
// and looking.
//
// ## Why the backgrounds are not hashed
//
// Wave 5, Track I (DECISIONS §90) makes the pictures in `src/backgrounds/` real
// `media_assets` rows on a fresh install, so a church can reach one from the
// Library and from a template's media layer instead of only from the template
// editor's build-time picker. The seed that writes those rows runs in Rust,
// where Vite's `[hash]` is unknowable — and duplicating the files into the
// binary a second time would add fourteen megabytes to every download of a
// picture that is already in there.
//
// So the backgrounds, and only the backgrounds, are emitted at a stable path:
// `backgrounds/<file>`. `channels::serve_embedded` already serves any path in
// the bundle, so the seed can name one and every screen can load it.
//
// ## Why the name is sanitised
//
// `serve_embedded` takes the path straight off the HTTP request line and hands
// it to `include_dir`. It does not URL-decode. A file called `02 Emerald
// Halfton texture Background.jpg` would be requested as `02%20Emerald%20…` and
// 404 on every output page, so the served name keeps only characters that
// survive a URL untouched.

/** The folder inside `dist/` that bundled backgrounds are served from. */
export const BUNDLED_BACKGROUND_DIR = 'backgrounds';

/** Vite's default for everything that is not a bundled background. */
const HASHED = 'assets/[name]-[hash][extname]';

/** The source folder an asset must have come from to count as a background. */
const SOURCE = 'src/backgrounds/';

/** The source paths rollup recorded for an asset, old and new field alike. */
function originsOf(info) {
  if (Array.isArray(info?.originalFileNames)) return info.originalFileNames;
  if (typeof info?.originalFileName === 'string') return [info.originalFileName];
  return [];
}

/** Did this asset come out of `src/backgrounds/`? */
export function isBundledBackground(origins) {
  return (origins || []).some((n) => String(n).replace(/\\/g, '/').includes(SOURCE));
}

/**
 * The served file name for a background, from its source path or file name.
 * Anything a URL would have to encode becomes an underscore.
 */
export function bundledBackgroundName(original) {
  const file = String(original).replace(/\\/g, '/').split('/').pop();
  return file.replace(/[^A-Za-z0-9._-]/g, '_');
}

/**
 * `build.rollupOptions.output.assetFileNames`. A background gets its stable
 * path; everything else gets the hashed pattern Vite would have used anyway.
 */
export function bundledAssetFileName(info) {
  const origins = originsOf(info);
  if (!isBundledBackground(origins)) return HASHED;
  const source = origins.find((n) => String(n).replace(/\\/g, '/').includes(SOURCE));
  return `${BUNDLED_BACKGROUND_DIR}/${bundledBackgroundName(source)}`;
}

/** The `media_assets.path` prefix that marks a picture Relay ships. */
export const BUNDLED_PREFIX = 'bundled:';

/**
 * Where a `media_assets` row's file is served from.
 *
 * THE SAME RULE AS `main.rs::media_url`, and it has to be, because there are two
 * doors and only two: the backend builds this URL for every output screen, and
 * the Library's media pane builds it again for the thumbnail — the file itself,
 * because nothing in this codebase generates a thumbnail. An imported file lives
 * under `/media/<id>`; a bundled picture has no file there at all and is served
 * out of the bundle at the path its marker names (DECISIONS §90).
 *
 * A row with no `path` is served by id, which is what every row did before
 * bundled ones existed.
 */
export function mediaUrl(host, asset) {
  const path = asset?.path ?? '';
  if (path.startsWith(BUNDLED_PREFIX)) {
    return `http://${host}:8032/${path.slice(BUNDLED_PREFIX.length)}`;
  }
  return `http://${host}:8032/media/${asset?.id}`;
}
