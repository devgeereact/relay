<script>
  // Library → Announcements: text notice slides (title + body) the operator
  // drafts ahead of a service and fires like any other cue. A plain content
  // type — the template engine renders it; nothing is special-cased downstream.
  import { onMount } from 'svelte';
  import {
    listAnnouncements,
    saveAnnouncement,
    deleteAnnouncement,
    fireContent,
    live,
    screenBlack,
    rehearsing,
  } from '../../stores/capture.js';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import VerseDeck from './VerseDeck.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import ErrorState from '../../ui/ErrorState.svelte';
  import { listActiveTemplates, getContentTemplates, loadTemplates, templates, readErrors } from '../../stores/capture.js';

  export let startDraft = false; // New → "Draft announcement" opens the editor
  /** The Library's one search box. */
  export let query = '';
  export let queue = [];
  export let onQueueChange = () => {};

  let items = [];
  let msg = '';
  let msgT;
  // A FAILURE IS NOT A SUCCESS IN A DIFFERENT COLOUR.
  //
  // Both used to go through `flash()` into one emerald line with no role, so a
  // save that failed was announced to nobody and looked, at a glance, exactly
  // like a save that worked. `err` renders assertively and in the failure colour;
  // `msg` stays the quiet confirmation it always was.
  let err = '';
  // Editor state: null = list view; else { id, title, body } being edited.
  let edit = null;

  let template = null;
  let checked = new Set();
  let layout = 'grid';

  onMount(async () => {
    await refresh();
    // The ANNOUNCE template, not the scripture one — a notice is not a verse.
    const [tpls, ct] = await Promise.all([
      listActiveTemplates().catch(() => []),
      getContentTemplates().catch(() => ({})),
    ]);
    await loadTemplates().catch(() => {});
    const all = $templates ?? [];
    template = all.find((t) => t.id === ct?.announce) ?? tpls[0] ?? all[0] ?? null;
    if (startDraft) draft();
  });

  function toggleQueue(item) {
    if (queue.some((q) => q.reference === item.reference)) {
      onQueueChange(queue.filter((q) => q.reference !== item.reference));
    } else {
      onQueueChange([...queue, { reference: item.reference, text: item.text }]);
    }
  }
  function toggleCheck(item) {
    const next = new Set(checked);
    next.has(item.reference) ? next.delete(item.reference) : next.add(item.reference);
    checked = next;
  }
  /** Duplicating a notice is a REAL new row, not a session overlay — an
      announcement is the operator's own text, so there is nothing to protect. */
  async function duplicate(item) {
    const a = items.find((x) => x.id === item.id);
    if (!a) return;
    try {
      await saveAnnouncement(null, `${a.title} (copy)`, a.body);
      await refresh();
      flash('Duplicated.');
    } catch (e) {
      flash(humanError(e));
    }
  }
  async function refresh() {
    items = await listAnnouncements();
  }
  function flash(t) {
    err = '';
    msg = t;
    clearTimeout(msgT);
    msgT = setTimeout(() => (msg = ''), 2600);
  }

  function draft() {
    edit = { id: null, title: '', body: '' };
  }
  function open(a) {
    edit = { id: a.id, title: a.title, body: a.body };
  }
  async function save() {
    const title = edit.title.trim();
    const body = edit.body.trim();
    if (!title && !body) {
      flash('Add a title or body first.');
      return;
    }
    try {
      await saveAnnouncement(edit.id, title, body);
      edit = null;
      await refresh();
      flash('Saved.');
    } catch (e) {
      err = humanError(e);
    }
  }
  // Two-step delete (no native confirm — Tauri's webview doesn't implement it).
  let delArm = null;
  let delArmT;
  async function remove(a, ev) {
    ev.stopPropagation();
    if (delArm !== a.id) {
      delArm = a.id;
      clearTimeout(delArmT);
      delArmT = setTimeout(() => (delArm = null), 3000);
      return;
    }
    clearTimeout(delArmT);
    delArm = null;
    // `deleteAnnouncement` THROWS (GROUP 1). Unguarded, a refusal — the service
    // lock, most likely, mid-service — became an unhandled rejection: the refresh
    // below never ran, the row stayed on screen, and nothing was said. An operator
    // reads that as a delete that did not take and presses it again.
    try {
      await deleteAnnouncement(a.id);
    } catch (e) {
      err = humanError(e);
      return;
    }
    if (edit && edit.id === a.id) edit = null;
    await refresh();
  }
  // ONE CLICK GOES LIVE, the same as every other pane in the Library.
  let firing = '';
  async function send(a) {
    if ($safeMode) return;
    firing = a.reference ?? a.title;
    try {
      const title = a.title ?? a.label ?? a.reference ?? '';
      const body = a.body ?? a.text ?? title;
      await fireContent(title, body, 'announce');
      flash(`${title || 'Announcement'} is on the screens`);
    } catch (e) {
      flash(humanError(e));
    }
    firing = '';
  }

  const deckOf = (list) =>
    list.map((a, i) => ({
      key: `a${a.id}`,
      id: a.id,
      reference: a.title || 'Untitled',
      label: a.title || 'Untitled',
      text: a.body,
      slideNo: i + 1,
    }));

  const matches = (a) =>
    !query?.trim() ||
    `${a.title ?? ''} ${a.body ?? ''}`.toLowerCase().includes(query.trim().toLowerCase());
  $: shown = items.filter(matches);
  $: deck = deckOf(shown);
  $: queuedRefs = new Set(queue.map((q) => q.reference));
  // What is on the wall right now, so a card can wear the tally.
  $: liveRef = !$screenBlack && $live ? ($live.reference ?? '') : null;
  const isLive = (a, r) => r !== null && !!(a.title || '') && r === a.title;
</script>

<div class="an">
  <section class="an-panel">
    <header class="an-head">
      <div class="an-where">
        <b>Announcements</b>
        <span>{deck.length} notice{deck.length === 1 ? '' : 's'}</span>
      </div>
      <button class="r-btn ghost sm" on:click={draft}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
        New notice
      </button>
      <div class="r-seg" role="group" aria-label="Layout">
        <button class:on={layout === 'grid'} aria-label="Grid" on:click={() => (layout = 'grid')}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1.4" /><rect x="14" y="3" width="7" height="7" rx="1.4" /><rect x="3" y="14" width="7" height="7" rx="1.4" /><rect x="14" y="14" width="7" height="7" rx="1.4" /></svg>
        </button>
        <button class:on={layout === 'list'} aria-label="List" on:click={() => (layout = 'list')}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="5" width="18" height="2.6" rx="1.3" /><rect x="3" y="10.7" width="18" height="2.6" rx="1.3" /><rect x="3" y="16.4" width="18" height="2.6" rx="1.3" /></svg>
        </button>
      </div>
    </header>

    <div class="an-body r-scroll">
      {#if edit}
        <!-- NOT `role="dialog"`. It is an in-flow panel — no scrim, no
             `position:fixed`, no `aria-modal`, no focus trap, nothing bound to
             Escape — and claiming the role made `shortcuts.js` treat it as a modal
             that owns the Escape key. `shortcuts.js` probes the DOM precisely so
             nobody has to remember to register an overlay; the price is that a
             wrong role silently DISARMS the panic key instead of merely
             mislabelling a box.
             That mattered more here than anywhere else: `Esc` is the only panic key
             that survives a focused text field (`B` is suppressed while typing, or
             "Habakkuk" would black out the wall on the second keystroke) and this
             panel is nothing but text fields. While it was open, Escape neither
             closed the editor nor cleared the screens. It did nothing at all.
             `role="group"` is what this is: a labelled region, not a modal. -->
        <div class="an-editor" role="group" aria-label="Announcement editor">
          <div class="an-ehead">
            <span class="r-lbl">{edit.id ? 'Edit announcement' : 'New announcement'}</span>
            <span class="an-spring"></span>
            <button class="r-btn ghost sm" on:click={() => (edit = null)}>Cancel</button>
            <button class="r-btn primary sm" on:click={save}>Save</button>
          </div>
          <label class="an-field">
            <span class="r-lbl">Title</span>
            <input class="r-input" bind:value={edit.title} placeholder="e.g. Midweek service — Wednesday 7pm" />
          </label>
          <label class="an-field">
            <span class="r-lbl">Body</span>
            <textarea class="r-input an-text" bind:value={edit.body} placeholder="The notice text shown on screen…"></textarea>
          </label>
        </div>
      {/if}

      {#if deck.length}
        <VerseDeck
          items={deck}
          {template}
          liveRef={liveRef}
          rehearsing={$rehearsing}
          {checked}
          {queuedRefs}
          busyRef={firing}
          {layout}
          showStar={false}
          can={{ queue: true, favourite: false, edit: true, duplicate: true, add: false, move: false }}
          onCheck={toggleCheck}
          onFire={send}
          onQueue={toggleQueue}
          onEdit={(d) => open(items.find((x) => x.id === d.id))}
          onDuplicate={duplicate}
          onDelete={(d) => remove(items.find((x) => x.id === d.id), new Event('x'))} />
      {:else if !edit && $readErrors.listAnnouncements}
        <!-- RG-95. `listAnnouncements` swallows to `[]`, so a database that did not
             answer used to read as "no announcements yet" — and the operator's
             answer to that sentence is to type the notices again. -->
        <ErrorState error={$readErrors.listAnnouncements} onRetry={refresh} />
      {:else if !edit}
        <EmptyState
          message={query?.trim()
            ? `No announcements matching “${query.trim()}”.`
            : 'No announcements yet — draft one with New notice.'} />
      {/if}
    </div>
  </section>

  <!-- Announced, both of them: an operator watching the wall is not watching this
       corner, and a screen-reader user was told neither. -->
  {#if err}<p class="an-err" role="alert">{err}</p>{/if}
  {#if msg}<p class="an-msg" role="status" aria-live="polite">{msg}</p>{/if}
</div>

<style>
  .an { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; }
  .an-panel { display: flex; flex-direction: column; min-height: 0; flex: 1;
    background: var(--v-bg); border: 1px solid var(--v-line); border-radius: var(--v-r-lg); }
  .an-head { display: flex; align-items: center; gap: 12px; padding: 11px 14px;
    border-bottom: 1px solid var(--v-line); }
  .an-where { flex: 1; min-width: 0; }
  .an-where b { display: block; font-size: 15px; font-weight: 600; color: var(--v-txt); }
  .an-where span { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .an-body { flex: 1; min-height: 0; overflow-y: auto; padding: 12px;
    display: flex; flex-direction: column; gap: 12px; }
  .an-editor { display: flex; flex-direction: column; gap: 12px; padding: 14px;
    background: var(--v-surf); border: 1px solid var(--v-line2); border-radius: var(--v-r-md); }
  .an-ehead { display: flex; align-items: center; gap: 8px; }
  .an-spring { flex: 1; }
  .an-field { display: flex; flex-direction: column; gap: 5px; }
  .an-text { min-height: 110px; padding: 10px 13px; line-height: 1.5; resize: vertical;
    font-family: var(--f-body); }
  .an-msg { margin: 0; font-size: var(--v-fs-b2); color: var(--v-emerald); }
  /* Rose, never amber: amber is the tally light and means ON AIR. */
  .an-err { margin: 0; font-size: var(--v-fs-b2); color: var(--v-red); }
</style>
