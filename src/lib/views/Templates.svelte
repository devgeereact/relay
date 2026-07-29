<script>
  // Templates tab = two modes that mirror the two mockups: a GALLERY of every
  // template (relay-templetes-screen) and an EDITOR for one (relay-templeteeditor
  // -screen). Browsing is the default; Edit / New opens the editor, Back returns.
  //
  // The split is the same shape as Planner (build list vs one plan): a browse
  // surface and a make surface want opposite layouts, so they are separate
  // components rather than one screen doing both badly.
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';

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
</script>

{#if mode === 'editor'}
  <TemplateEditor templateId={editingId} on:back={backToGallery} />
{:else}
  <TemplateGallery on:edit={openEditor} />
{/if}
