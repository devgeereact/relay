# Template backgrounds

Drop image files here (`.jpg` `.jpeg` `.png` `.webp` `.avif`) and they appear
automatically in the Templates editor's **Background → Library** picker after a
rebuild — no code change needed. `src/lib/backgrounds.js` globs this folder and
Vite bundles each file with a hashed URL that resolves the same way on the
operator console, native output windows, and kiosk/OBS clients (`:8032`).

Naming: the filename becomes the label, so `deep-blue-marble.jpg` shows as
"Deep Blue Marble". Keep them 1920×1080 (16:9) for a clean fit.

These are bundled INTO the app binary, so keep the set curated — a folder of
30 full-resolution photos adds that many megabytes to every download.
