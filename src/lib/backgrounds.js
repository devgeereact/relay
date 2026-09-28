// Bundled template backgrounds.
//
// Any image dropped into `src/backgrounds/` is picked up here at build time and
// offered in the Templates editor. Each resolves identically on the operator
// console, native output windows and kiosk/OBS clients — so a template that
// references one shows the same picture on every screen, with no per-environment
// path juggling.
//
// THE URLS ARE NOT HASHED, and this comment said they were for as long as they
// had not been (RG-157). Wave 5 Track I (DECISIONS §90) made these pictures real
// `media_assets` rows on a fresh install, and the Rust seed that writes them
// cannot know a Vite content hash, so `vite.config.js` emits them at a stable
// `backgrounds/<file>` — the rule lives in `bundledbackgrounds.js`, where it is
// tested rather than rebuilt and eyeballed.
//
// THAT LEAVES A TRAIL. The picker writes `b.url` straight into a layer's `image`,
// so a background chosen BEFORE that change is stored as the old hashed
// `/assets/<stem>-<hash>.<ext>` and resolves to nothing. Relay does not rewrite
// it — the editor names it and re-picking it is the repair (`staleimagepath.test.js`),
// and the model-level answer is a layer referencing a `media_assets` row by id
// rather than a URL at all, which is a decision and not a patch.
//
// The glob is eager + `?url`, so `BACKGROUNDS` is a plain array ready at import.
// An empty folder yields an empty array (no error) — the picker just shows its
// "drop files here" hint until images exist.
const modules = import.meta.glob('../backgrounds/*.{jpg,jpeg,png,webp,avif}', {
  eager: true,
  query: '?url',
  import: 'default',
});

/** Turn `deep-blue_marble.jpg` into "Deep Blue Marble". */
function prettyName(path) {
  const file = path.split('/').pop().replace(/\.[a-z]+$/i, '');
  return file
    .replace(/[-_]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export const BACKGROUNDS = Object.entries(modules)
  .map(([path, url]) => ({ file: path.split('/').pop(), name: prettyName(path), url }))
  .sort((a, b) => a.name.localeCompare(b.name));
