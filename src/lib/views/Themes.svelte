<script>
  // THEMES IS A DESK, NOT A WORKSPACE — this file is the way in from the old tab.
  //
  // Themes moved INTO the Templates workspace (docs/REBRAND.md §2): a theme is
  // the style layer beneath templates (DECISIONS §27), it never reaches a wall
  // on its own, and it is only ever seen through a template. The gallery and the
  // editor did not move or fork — `Templates.svelte` mounts the same two
  // components, on a desk strip beside Templates.
  //
  // WHY THIS FILE STILL EXISTS. The shell's workspace strip is being reshaped in
  // the same wave, and the two changes land separately. While the `themes` tab
  // is still on the strip it must not open a dead screen, so it opens the
  // Templates workspace on the Themes desk — the same surface, reached the way
  // it always was.
  //
  // **When the shell drops the `themes` entry from its strip and from App.svelte's
  // view map, DELETE THIS FILE.** Left behind it is a component nothing renders,
  // which `scripts/qa-inventory.mjs` reports as an orphan — and an orphan is how
  // a surface stops being covered while its tests stay green.
  import Templates from './Templates.svelte';
</script>

<Templates initialDesk="themes" />
