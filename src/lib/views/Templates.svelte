<script>
  // THE TEMPLATES WORKSPACE — two desks, each with a browse and a make surface.
  //
  // ── The desks (docs/REBRAND.md §2) ────────────────────────────────────────
  // **Templates** is how a verse, a song or a notice looks on a screen.
  // **Themes** is the style layer BENEATH templates: a theme sets default
  // `style` keys and a template overrides them key by key (DECISIONS §27), so a
  // theme is never the last word and never reaches a wall on its own.
  //
  // Themes used to be a workspace of its own on the shell's strip. It is not one
  // of the six the rebrand's workspace grammar names, and the thing it edits is
  // only ever seen THROUGH a template — so it is a desk inside this workspace
  // rather than a tab beside it. **Nothing became unreachable**: every control
  // the Themes tab carried is still rendered, one press of the desk strip away,
  // which is what `node scripts/qa-inventory.mjs` is checked against.
  //
  // ── The two modes, per desk ───────────────────────────────────────────────
  // A GALLERY of everything and an EDITOR for one. The split is the same shape
  // as Planner (build list vs one plan): a browse surface and a make surface
  // want opposite layouts, so they are separate components rather than one
  // screen doing both badly.
  //
  // Switching desk always lands on that desk's GALLERY. Coming back to a
  // half-finished editor an operator has navigated away from would restore a
  // surface they did not ask for, and the editor's own Back is the way out of
  // it. Nothing here can reach an output, so a desk change costs a service
  // nothing (the rebrand's own rule: loading, switching workspace or editing a
  // template may never change what is on the programme).
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';
  import ThemeGallery from './themes/ThemeGallery.svelte';
  import ThemeEditor from './themes/ThemeEditor.svelte';

  /** Which desk to open on. The workspace opens on Templates; `Themes.svelte`
   *  passes 'themes' so the old tab keeps landing where it always did while the
   *  shell's strip is being reshaped. */
  export let initialDesk = 'templates';

  let desk = initialDesk; // templates | themes
  let mode = 'gallery'; // gallery | editor
  let editingId = null;

  function openEditor(e) {
    editingId = e.detail.id;
    mode = 'editor';
  }
  function backToGallery() {
    mode = 'gallery';
    editingId = null;
  }
  function changeDesk(e) {
    desk = e.detail.desk;
    backToGallery();
  }
</script>

{#if desk === 'themes'}
  {#if mode === 'editor'}
    <ThemeEditor themeId={editingId} on:back={backToGallery} />
  {:else}
    <ThemeGallery on:edit={openEditor} on:desk={changeDesk} />
  {/if}
{:else if mode === 'editor'}
  <TemplateEditor templateId={editingId} on:back={backToGallery} />
{:else}
  <TemplateGallery on:edit={openEditor} on:desk={changeDesk} />
{/if}
