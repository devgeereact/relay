// Bundled template backgrounds.
//
// Any image dropped into `src/backgrounds/` is picked up here at build time and
// offered in the Templates editor. Vite bundles each with a hashed URL that
// resolves identically on the operator console, native output windows, and
// kiosk/OBS clients — so a template that references one shows the same picture on
// every screen, with no per-environment path juggling.
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
