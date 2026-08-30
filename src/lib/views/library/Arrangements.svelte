<script>
  // ARRANGEMENTS — a named running order for a song's sections.
  //
  // "Verse 1, chorus, verse 2, chorus, bridge, chorus, chorus." Every worship
  // team has one and no two churches sing the same song the same way.
  //
  // ── Why this file exists ──────────────────────────────────────────────────
  //
  // The backend for this shipped a long time ago: `save_arrangement`,
  // `list_arrangements` and `delete_arrangement` are registered, the table is in
  // the schema, `cues.js` expands a plan cue through an arrangement, and the
  // Planner has a picker that offers them when a song is added. Every part of
  // the chain existed except the one that lets a person MAKE one — so the picker
  // could only ever be empty, and the feature was, in practice, absent.
  //
  // It was the single dead command in the repository, recorded in CLAUDE.md
  // rather than hidden, and found by `qa-inventory.mjs` tracing a command one hop
  // further than the contract test does: not "does a wrapper call it" but "does a
  // control an operator can see reach the wrapper".
  //
  // ── The one rule that is not obvious ──────────────────────────────────────
  //
  // An arrangement is stored as SECTION INDICES, not copied words. That is
  // deliberate and right: fix a typo in verse two and every arrangement still
  // plays verse two. But it also means that reordering, inserting, deleting or
  // renaming a section moves the ground under an arrangement — index 3 stops
  // meaning what the person who chose it meant.
  //
  // Relay does not guess which section they wanted. The arrangement is marked
  // stale, it is not offered to a plan until somebody has looked at it, and
  // saving it is the repair. See DECISIONS §55.

  import { humanError } from '../../errors.js';
  import { listArrangements, saveArrangement, deleteArrangement } from '../../stores/capture.js';

  /** The open song — `{ id, title, sections: [{ tag, label, lyrics }] }`. */
  export let song = null;
  /** Told when something was saved or deleted, so the pane's owner can say it. */
  export let onMessage = () => {};

  let list = [];
  let loading = false;
  let error = '';

  // The one being edited: `{ id | null, name, sequence }`. null = the list.
  let draft = null;
  let saving = false;
  let armedDelete = null; // id of the arrangement asking "sure?"

  let loadedFor = null;
  $: if (song?.id !== loadedFor) {
    loadedFor = song?.id ?? null;
    draft = null;
    armedDelete = null;
    load();
  }

  async function load() {
    if (!song) {
      list = [];
      return;
    }
    loading = true;
    error = '';
    try {
      list = (await listArrangements(song.id)) ?? [];
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  }

  const sections = () => song?.sections ?? [];

  /** What a sequence reads as — the section names, in play order. */
  function readable(seq) {
    return (seq ?? [])
      .map((i) => sections()[i]?.tag || sections()[i]?.label || '?')
      .join(' · ');
  }

  /** Is every index in this sequence still a real section? */
  function resolves(seq) {
    return (seq ?? []).every((i) => sections()[i] !== undefined);
  }

  function startNew() {
    draft = { id: null, name: '', sequence: [] };
    armedDelete = null;
  }

  function edit(a) {
    // A copy. Editing the row in place would leave a half-built order on screen
    // if the operator backs out.
    draft = { id: a.id, name: a.name, sequence: [...a.sequence], stale: a.stale };
    armedDelete = null;
  }

  const append = (i) => (draft.sequence = [...draft.sequence, i]);
  const removeAt = (n) =>
    (draft.sequence = draft.sequence.filter((_, k) => k !== n));

  function move(n, by) {
    const to = n + by;
    if (to < 0 || to >= draft.sequence.length) return;
    const next = [...draft.sequence];
    [next[n], next[to]] = [next[to], next[n]];
    draft.sequence = next;
  }

  async function save() {
    if (!draft || !song) return;
    const name = draft.name.trim();
    // Refused here as well as in Rust, so the operator is told before the round
    // trip rather than shown a backend error string.
    if (!name) {
      error = 'Give the arrangement a name — “Sunday”, “Short”, “With bridge”.';
      return;
    }
    if (!draft.sequence.length) {
      error = 'An arrangement needs at least one section. Click the sections below to build it.';
      return;
    }
    saving = true;
    error = '';
    try {
      await saveArrangement(song.id, draft.id, name, draft.sequence);
      onMessage(`Saved “${name}”`);
      draft = null;
      await load();
    } catch (e) {
      error = humanError(e);
    }
    saving = false;
  }

  async function remove(a) {
    if (armedDelete !== a.id) {
      armedDelete = a.id;
      return;
    }
    armedDelete = null;
    error = '';
    try {
      await deleteArrangement(a.id);
      onMessage(`Deleted “${a.name}”`);
      await load();
    } catch (e) {
      // Deleting is held back while a service is recording (Service Lock), and
      // that refusal is a sentence worth reading, not a swallowed failure.
      error = humanError(e);
    }
  }
</script>

<div class="ar">
  <p class="r-lbl ar-head">Arrangements</p>

  {#if !song}
    <p class="ar-empty">Open a song to build a running order for it.</p>
  {:else if loading}
    <p class="ar-empty">Loading…</p>
  {:else if draft}
    <!-- ── BUILDING ONE ────────────────────────────────────────────────── -->
    <label class="r-lbl" for="ar-name">Name</label>
    <input
      id="ar-name"
      class="r-input ar-name"
      bind:value={draft.name}
      placeholder="Sunday morning"
      spellcheck="false" />

    {#if draft.stale}
      <p class="ar-stale">
        The song’s sections changed since this was built, so its order may no longer
        point at the parts you chose. Check it below and save — that is the repair.
      </p>
    {/if}

    <p class="r-lbl ar-sub">Play order</p>
    {#if draft.sequence.length}
      <ol class="ar-seq">
        {#each draft.sequence as idx, n}
          <li class="ar-step" class:missing={sections()[idx] === undefined}>
            <span class="ar-stepno r-mono">{n + 1}</span>
            <span class="ar-steptag">
              {sections()[idx]?.tag || sections()[idx]?.label || 'section that no longer exists'}
            </span>
            <button
              class="r-btn ghost xs r-focus"
              aria-label="Move {n + 1} earlier"
              disabled={n === 0}
              on:click={() => move(n, -1)}>↑</button>
            <button
              class="r-btn ghost xs r-focus"
              aria-label="Move {n + 1} later"
              disabled={n === draft.sequence.length - 1}
              on:click={() => move(n, 1)}>↓</button>
            <button
              class="r-btn ghost xs r-focus"
              aria-label="Remove step {n + 1}"
              on:click={() => removeAt(n)}>✕</button>
          </li>
        {/each}
      </ol>
    {:else}
      <p class="ar-empty">Nothing yet — click a section to add it.</p>
    {/if}

    <p class="r-lbl ar-sub">Sections — click to add, as many times as you sing it</p>
    <div class="ar-palette">
      {#each sections() as s, i}
        <button class="ar-chip r-focus" on:click={() => append(i)}>
          {s.tag || s.label || `Section ${i + 1}`}
        </button>
      {/each}
    </div>

    <div class="ar-acts">
      <button class="r-btn ghost sm" on:click={() => (draft = null)}>Cancel</button>
      <span class="ar-spring"></span>
      <button class="r-btn primary sm" disabled={saving} on:click={save}>
        {saving ? 'Saving…' : 'Save arrangement'}
      </button>
    </div>
  {:else}
    <!-- ── THE LIST ────────────────────────────────────────────────────── -->
    {#if !list.length}
      <p class="ar-empty">
        No arrangements yet. Every song plays in its own order until you build one —
        that order is called “Standard” and is never stored.
      </p>
    {/if}

    {#each list as a (a.id)}
      <div class="ar-row" class:stale={a.stale}>
        <div class="ar-rowmain">
          <b>{a.name}</b>
          {#if a.stale}
            <span class="ar-badge">NEEDS CHECKING</span>
          {/if}
          <span class="r-mono ar-rowseq">{readable(a.sequence)}</span>
          {#if a.stale}
            <span class="ar-why">The song’s sections changed since this was built.</span>
          {:else if !resolves(a.sequence)}
            <span class="ar-why">Part of this order points at a section that is gone.</span>
          {/if}
        </div>
        <button class="r-btn ghost sm r-focus" on:click={() => edit(a)}>Edit</button>
        <button
          class="r-btn danger sm r-focus"
          class:armed={armedDelete === a.id}
          on:click={() => remove(a)}>
          {armedDelete === a.id ? 'Delete — sure?' : 'Delete'}
        </button>
      </div>
    {/each}

    <div class="ar-acts">
      <button class="r-btn primary sm" disabled={!sections().length} on:click={startNew}>
        New arrangement
      </button>
      {#if !sections().length}
        <span class="ar-why">Add some lyrics first — an arrangement orders sections.</span>
      {/if}
    </div>
  {/if}

  {#if error}<p class="ar-err" role="alert">{error}</p>{/if}
</div>

<style>
  .ar {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
  }
  .ar-head {
    margin: 0;
  }
  .ar-sub {
    margin: 6px 0 0;
  }
  .ar-empty,
  .ar-why {
    color: var(--r-dim, #8b8f98);
    font-size: 12px;
    margin: 0;
  }
  .ar-name {
    width: 100%;
  }

  /* Play order */
  .ar-seq {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .ar-step {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border: 1px solid var(--r-line, #2a2d34);
    border-radius: 6px;
  }
  .ar-step.missing {
    border-color: var(--r-rose, #e0526a);
  }
  .ar-stepno {
    color: var(--r-dim, #8b8f98);
    font-size: 11px;
    min-width: 1.4em;
  }
  .ar-steptag {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Section palette */
  .ar-palette {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .ar-chip {
    padding: 4px 10px;
    border: 1px solid var(--r-line, #2a2d34);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .ar-chip:hover {
    border-color: var(--r-accent, #6aa9ff);
  }

  /* The list */
  .ar-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    border: 1px solid var(--r-line, #2a2d34);
    border-radius: 6px;
  }
  /* Amber is ON AIR and is never allowed to mean anything else (DECISIONS §22),
     so a stale arrangement is rose — a thing that is wrong, not a thing that is
     live. */
  .ar-row.stale {
    border-color: var(--r-rose, #e0526a);
  }
  .ar-rowmain {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .ar-rowseq {
    font-size: 11px;
    color: var(--r-dim, #8b8f98);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ar-badge {
    align-self: flex-start;
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--r-rose, #e0526a);
    border: 1px solid currentColor;
    border-radius: 4px;
    padding: 0 4px;
  }

  .ar-stale {
    margin: 0;
    font-size: 12px;
    color: var(--r-rose, #e0526a);
  }
  .ar-acts {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ar-spring {
    flex: 1;
  }
  .ar-err {
    margin: 0;
    color: var(--r-rose, #e0526a);
    font-size: 12px;
  }
</style>
