# Template backgrounds

Drop image files here (`.jpg` `.jpeg` `.png` `.webp` `.avif`) and they appear
automatically in the Templates editor's **Background → Library** picker after a
rebuild — no code change needed. `src/lib/backgrounds.js` globs this folder and
Vite bundles each file with a hashed URL that resolves the same way on the
operator console, native output windows, and kiosk/OBS clients (`:8032`).

Naming: the filename becomes the label, so `deep-blue-marble.jpg` shows as
"Deep Blue Marble". Keep them 1920×1080 (16:9) for a clean fit.

**They are also the Library's picture shelf.** Since DECISIONS §90 a fresh
install seeds one `media_assets` row per file here, so a background can be
fired, put in a plan or bound to a template's media layer, not only picked in
the editor. The build gives these files (and only these) a stable, unhashed path
inside `dist/` — `backgrounds/<file>`, with anything a URL would have to encode
replaced by an underscore — because the Rust seed that writes those rows cannot
know a Vite content hash. `src/lib/bundledbackgrounds.js` is the rule.

Adding or removing a file here therefore changes what a fresh install contains,
and `qa::the_bare_fixture_is_a_first_launch_and_nothing_more` will say so. That
is deliberate: the seed is meant to fail loudly when it drifts.

These are bundled INTO the app binary, so keep the set curated — a folder of
30 full-resolution photos adds that many megabytes to every download.
