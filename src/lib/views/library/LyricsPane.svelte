<script>
  // LIBRARY → LYRICS. Songs, the deck they make, and an editor that reflows as
  // you type.
  //
  // ── The three things a worship desk has to get right ──────────────────────
  //
  // 1. YOU PICK BY LOOKING. A rendered deck, not a list of section names. Under
  //    pressure, in a dark booth, recognition is the only thing that works.
  //
  // 2. TEXT AND SLIDES ARE NOT THE SAME THING. A section is what the song IS
  //    (Verse 1, Chorus); a slide is what fits on a wall. Relay used to project
  //    sections verbatim, so a nine-line verse went up as nine lines of tiny
  //    text — or was hand-split into fake sections that then lied about the
  //    song's structure in the plan and in every arrangement built on it.
  //    `lib/reflow.js` does the break, and it is a pure, tested function.
  //
  // 3. EDITING MUST NOT MOVE THE WALL. A word fixed mid-service must not change
  //    what the congregation is reading half a line at a time. `fire_content`
  //    sends the TEXT of the slide at the moment it is fired, so edits reach the
  //    screen only when the operator fires again — and while something of this
  //    song is live, the editor says so out loud.
  import { onMount } from 'svelte';
  import VerseDeck from './VerseDeck.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { parseLyrics, toText, reflow } from '../../reflow.js';
  import { session } from '../../session.js';
  import {
    listSongs,
    searchSongs,
    getSong,
    saveSong,
    deleteSong,
    fireContent,
    listActiveTemplates,
    getContentTemplates,
    loadTemplates,
    templates,
    live,
    screenBlack,
    rehearsing,
  } from '../../stores/capture.js';

  /** Search text from the Library's one search box. */
  export let query = '';
  export let onSelect = () => {};
  export let queue = [];
  export let onQueueChange = () => {};

  let songs = [];
  let song = null;
  let text = '';
  let saved = '';
  let template = null;
  let loading = true;
  let loadingSong = false;
  let editing = false;
  let saving = false;
  let error = '';
  let msg = '';
  let firing = '';
  let armedDelete = false;
  let armedT;
  let checked = new Set();
  let layout = 'grid';

  // THE PROJECTION RULE. It decides what a congregation reads, so it survives a
  // reload — it is persisted with the session rather than reset to a default
  // every time the tab is opened.
  // THE PROJECTION RULE, with no control on it.
  //
  // It was a segmented picker — first "2 3 4 6", then Large/Medium/Small. Both
  // were a question the operator does not want at the moment they are looking
  // for a chorus, and neither told them what it would do until they tried it.
  // Four lines a slide is the size a congregation reads, so that is the rule.
  // It stays configurable in the session (nothing is hard-coded) and can come
  // back as a Settings preference, which is where a once-a-year decision
  // belongs — not on the run surface.
  $: linesPerSlide = Number($session.lyricLines) > 0 ? Number($session.lyricLines) : 4;

  onMount(async () => {
    try {
      const [list, tpls, ct] = await Promise.all([
        listSongs(),
        listActiveTemplates().catch(() => []),
        getContentTemplates().catch(() => ({})),
      ]);
      songs = list ?? [];
      // The SONG template, not the scripture one — a lyric is not a verse and
      // the two are separately designed (per-content-type templates).
      await loadTemplates().catch(() => {});
      const all = $templates ?? [];
      template = all.find((t) => t.id === ct?.song) ?? tpls[0] ?? all[0] ?? null;
      if (songs.length) await open(songs[0]);
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  });

  let lastQuery = null;
  $: if (query !== lastQuery) {
    lastQuery = query;
    runSearch(query);
  }
  async function runSearch(q) {
    songs = (q?.trim() ? await searchSongs(q.trim()) : await listSongs()) ?? [];
  }

  async function open(s) {
    if (dirty && !confirmDiscard()) return;
    loadingSong = true;
    error = '';
    try {
      const full = await getSong(s.id);
      song = full;
      saved = toText(full?.sections ?? []);
      text = saved;
      editing = false;
    } catch (e) {
      error = humanError(e);
    }
    loadingSong = false;
  }

  // No native confirm(): Tauri's webview does not implement it, so a dialog
  // there would silently return undefined and the edit would be dropped.
  let pendingDiscard = false;
  function confirmDiscard() {
    pendingDiscard = true;
    return false;
  }

  async function save() {
    if (!song) return;
    saving = true;
    error = '';
    msg = '';
    try {
      await saveSong({ ...song, sections: parseLyrics(text) });
      saved = text;
      song = { ...song, sections: parseLyrics(text) };
      msg = `Saved ${song.title}`;
      // The rail shows a section count; it just changed.
      songs = (await listSongs()) ?? songs;
    } catch (e) {
      error = humanError(e);
    }
    saving = false;
  }

  function revert() {
    text = saved;
    pendingDiscard = false;
  }

  async function removeSong() {
    if (!song) return;
    if (!armedDelete) {
      armedDelete = true;
      clearTimeout(armedT);
      armedT = setTimeout(() => (armedDelete = false), 4000);
      return;
    }
    armedDelete = false;
    try {
      await deleteSong(song.id);
      songs = songs.filter((s) => s.id !== song.id);
      song = null;
      text = saved = '';
      msg = 'Song deleted.';
    } catch (e) {
      error = humanError(e);
    }
  }

  async function fire(it) {
    if ($safeMode) return;
    firing = it.reference;
    error = '';
    try {
      await fireContent(it.reference, it.text, 'song');
      msg = `${it.label} is on the screens`;
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  // ── Derived ───────────────────────────────────────────────────────────────
  $: dirty = text !== saved;
  $: sections = parseLyrics(text);
  // THE DECK. Recomputed from the text on every keystroke, so what the editor
  // shows and what the operator will fire cannot drift apart.
  $: deck = reflow(sections, { linesPerSlide });
  $: slides = deck.map((s, i) => ({
    key: `${song?.id ?? 0}-${s.key}`,
    slideNo: i + 1,
    section: s.section,
    // The reference is what the cue is CALLED in history and in a plan.
    reference: `${song?.title ?? ''} · ${s.label}`,
    label: s.label,
    text: s.lyrics,
    translation: null,
    // A lyric slide projects the LYRIC. The congregation is not singing the title.
    hideReference: true,
  }));
  // Is any slide of THIS song on the wall right now? If so, editing its words
  // is a live-adjacent act and has to say so.
  $: liveRef = !$screenBlack && $live ? ($live.reference ?? '') : '';
  $: songIsLive = !!song && liveRef.startsWith(`${song.title} ·`);
  $: queuedRefs = new Set(queue.map((q) => q.reference));

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

  // ── Deck edits are REAL edits here ────────────────────────────────────────
  //
  // A lyric is the operator's own text, not scripture, so editing a slide edits
  // the SONG — through the same `parseLyrics` round trip the text editor uses,
  // so the two can never disagree about what the song is. Scripture gets a
  // session overlay instead, because the KJV is not ours to rewrite.
  function sectionsNow() {
    return parseLyrics(text);
  }
  function writeSections(next) {
    text = toText(next);
  }
  function editSlide(item) {
    editing = true;
    // Put the caret work where the operator is looking: open the editor and
    // scroll the textarea to the section this slide came from.
    requestAnimationFrame(() => {
      const el = document.getElementById('ly-text');
      if (!el) return;
      const secs = sectionsNow();
      const before = toText(secs.slice(0, item.section)).length;
      el.focus();
      el.setSelectionRange(before, before);
      el.scrollTop = Math.max(0, (before / Math.max(1, text.length)) * el.scrollHeight - 40);
    });
  }
  function duplicateSlide(item) {
    const secs = sectionsNow();
    const sec = secs[item.section];
    if (!sec) return;
    secs.splice(item.section + 1, 0, { ...sec, label: `${sec.label} (copy)` });
    writeSections(secs);
    msg = 'Section duplicated — Save to keep it.';
  }
  /**
   * MOVE A SECTION. Order is the operator's to choose here — a song is the
   * church's own words, and a chorus can go wherever the band takes it. It
   * writes through the same round trip as every other edit, so the deck, the
   * text editor and what gets saved cannot disagree.
   */
  function moveSlide(item, delta) {
    const secs = sectionsNow();
    const i = item.section;
    const j = i + delta;
    // No wrapping: a mis-click at the top must not send the first verse last.
    if (i < 0 || j < 0 || j >= secs.length) return;
    [secs[i], secs[j]] = [secs[j], secs[i]];
    writeSections(secs);
    msg = 'Section moved — Save to keep it.';
  }

  function addSlideAfter(item) {
    const secs = sectionsNow();
    secs.splice(item.section + 1, 0, { tag: '', label: 'New section', lyrics: '' });
    writeSections(secs);
    msg = 'Section added — Save to keep it.';
  }
</script>

<div class="ly">
  <div class="ly-grid" class:solo={!songs.length}>
    <!-- SONGS -->
    {#if songs.length || loading}
      <nav class="ly-panel ly-rail" aria-label="Songs">
        <p class="r-lbl ly-panelhead">Songs</p>
        <div class="ly-list r-scroll">
          {#each songs as s (s.id)}
            <button class="ly-song r-focus" class:on={song?.id === s.id} on:click={() => open(s)}>
              <b>{s.title}</b>
              <span>
                {s.author ? `${s.author} · ` : ''}{s.section_count} section{s.section_count === 1 ? '' : 's'}
              </span>
            </button>
          {/each}
        </div>
        <div class="ly-panelfoot">
          <span class="r-mono ly-count">{songs.length} song{songs.length === 1 ? '' : 's'}</span>
        </div>
      </nav>
    {/if}

    <section class="ly-panel ly-main">
      <header class="ly-mainhead">
        <div class="ly-where">
          <b>{song?.title ?? (loading ? 'Loading…' : 'No song selected')}</b>
          <span>
            {#if song}
              {song.author ? `${song.author} · ` : ''}{sections.length} section{sections.length === 1 ? '' : 's'}
              · {deck.length} slide{deck.length === 1 ? '' : 's'}{dirty ? ' · unsaved' : ''}
            {:else}
              &nbsp;
            {/if}
          </span>
        </div>

        {#if song}
          <div class="r-seg" role="group" aria-label="Layout">
            <button class:on={layout === 'grid'} aria-label="Grid" on:click={() => (layout = 'grid')}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1.4" /><rect x="14" y="3" width="7" height="7" rx="1.4" /><rect x="3" y="14" width="7" height="7" rx="1.4" /><rect x="14" y="14" width="7" height="7" rx="1.4" /></svg>
            </button>
            <button class:on={layout === 'list'} aria-label="List" on:click={() => (layout = 'list')}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="5" width="18" height="2.6" rx="1.3" /><rect x="3" y="10.7" width="18" height="2.6" rx="1.3" /><rect x="3" y="16.4" width="18" height="2.6" rx="1.3" /></svg>
            </button>
          </div>

          <button class="r-btn ghost sm" on:click={() => (editing = !editing)}>
            {editing ? 'Done' : 'Edit lyrics'}
          </button>
          {#if dirty}
            <button class="r-btn primary sm" disabled={saving} on:click={save}>
              {saving ? 'Saving…' : 'Save'}
            </button>
          {/if}
        {/if}
      </header>

      {#if pendingDiscard}
        <div class="ly-warn">
          <span>{song?.title ?? 'This song'} has unsaved edits.</span>
          <button class="r-btn primary sm" on:click={save}>Save them</button>
          <button class="r-btn ghost sm" on:click={revert}>Discard</button>
        </div>
      {/if}

      {#if editing && songIsLive}
        <!-- Not a warning about damage — a statement of fact, so nobody edits a
             word expecting the wall to follow. -->
        <div class="ly-note">
          A slide from this song is on the screens. Editing changes the deck; the
          congregation sees it when you fire the slide again.
        </div>
      {/if}

      <div class="ly-body" class:split={editing}>
        {#if editing}
          <div class="ly-edit">
            <label class="r-lbl" for="ly-text">Lyrics</label>
            <textarea
              id="ly-text"
              class="r-input ly-text"
              bind:value={text}
              spellcheck="false"
              placeholder={'[Verse 1]\nline one\nline two\n\n[Chorus]\nsing it'}></textarea>
            <p class="ly-help">
              A blank line starts a new section. <code>[Chorus]</code>, <code>Chorus</code> or
              <code>V2</code> on its own line names one. The deck on the right rebuilds as you type.
            </p>
            <div class="ly-editacts">
              <button class="r-btn ghost sm" disabled={!dirty} on:click={revert}>Revert</button>
              <span class="ly-spring"></span>
              <button class="r-btn danger sm" class:armed={armedDelete} on:click={removeSong}>
                {armedDelete ? 'Delete — sure?' : 'Delete song'}
              </button>
            </div>
          </div>
        {/if}

        <div class="ly-deck r-scroll">
          {#if loading || loadingSong}
            <Loading what={loading ? 'songs' : 'the song'} />
          {:else if !songs.length}
            <EmptyState
              message={query?.trim()
                ? `No songs matching “${query.trim()}”.`
                : 'No songs yet — import or paste one with the Import button.'} />
          {:else if !slides.length}
            <EmptyState message="This song has no words yet — use Edit lyrics." />
          {:else}
            <VerseDeck
              items={slides}
              {template}
              liveRef={$live?.reference ?? null}
              rehearsing={$rehearsing}
              {checked}
              {queuedRefs}
              busyRef={firing}
              {layout}
              showStar={false}
              can={{ queue: true, favourite: false, edit: true, duplicate: true, add: true, move: true }}
              onCheck={toggleCheck}
              onFire={fire}
              onQueue={toggleQueue}
              onEdit={editSlide}
              onDuplicate={duplicateSlide}
              onAddAfter={addSlideAfter}
              onMove={moveSlide} />
          {/if}
        </div>
      </div>
    </section>
  </div>

  {#if msg}<p class="ly-msg">{msg}</p>{/if}
  {#if error}<p class="ly-err">{error}</p>{/if}
</div>

<style>
  .ly {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    flex: 1;
  }
  .ly-grid {
    display: grid;
    grid-template-columns: 216px minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
    flex: 1;
  }
  .ly-grid.solo {
    grid-template-columns: minmax(0, 1fr);
  }
  .ly-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--v-bg);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
  }
  .ly-panelhead {
    margin: 0;
    padding: 13px 14px 9px;
  }
  .ly-list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 8px 8px;
  }
  .ly-song {
    display: block;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--v-r-md);
    background: none;
    border: 0;
    color: var(--v-dim);
    font-family: var(--f-body);
    text-align: left;
    cursor: pointer;
  }
  .ly-song:hover:not(.on) {
    background: var(--v-surf2);
    color: var(--v-txt);
  }
  /* Selection is chrome — the accent, never amber. */
  .ly-song.on {
    background: var(--v-accent-fill);
    color: var(--v-accent-ink);
  }
  .ly-song b {
    display: block;
    font-size: 13px;
    font-weight: 600;
  }
  .ly-song span {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    color: var(--v-faint);
  }
  .ly-song.on span {
    color: rgba(255, 255, 255, 0.72);
  }
  .ly-panelfoot {
    padding: 10px 14px;
    border-top: 1px solid var(--v-line);
  }
  .ly-count {
    font-size: 10px;
    color: var(--v-faint);
  }

  .ly-mainhead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--v-line);
    flex-wrap: wrap;
  }
  .ly-where {
    flex: 1;
    min-width: 140px;
  }
  .ly-where b {
    display: block;
    font-size: 15px;
    font-weight: 600;
    color: var(--v-txt);
  }
  .ly-where span {
    font-size: var(--v-fs-cap);
    color: var(--v-faint);
  }

  .ly-warn,
  .ly-note {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 14px;
    font-size: var(--v-fs-b2);
    border-bottom: 1px solid var(--v-line);
  }
  .ly-warn {
    background: var(--v-amethyst-soft);
    color: var(--v-txt);
  }
  .ly-warn span {
    flex: 1;
  }
  /* Amber, because it is about what the congregation can see — and it is a
     statement, not an alarm, so it is a strip and not a modal. */
  .ly-note {
    background: var(--v-amber-soft);
    color: var(--v-amber2);
  }

  .ly-body {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
    flex: 1;
  }
  .ly-body.split {
    grid-template-columns: minmax(280px, 34%) minmax(0, 1fr);
  }
  .ly-edit {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-height: 0;
    padding: 12px;
    border-right: 1px solid var(--v-line);
  }
  .ly-text {
    flex: 1;
    min-height: 0;
    width: 100%;
    padding: 11px 13px;
    height: auto;
    resize: none;
    font-family: var(--f-mono);
    font-size: 12.5px;
    line-height: 1.65;
    tab-size: 2;
  }
  .ly-help {
    margin: 0;
    font-size: var(--v-fs-cap);
    line-height: 1.55;
    color: var(--v-faint);
  }
  .ly-help code {
    font-family: var(--f-mono);
    font-size: 10.5px;
    padding: 1px 4px;
    border-radius: var(--v-r-sm);
    background: var(--v-surf2);
    color: var(--v-dim);
  }
  .ly-editacts {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .ly-spring {
    flex: 1;
  }
  .ly-editacts .armed {
    background: var(--v-rose-soft);
  }

  .ly-deck {
    min-height: 0;
    overflow-y: auto;
    padding: 12px;
  }

  .ly-msg,
  .ly-err {
    margin: 0;
    font-size: var(--v-fs-b2);
  }
  .ly-msg {
    color: var(--v-emerald);
  }
  .ly-err {
    color: var(--v-red);
  }

  @media (max-width: 1140px) {
    .ly-grid {
      grid-template-columns: 180px minmax(0, 1fr);
    }
    .ly-body.split {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(180px, 40%) minmax(0, 1fr);
    }
    .ly-edit {
      border-right: 0;
      border-bottom: 1px solid var(--v-line);
    }
  }
  @media (max-width: 860px) {
    .ly-grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .ly-rail {
      display: none;
    }
  }
</style>
