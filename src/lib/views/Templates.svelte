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
  // ── Where the desk strip lives ────────────────────────────────────────────
  // In the galleries, through the ONE shared `DeskStrip`, which both render and
  // neither owns. This file is the router: it hears `on:desk` and writes the
  // choice, and it renders no strip of its own — two strips would be two answers
  // to "which desk am I on".
  //
  // ── The desk IS the session ───────────────────────────────────────────────
  // The same way the active tab is: one direction, one source of truth, and a
  // reload puts the operator back where they were. A local `let` mirrored back
  // would be a second copy that the next `setSession` from anywhere overwrites.
  // `migrateSession` sends an operator whose last session was the old Themes TAB
  // to this workspace, on the Themes desk.
  //
  // Switching desk always lands on that desk's GALLERY. Coming back to a
  // half-finished editor an operator has navigated away from would restore a
  // surface they did not ask for, and the editor's own Back is the way out of
  // it. Nothing here can reach an output, so a desk change costs a service
  // nothing (the rebrand's own rule: loading, switching workspace or editing a
  // template may never change what is on the programme).
  import { session, setSession } from '../session.js';
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';
  import ThemeGallery from './themes/ThemeGallery.svelte';
  import ThemeEditor from './themes/ThemeEditor.svelte';

  const DESKS = ['templates', 'themes'];
  $: desk = DESKS.includes($session.templatesDesk) ? $session.templatesDesk : 'templates';

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
  function changeDesk(e) {
    setSession({ templatesDesk: e.detail.desk });
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
  <TemplateEditor templateId={editingId} layerId={editingLayerId} on:back={backToGallery} />
{:else}
  <TemplateGallery on:edit={openEditor} on:desk={changeDesk} />
{/if}
