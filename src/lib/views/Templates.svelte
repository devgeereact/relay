<script>
  // THE TEMPLATES WORKSPACE — two desks, one workspace (docs/REBRAND.md §2).
  //
  // Templates and Themes were two of the eight tabs, and they are one pipeline:
  // a theme sets default `style` keys, a template overrides them per key, and the
  // template is what fires. A theme never reaches a wall on its own, so it was
  // never one of the six things an operator RUNS a service from — it is where you
  // go while you are already editing a look.
  //
  // ── THIS FILE IS THE ROUTER AND THE MOUNT POINT ────────────────────────────
  //
  // It owns the desk switch and nothing else; the galleries and the editors own
  // their own bodies. The switch is deliberately visible here rather than folded
  // into `TemplateGallery`'s `WorkspaceFrame` head slot, so that the boundary is
  // obvious to the next person: moving it into the head is a one-line change in
  // each gallery, and doing it there would put half the routing inside a view.
  //
  // Each desk keeps its own gallery/editor mode. Switching desk does NOT reset
  // the other one, and — the rule that matters — switching desk, opening an
  // editor and saving a template all touch nothing that is on the programme.
  import { session, setSession } from '../session.js';
  import { t } from '../i18n.js';
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';
  import ThemeGallery from './themes/ThemeGallery.svelte';
  import ThemeEditor from './themes/ThemeEditor.svelte';

  const DESKS = [
    { key: 'templates', label: 'tab.templates' },
    { key: 'themes', label: 'tab.themes' },
  ];
  // The desk IS the session, the same way the active tab is: one direction, one
  // source of truth, and a reload puts the operator back where they were. A local
  // `let` mirrored back would be a second copy that the next `setSession` from
  // anywhere overwrites — the bug the bottom nav had.
  $: desk = DESKS.some((d) => d.key === $session.templatesDesk) ? $session.templatesDesk : 'templates';
  const goDesk = (key) => setSession({ templatesDesk: key });

  // Each desk's own browse/edit position. Kept per desk so switching across and
  // back does not throw away the template somebody had open.
  let tplMode = 'gallery'; // gallery | editor
  let tplId = null;
  let themeMode = 'gallery';
  let themeId = null;
</script>

<div class="tw">
  <!-- Rendered ABOVE the desk rather than inside it, because both desks lay
       themselves out in `WorkspaceFrame` and the frame owns a full-height grid.
       Steel blue for the selected desk: it is the thing being worked on, and
       amber/amethyst/cyan are all spoken for by the colour law. -->
  <nav class="tw-desks" aria-label="Templates desks">
    {#each DESKS as d (d.key)}
      <button
        class="tw-desk r-focus"
        class:on={d.key === desk}
        aria-current={d.key === desk}
        on:click={() => goDesk(d.key)}
      >{$t(d.label)}</button>
    {/each}
  </nav>

  <div class="tw-body">
    {#if desk === 'themes'}
      {#if themeMode === 'editor'}
        <ThemeEditor themeId={themeId} on:back={() => { themeMode = 'gallery'; themeId = null; }} />
      {:else}
        <ThemeGallery on:edit={(e) => { themeId = e.detail.id; themeMode = 'editor'; }} />
      {/if}
    {:else if tplMode === 'editor'}
      <TemplateEditor templateId={tplId} on:back={() => { tplMode = 'gallery'; tplId = null; }} />
    {:else}
      <TemplateGallery on:edit={(e) => { tplId = e.detail.id; tplMode = 'editor'; }} />
    {/if}
  </div>
</div>

<style>
  .tw { display: flex; flex-direction: column; gap: var(--v-sp-sm); height: 100%; min-height: 0; }
  .tw-desks { display: flex; align-items: center; gap: 2px; flex: 0 0 auto; }
  /* The same instrument as the chrome's workspace tabs, one level down: a
     selection underline in steel blue, no pill (§1 — a pill in a control room
     reads as a toy), 3px corners. */
  .tw-desk {
    padding: 4px 12px; border: 0; border-radius: var(--v-r-sm); background: transparent;
    color: var(--v-dim); font-family: var(--f-body); font-size: var(--v-fs-b2);
    font-weight: 600; cursor: pointer; white-space: nowrap;
    transition: background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease);
  }
  .tw-desk:hover:not(.on) { background: var(--v-surf3); color: var(--v-txt); }
  .tw-desk.on { background: var(--v-surf3); color: var(--v-txt); box-shadow: inset 0 -2px 0 var(--v-sel); }
  .tw-body { flex: 1; min-height: 0; }
</style>
