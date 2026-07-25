<script>
  // Themes tab = the style layer beneath templates (the IA spine). Same two-mode
  // shape as Templates and Planner: a GALLERY of every theme (builtin + custom)
  // and an EDITOR for one. Browsing is the default; New / Edit opens the editor.
  //
  // A theme never reaches a wall on its own — it is applied to a template, and
  // the template is what fires. So nothing here is a live control.
  import ThemeGallery from './themes/ThemeGallery.svelte';
  import ThemeEditor from './themes/ThemeEditor.svelte';

  let mode = 'gallery'; // gallery | editor
  let editingId = null; // theme id (builtin negative, custom positive), or null for new

  function openEditor(e) {
    editingId = e.detail.id;
    mode = 'editor';
  }
  function backToGallery() {
    mode = 'gallery';
    editingId = null;
  }
</script>

{#if mode === 'editor'}
  <ThemeEditor themeId={editingId} on:back={backToGallery} />
{:else}
  <ThemeGallery on:edit={openEditor} />
{/if}
