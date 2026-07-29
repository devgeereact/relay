<script>
  // Stage Displays tab = two modes mirroring the two mockups: a GALLERY of every
  // display (relay-stagedisplay-screen) and an EDITOR for one (relay-stagedisplay
  // editor-screen). Same split as Templates and Planner: a browse surface and a
  // make surface want opposite layouts, so they are separate components.
  import DisplayGallery from './stagedisplays/DisplayGallery.svelte';
  import DisplayEditor from './stagedisplays/DisplayEditor.svelte';

  let mode = 'gallery';
  let editingId = null;

  function openEditor(e) {
    editingId = e.detail.id;
    mode = 'editor';
  }
  function back() {
    mode = 'gallery';
    editingId = null;
  }
</script>

{#if mode === 'editor'}
  <DisplayEditor displayId={editingId} on:back={back} />
{:else}
  <DisplayGallery on:edit={openEditor} />
{/if}
