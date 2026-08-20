<script>
  // Mobile stage-display remote — the preacher opens this on a phone/iPad (via
  // QR or the LAN URL) to see the live verse + reference in real time. No Tauri
  // runtime: it connects to the kiosk WebSocket hub (:8031) for content, exactly
  // like an OBS/kiosk output, but rendered as a readable mobile confidence view.
  import { onMount, onDestroy } from 'svelte';

  let content = null;
  let visible = false;
  let note = ''; // operator's confidence-monitor note for the live cue
  let next = null; // { label, text } — the "up next" preview
  let connected = false;
  let ws = null;
  let closed = false;
  let clock = '';
  let timer;

  // Preacher control plane — the phone can DRIVE the wall, not just mirror it.
  // Hits the LAN HTTP API on :8031's sibling port (:8032/api/*), which runs the
  // SAME fire/nav path the console does. LAN-only, no auth (see channels.rs).
  let showCtl = false;
  let q = '';
  let results = [];
  let searching = false;
  let busy = false; // a nav/fire request is in flight
  let ctlErr = '';
  const API = `http://${location.hostname || 'localhost'}:8032/api`;

  // Anything that CHANGES the wall goes by POST, and the backend refuses it as a
  // GET (405). That is what stops `<img src=".../api/black">` on any page anyone on
  // the church network happens to open from blacking out the congregation's wall —
  // an image, a script, a stylesheet and a link can only ever issue GET.
  // DECISIONS §35. `search` and `live` mutate nothing and stay GET.
  const MUTATES = new Set(['fire', 'next', 'prev', 'clear', 'black']);

  async function api(path) {
    const route = path.split('?')[0];
    const method = MUTATES.has(route) ? 'POST' : 'GET';
    const r = await fetch(`${API}/${path}`, { method });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const j = await r.json();
    if (!j.ok) throw new Error(j.error || 'failed');
    return j;
  }

  let searchSeq = 0;
  async function doSearch() {
    const term = q.trim();
    if (!term) { results = []; return; }
    const seq = ++searchSeq;
    searching = true;
    ctlErr = '';
    try {
      const j = await api(`search?q=${encodeURIComponent(term)}`);
      if (seq === searchSeq) results = j.results || [];
    } catch (e) {
      if (seq === searchSeq) { results = []; ctlErr = 'Search failed — check the connection.'; }
    } finally {
      if (seq === searchSeq) searching = false;
    }
  }

  async function fire(reference) {
    if (busy) return;
    busy = true; ctlErr = '';
    try {
      await api(`fire?ref=${encodeURIComponent(reference)}`);
      results = []; q = '';
    } catch (e) {
      ctlErr = 'Could not put that on screen.';
    } finally { busy = false; }
  }

  // Not every outcome is a failure, and the preacher is entitled to know WHICH.
  // The end of a reading is a correct boundary; a verse missing from the library
  // is a real fault. Only `fired` moved the wall.
  const NAV_SAID = {
    end_of_passage: 'End of the reading.',
    no_passage: 'Nothing on screen yet — tap a verse first.',
    not_in_library: 'That verse is not in the library.',
  };

  async function nav(dir) {
    if (busy) return;
    busy = true; ctlErr = '';
    try {
      // The backend answers `ok: true` for every outcome it handled — including
      // the ones where NOTHING MOVED. So the catch below is not enough on its own:
      // it only ever fires on a transport failure, which meant tapping Next at the
      // end of a reading did nothing, said nothing, and left the preacher tapping.
      const j = await api(dir); // 'next' | 'prev'
      if (j.nav && j.nav.kind !== 'fired') {
        ctlErr = NAV_SAID[j.nav.kind] ?? (dir === 'next' ? 'No next verse.' : 'No previous verse.');
      }
    } catch (e) {
      ctlErr = dir === 'next' ? 'No next verse.' : 'No previous verse.';
    } finally { busy = false; }
  }

  // Countdown mirror — ticked by the same 1s timer as the wall clock.
  let cdTo = null;
  let cdDone = '';
  let nowMs = 0;
  $: cdRemain = cdTo ? Math.max(0, cdTo - nowMs) : null;
  $: cdFinished = cdRemain === 0;
  $: cdText = (() => {
    if (cdRemain == null) return '';
    const s = Math.round(cdRemain / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  })();

  function apply(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation };
      note = m.stage_note || '';
      cdTo = m.countdown_to || null;
      cdDone = m.countdown_done || '';
      nowMs = Date.now();
      visible = true;
    } else if (m.kind === 'clear' || m.kind === 'black') {
      // `black` HAS to be here, and it was not.
      //
      // The hub publishes four kinds and this page handled three. `Output.svelte`
      // honours `black`; this one did not — so the operator hit `B`, the
      // congregation's wall went dark, and the screen the preacher is READING FROM
      // kept the verse. The console reported success, correctly: the message did
      // leave the machine. Nobody was told a screen had ignored it.
      //
      // Blanking on `black` rather than ignoring it is the conservative reading of
      // a genuine ambiguity, and the ambiguity is worth stating because the other
      // answer is defensible. A stage monitor faces the PREACHER, not the
      // congregation, so one could argue a blackout — whose purpose is "the
      // congregation must see nothing" — should leave it alone. But `clear`
      // already blanks this page, and **the harsher control must never do less
      // than the milder one**. An operator who has just hit the emergency key
      // cannot be asked to remember that it reaches three screens out of four.
      //
      // If Relay ever decides the stage monitor should survive a panic, it must
      // survive BOTH controls, deliberately, in both branches — not by one of them
      // being forgotten.
      visible = false;
      note = '';
      cdTo = null;
      next = null;
    } else if (m.kind === 'stage_next') {
      next = m.label || m.text ? { label: m.label || '', text: m.text || '' } : null;
    }
  }

  function connect(host) {
    if (closed) return;
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onopen = () => (connected = true);
      ws.onmessage = (e) => {
        try { apply(JSON.parse(e.data)); } catch { /* ignore */ }
      };
      ws.onclose = () => {
        connected = false;
        if (!closed) setTimeout(() => connect(host), 1500);
      };
      ws.onerror = () => { try { ws.close(); } catch { /* onclose retries */ } };
    } catch {
      if (!closed) setTimeout(() => connect(host), 1500);
    }
  }

  onMount(() => {
    connect(location.hostname || 'localhost');
    const tick = () => {
      clock = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
      nowMs = Date.now(); // drives the countdown mirror
    };
    tick();
    timer = setInterval(tick, 1000);
  });
  onDestroy(() => {
    closed = true;
    if (ws) ws.close();
    clearInterval(timer);
  });
</script>

<div class="sr">
  <header>
    <span class="brand">Relay · Stage</span>
    <span class="status" class:on={connected}><i></i>{connected ? 'live' : 'connecting…'}</span>
    <span class="clock">{clock}</span>
    <button class="ctl-toggle" class:active={showCtl} on:click={() => (showCtl = !showCtl)} aria-label="Control panel">
      {showCtl ? 'Done' : 'Control'}
    </button>
  </header>
  <main>
    {#if visible && cdTo}
      {#if content.reference && !cdFinished}<div class="ref">{content.reference}</div>{/if}
      <div class="verse countdown">{cdFinished ? (cdDone || '0:00') : cdText}</div>
      {#if note}<div class="note"><span class="note-lbl">Note</span>{note}</div>{/if}
    {:else if visible && content}
      {#if content.reference}<div class="ref">{content.reference}{content.translation ? ' · ' + content.translation : ''}</div>{/if}
      {#if content.text}<div class="verse">{#if content.reference}“{content.text}”{:else}{content.text}{/if}</div>{/if}
      {#if note}<div class="note"><span class="note-lbl">Note</span>{note}</div>{/if}
    {:else}
      <div class="idle">— standby —</div>
    {/if}
  </main>
  {#if showCtl}
    <section class="ctl">
      <div class="nav-row">
        <button class="nav-btn" on:click={() => nav('prev')} disabled={busy}>‹ Prev</button>
        <button class="nav-btn" on:click={() => nav('next')} disabled={busy}>Next ›</button>
      </div>
      <form class="search" on:submit|preventDefault={doSearch}>
        <input
          type="search"
          inputmode="search"
          enterkeyhint="search"
          placeholder="Search a verse — “John 3:16” or “shepherd”"
          bind:value={q}
          on:input={doSearch}
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false" />
        <button type="submit" class="go" disabled={searching}>{searching ? '…' : 'Go'}</button>
      </form>
      {#if ctlErr}<div class="ctl-err">{ctlErr}</div>{/if}
      {#if results.length}
        <ul class="results">
          {#each results as r}
            <li>
              <button class="result" on:click={() => fire(r.reference)} disabled={busy}>
                <span class="r-ref">{r.reference}</span>
                <span class="r-text">{r.text}</span>
              </button>
            </li>
          {/each}
        </ul>
      {:else if q.trim() && !searching}
        <div class="no-results">No matches.</div>
      {/if}
    </section>
  {/if}
  {#if next}
    <footer class="next">
      <span class="next-lbl">Up next</span>
      <div class="next-body">
        {#if next.label}<span class="next-ref">{next.label}</span>{/if}
        {#if next.text}<span class="next-text">{next.text}</span>{/if}
      </div>
    </footer>
  {/if}
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: var(--v-void); overflow: hidden; -webkit-font-smoothing: antialiased; }
  .sr {
    height: 100dvh; display: flex; flex-direction: column; color: var(--v-txt);
    font-family: var(--f-body);
    padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
  }
  header { display: flex; align-items: center; gap: 12px; padding: 14px 18px; border-bottom: 1px solid rgba(255,255,255,.08); flex: 0 0 auto; }
  .brand { font-family: var(--f-head); font-weight: 700; font-size: 16px; color: var(--v-amber); }
  /* CONTRAST. This page is read on a phone, at arm's length, in a lit auditorium —
     by the preacher, mid-sermon. It is the least forgiving reading condition in the
     whole product, and it had the worst text in it.

     These were #6c6b71 (3.75:1) and, for .idle, #4a4a50 (2.25:1) — both below the
     WCAG AA floor of 4.5:1 on this background. #6c6b71 is the exact value app.css
     documents as REMOVED for failing AA; the console was fixed and the phone was
     left behind, because it hardcodes hexes instead of using the --v-* tokens.

     #88888d is --v-faint: 5.61:1 here. Still quiet, and actually readable. */
  .status { margin-left: auto; display: inline-flex; align-items: center; gap: 6px; font-family: var(--f-mono); font-size: 11px; color: var(--v-faint); }
  .status.on { color: var(--v-emerald); }
  .status i { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
  .status.on i { box-shadow: 0 0 8px currentColor; animation: p 1.7s ease-in-out infinite; }
  @keyframes p { 0%, 100% { opacity: 1; } 50% { opacity: .4; } }
  .clock { font-family: var(--f-mono); font-size: 13px; color: var(--v-dim); }
  main { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; padding: 24px; gap: 18px; min-height: 0; }
  .ref { font-family: var(--f-mono); font-size: clamp(13px, 3.5vw, 20px); letter-spacing: .18em; text-transform: uppercase; color: var(--v-amber); }
  .verse { font-family: var(--f-serif); font-size: clamp(26px, 7vw, 64px); line-height: 1.28; color: var(--v-txt); max-width: 16ch; }
  /* The DEFAULT resting state of the preacher's phone — the thing on screen before
     anything is fired, and therefore the text most likely to be looked at. It was
     2.25:1: the worst contrast in the product, in its least forgiving location. */
  .idle { font-family: var(--f-mono); color: var(--v-faint); font-size: 14px; letter-spacing: .1em; }
  .countdown { font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    font-size: clamp(56px, 20vw, 160px); color: var(--v-amber); line-height: 1; letter-spacing: .02em; max-width: none; }
  /* Operator's cue note — confidence-monitor only, never on the main output. */
  .note { display: inline-flex; align-items: center; gap: 10px; max-width: 30ch; margin-top: 4px;
    padding: 10px 16px; border-radius: 12px; background: rgba(255,176,0,.1);
    border: 1px solid rgba(255,176,0,.32); color: var(--v-amber2);
    font-family: var(--f-body); font-size: clamp(14px, 2.6vw, 20px); line-height: 1.35; }
  .note-lbl { font-family: var(--f-mono); font-size: 9px; font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; color: var(--v-amber); flex: 0 0 auto; }
  /* Up-next panel — confidence info the preacher wants, kept off the main output. */
  .next { flex: 0 0 auto; display: flex; align-items: baseline; gap: 14px; padding: 14px 20px;
    border-top: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  .next-lbl { font-family: var(--f-mono); font-size: 10px; font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; color: var(--v-faint); flex: 0 0 auto; }
  .next-body { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .next-ref { font-family: var(--f-mono); font-size: 12px; letter-spacing: .06em; color: var(--v-amber);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .next-text { font-family: var(--f-head); font-size: 16px; color: var(--v-dim); line-height: 1.3;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  @media (orientation: landscape) { .verse { font-size: clamp(28px, 6vw, 72px); max-width: 22ch; } }
  @media (prefers-reduced-motion: reduce) { .status.on i { animation: none; } }

  /* Preacher control panel — a phone that DRIVES the wall. Touch-sized targets
     (44px+), high contrast, and it never touches the mirror above it. */
  .ctl-toggle { flex: 0 0 auto; font-family: var(--f-mono); font-size: 11px; font-weight: 700;
    letter-spacing: .12em; text-transform: uppercase; color: var(--v-dim); cursor: pointer;
    background: rgba(255,255,255,.04); border: 1px solid rgba(255,255,255,.14);
    border-radius: 8px; padding: 7px 12px; }
  .ctl-toggle.active { color: var(--v-amber); border-color: rgba(255,176,0,.4); background: rgba(255,176,0,.08); }
  .ctl { flex: 0 0 auto; display: flex; flex-direction: column; gap: 12px; padding: 16px 18px;
    border-top: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.02);
    max-height: 60dvh; overflow-y: auto; }
  .nav-row { display: flex; gap: 12px; }
  .nav-btn { flex: 1; min-height: 52px; font-family: var(--f-head); font-weight: 700; font-size: 18px;
    color: var(--v-txt); background: rgba(255,255,255,.05); border: 1px solid rgba(255,255,255,.16);
    border-radius: 12px; cursor: pointer; }
  .nav-btn:active { background: rgba(255,255,255,.1); }
  .nav-btn:disabled { opacity: .4; }
  .search { display: flex; gap: 10px; }
  .search input { flex: 1; min-height: 48px; padding: 0 16px; font-family: var(--f-body); font-size: 17px;
    color: var(--v-txt); background: var(--v-void); border: 1px solid rgba(255,255,255,.18);
    border-radius: 12px; -webkit-appearance: none; }
  .search input::placeholder { color: var(--v-faint); }
  .search input:focus { outline: none; border-color: rgba(255,176,0,.5); }
  .go { flex: 0 0 auto; min-width: 56px; min-height: 48px; font-family: var(--f-mono); font-weight: 700;
    font-size: 14px; color: var(--v-void); background: var(--v-amber); border: none; border-radius: 12px; cursor: pointer; }
  .go:disabled { opacity: .5; }
  .ctl-err { font-family: var(--f-mono); font-size: 12px; color: var(--v-amber2); }
  .results { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .result { display: flex; flex-direction: column; align-items: flex-start; gap: 3px; width: 100%; text-align: left;
    padding: 12px 14px; background: rgba(255,255,255,.03); border: 1px solid rgba(255,255,255,.1);
    border-radius: 12px; cursor: pointer; }
  .result:active { background: rgba(255,176,0,.1); border-color: rgba(255,176,0,.35); }
  .result:disabled { opacity: .5; }
  .r-ref { font-family: var(--f-mono); font-size: 12px; letter-spacing: .08em; text-transform: uppercase; color: var(--v-amber); }
  .r-text { font-family: var(--f-serif); font-size: 15px; color: var(--v-dim); line-height: 1.35;
    display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
  .no-results { font-family: var(--f-mono); font-size: 12px; color: var(--v-faint); }
</style>
