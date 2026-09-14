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
  // ── TWO AGENTS BUILT THIS MOUNT, AND THE OTHER ONE OWNS IT ────────────────
  //
  // The Templates agent (W-templates) built the same fold on its own branch, with
  // the switcher as `views/templates/DeskStrip.svelte` rendered by BOTH galleries
  // through an `on:desk` event — which is the right home for it, because that
  // agent owns those galleries and this one owns only the shell's strip and the
  // route into this workspace.
  //
  // So this file is deliberately shaped to be replaceable by that one:
  //
  //   · the same `initialDesk` prop, the same `desk`/`mode` state names, and the
  //     same `on:desk` handler, so the galleries' event lands either way;
  //   · `layerId` is carried through `openEditor` exactly as W-templates carries
  //     it, so nothing of the inspector's object jump is lost if this copy is the
  //     one that survives a merge;
  //   · the one block that is genuinely ONLY here is the `.tw-desks` strip
  //     below, marked at its own tag. **Delete it when `DeskStrip` lands** — two
  //     switchers four lines apart is the defect DeskStrip exists to prevent.
  //
  // ── THE ROUTE HALF, WHICH IS THIS AGENT'S ─────────────────────────────────
  //
  // `App.svelte` mounts a workspace with NO PROPS (`<svelte:component
  // this={current} />`), and `views/Themes.svelte` — which used to pass
  // `initialDesk="themes"` — is deleted, because a component nothing renders is
  // an orphan and `qa-inventory` counts them. So which desk you were last on is
  // carried by the SESSION, like the active tab and `liveDensity`: persisted,
  // restored on a reload, and set by `migrateSession` for anyone whose saved
  // session still says `activeTab: 'themes'`. `initialDesk` survives as an
  // override for any caller that does mount this with a prop.
  import { session, setSession } from '../session.js';
  import { t } from '../i18n.js';
  import TemplateGallery from './templates/TemplateGallery.svelte';
  import TemplateEditor from './templates/TemplateEditor.svelte';
  import ThemeGallery from './themes/ThemeGallery.svelte';
  import ThemeEditor from './themes/ThemeEditor.svelte';

  /** Which desk to open on when a caller says so explicitly. Without one, the
   *  session's remembered desk wins. */
  export let initialDesk = null;

  const DESKS = [
    { key: 'templates', label: 'tab.templates' },
    { key: 'themes', label: 'tab.themes' },
  ];
  const known = (k) => DESKS.some((d) => d.key === k);
  // The desk IS the session, the same way the active tab is: one direction, one
  // source of truth, so a reload puts the operator back where they were. A local
  // `let` mirrored back would be a second copy that the next `setSession` from
  // anywhere overwrites — the bug the shell's bottom nav had.
  $: desk = known(initialDesk) ? initialDesk : known($session.templatesDesk) ? $session.templatesDesk : 'templates';

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
  // Switching desk always lands on that desk's GALLERY. Coming back to a
  // half-finished editor an operator has navigated away from would restore a
  // surface they did not ask for, and the editor's own Back is the way out of it.
  // Nothing here can reach an output, so a desk change costs a service nothing
  // (the rebrand's own rule: loading, switching workspace or editing a template
  // may never change what is on the programme).
  function goDesk(key) {
    if (key === desk) return;
    initialDesk = null; // the operator's press outranks whatever mounted this
    setSession({ templatesDesk: key });
    backToGallery();
  }
  const onDeskEvent = (e) => goDesk(e.detail.desk);
</script>

<div class="tw">
  <!-- ▼ THE ONLY BLOCK THAT IS ONLY HERE. Delete it, and `.tw-desks`/`.tw-desk`
       below, the moment `views/templates/DeskStrip.svelte` lands — that switcher
       lives in each gallery's head slot, which is where it belongs, and two
       switchers on one screen is exactly what one shared component prevents.
       Until then this is the rendered control that keeps Themes reachable, and
       `qa-inventory` is the instrument that would notice if it were not. -->
  <nav class="tw-desks" aria-label="Templates workspace desks">
    {#each DESKS as d (d.key)}
      <button
        class="tw-desk r-focus"
        class:on={d.key === desk}
        aria-current={d.key === desk}
        on:click={() => goDesk(d.key)}
      >{$t(d.label)}</button>
    {/each}
  </nav>
  <!-- ▲ -->

  <div class="tw-body">
    {#if desk === 'themes'}
      {#if mode === 'editor'}
        <ThemeEditor themeId={editingId} on:back={backToGallery} />
      {:else}
        <ThemeGallery on:edit={openEditor} on:desk={onDeskEvent} />
      {/if}
    {:else if mode === 'editor'}
      <TemplateEditor templateId={editingId} layerId={editingLayerId} on:back={backToGallery} />
    {:else}
      <TemplateGallery on:edit={openEditor} on:desk={onDeskEvent} />
    {/if}
  </div>
</div>

<style>
  .tw { display: flex; flex-direction: column; gap: var(--v-sp-sm); height: 100%; min-height: 0; }
  .tw-body { flex: 1; min-height: 0; }
  /* ▼ Goes with the block above. */
  .tw-desks { display: flex; align-items: center; gap: 2px; flex: 0 0 auto; }
  /* The same instrument as the chrome's workspace tabs, one level down: a
     selection underline in steel blue — the thing you are working on — no pill
     (§1: a pill in a control room reads as a toy), 3px corners. */
  .tw-desk {
    padding: 4px 12px; border: 0; border-radius: var(--v-r-sm); background: transparent;
    color: var(--v-dim); font-family: var(--f-body); font-size: var(--v-fs-b2);
    font-weight: 600; cursor: pointer; white-space: nowrap;
    transition: background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease);
  }
  .tw-desk:hover:not(.on) { background: var(--v-surf3); color: var(--v-txt); }
  .tw-desk.on { background: var(--v-surf3); color: var(--v-txt); box-shadow: inset 0 -2px 0 var(--v-sel); }
  /* ▲ */
</style>
