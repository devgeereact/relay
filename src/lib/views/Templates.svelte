<script>
  // THE TEMPLATES WORKSPACE — browse, then make.
  //
  // A template is how a verse, a song or a notice looks on a screen, and it is
  // the whole answer: themes were folded into templates in wave 2 (DECISIONS
  // §87). A theme was a bag of defaults for the SAME flat `style` keys a
  // template already carries, so the desk beneath this one could only ever say
  // less than the template above it, on nine of its fourteen controls could say
  // nothing at all, and could disagree with it about what a screen wears. Every
  // themed template had its theme inlined into its own style before this desk
  // was removed, so no look changed.
  //
  // WHAT IS LEFT OF IT AND WHY. A layer's colour, fill or font may still be a
  // TOKEN (`theme:accent`) rather than a literal, so a stage or confidence
  // starter follows whatever template it is dropped into — see
  // `lib/styletokens.js`. The token resolves against the template's own style
  // now, which is what the merge already produced once a theme was out of it.
  //
  // This file is the router, and it has ONE decision left: browse or make.
  // Nothing here can reach an output, so opening the editor costs a service
  // nothing (the rebrand's own rule: loading, switching workspace or editing a
  // template may never change what is on the programme).
  //
  // The editor's own Back is the way out of it. Coming back to a half-finished
  // editor an operator has navigated away from would restore a surface they did
  // not ask for, which is why leaving is always to the gallery.
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';

  let mode = 'gallery'; // gallery | editor
  let editingId = null;
  // Which object the gallery's inspector was pointing at, if it was pointing at
  // one (docs/REBRAND.md §3.2). Carried through so a press on the object strip
  // opens the editor on that object rather than on nothing.
  let editingLayerId = null;

  function openEditor(e) {
    editingId = e.detail.id;
    editingLayerId = e.detail.layerId ?? null;
    mode = 'editor';
  }
  function backToGallery() {
    mode = 'gallery';
    editingId = null;
    editingLayerId = null;
  }
</script>

{#if mode === 'editor'}
  <TemplateEditor templateId={editingId} layerId={editingLayerId} on:back={backToGallery} />
{:else}
  <TemplateGallery on:edit={openEditor} />
{/if}
