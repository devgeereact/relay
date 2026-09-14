<script>
  import { formatCountdown, countdownWarning, formatElapsed } from './lib/layers.js';
  import { countdownRemainingMs, countdownIsPaused, countdownTotalMs } from './lib/countdown.js';
  // Mobile stage-display remote — the preacher opens this on a phone/iPad (via
  // QR or the LAN URL) to see the live verse + reference in real time. No Tauri
  // runtime: it connects to the kiosk WebSocket hub (:8031) for content, exactly
  // like an OBS/kiosk output, but rendered as a readable mobile confidence view.
  import { onMount, onDestroy } from 'svelte';

  let content = null;
  let visible = false;
  let note = ''; // operator's confidence-monitor note for the live cue
  // A WORD TO THE PREACHER. Takes the whole screen until the operator clears it.
  // It lives here, in the stage renderer, which is what makes "no congregation
  // screen can show it" a property of the system rather than a promise: the
  // output page has an explicit `false` verdict for this message kind
  // (r6-contracts.test.js).
  let alert = '';
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

  // ── ZONES (docs/REBRAND.md §5) ─────────────────────────────────────────────
  //
  // A stage monitor is not a congregation screen in other colours. Six things a
  // preacher might want, each switchable, and the figures either BESIDE the
  // reading or ACROSS THE BOTTOM.
  //
  // Clean by default: the reading fills the screen with the countdown and the
  // clock beneath it. Everything else is switched on by the person holding the
  // device — which is why this is stored per device, in `localStorage`, and not
  // in Relay's database. Two stage screens in one building are allowed to want
  // different things, and the console must not have to know about either.
  //
  // Every read and write is guarded: a private window, blocked site data or a
  // kiosk with storage disabled must give the DEFAULT layout, never a blank page.
  const ZONES = [
    { key: 'reading', label: 'Reading' },
    { key: 'next', label: 'Next' },
    { key: 'note', label: 'Note' },
    { key: 'countdown', label: 'Countdown' },
    { key: 'clock', label: 'Clock' },
    { key: 'elapsed', label: 'Service elapsed' },
  ];
  const DEFAULT_ZONES = {
    reading: true,
    next: false,
    note: false,
    countdown: true,
    clock: true,
    elapsed: false,
  };
  const ZONE_KEY = 'relay.stage.zones';
  let zones = { ...DEFAULT_ZONES };
  let figures = 'bottom'; // 'bottom' | 'beside'
  let showZones = false;

  function loadZones() {
    try {
      const raw = localStorage.getItem(ZONE_KEY);
      if (!raw) return;
      const saved = JSON.parse(raw);
      if (!saved || typeof saved !== 'object') return;
      // Key by key, off the DEFAULTS — so a zone added in a later version is on
      // its own default rather than absent, and a corrupt value cannot delete one.
      for (const z of ZONES) if (typeof saved[z.key] === 'boolean') zones[z.key] = saved[z.key];
      if (saved.figures === 'beside' || saved.figures === 'bottom') figures = saved.figures;
    } catch {
      /* defaults stand */
    }
  }
  function saveZones() {
    try {
      localStorage.setItem(ZONE_KEY, JSON.stringify({ ...zones, figures }));
    } catch {
      /* the layout still applies to this session */
    }
  }
  function toggleZone(key) {
    zones = { ...zones, [key]: !zones[key] };
    saveZones();
  }
  function setFigures(v) {
    figures = v;
    saveZones();
  }

  // Countdown mirror — ticked by the same 1s timer as the wall clock.
  let cdTo = null;
  let cdFrom = null;
  let cdPaused = null;
  let cdDone = '';
  let svcStart = null; // service-start epoch, for the elapsed zone
  let nowMs = 0;
  // ONE READER, shared with the wall and the console (docs/REBRAND.md §7). This was
  // its own subtraction, which was fine while the answer was a subtraction — and is
  // not, now that it has an exception. A preacher's own screen counting down through
  // a countdown the operator has HELD is the surface it matters most on.
  $: cdContent = { countdown_to: cdTo, countdown_from: cdFrom, countdown_paused_ms: cdPaused };
  $: cdRemain = countdownRemainingMs(cdContent, nowMs);
  $: cdFinished = cdRemain === 0 && !countdownIsPaused(cdContent);
  // ONE FORMATTER, shared with the wall (docs/REBRAND.md §7) — this page used to
  // carry its own copy of the same arithmetic.
  $: cdText = cdRemain == null ? '' : formatCountdown(cdRemain);
  // The span now genuinely rides with the content (`countdown_from`), so the
  // short-countdown half of the warning rule finally has an answer here too.
  $: cdWarn = cdRemain != null && countdownWarning(cdRemain, countdownTotalMs(cdContent));
  // THREE STACKED PAIRS, and no second piece of arithmetic. `hms` is the one
  // formatter's own `H:MM:SS`, split into its fields and the hours padded — so the
  // rail cannot drift from the figure beneath the reading, or from the wall.
  //
  // A FINISHED countdown is one row, not three zeroes: the operator's done message
  // is words, and "00 / 00 / 00" stacked down a rail says the clock is still
  // running. `formatCountdown` clamps at zero, so the pairs would be truthful and
  // useless.
  $: cdPairs =
    cdRemain == null || cdFinished
      ? []
      : formatCountdown(cdRemain, 'hms')
          .split(':')
          .map((p, i) => (i === 0 ? p.padStart(2, '0') : p));

  // SERVICE ELAPSED — counts up from the epoch the fired content carries. There is
  // no epoch when no service is recording, and an absence is shown as an absence:
  // a zero here would say "this service just started", which is a different claim.
  $: elapsedText = svcStart != null ? formatElapsed(nowMs - svcStart) : '';

  // THE COUNTDOWN IS A FIGURE, ALWAYS — it is never a mode the reading disappears
  // into. The first draft of this gave the countdown the reading's room whenever
  // the content had no verse text, which is EVERY countdown: `countdown_to` only
  // ever rides on a `countdown` cue (`main::start_countdown`), never beside a
  // verse. So the rail's stacked pairs — the thing §5 actually asks for — could
  // not be reached from any state Relay can be in. A zone nothing can render is a
  // zone nobody is looking at.
  $: figureList = [
    ...(zones.countdown && cdRemain != null ? ['countdown'] : []),
    ...(zones.clock ? ['clock'] : []),
    ...(zones.elapsed && elapsedText ? ['elapsed'] : []),
  ];
  // How many ROWS the beside-rail holds: a running countdown is three of them, a
  // finished one is a single line of words.
  $: railRows = figureList.reduce(
    (n, f) => n + (f === 'countdown' ? Math.max(1, cdPairs.length) : 1),
    0,
  );
  $: beside = figures === 'beside' && figureList.length > 0 && zones.reading;
  // Has the reading anything of its own to fill the screen with? A countdown cue
  // carries a LABEL and no body, and a pre-service countdown on a phone is the one
  // thing that page is being looked at for — so the figures take the room the
  // reading is not using. Still a flex BASIS, still clipped.
  $: readingHasBody = !!(visible && content?.text);

  function apply(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation };
      note = m.stage_note || '';
      cdTo = m.countdown_to || null;
      cdFrom = m.countdown_from || null;
      cdPaused = m.countdown_paused_ms ?? null;
      cdDone = m.countdown_done || '';
      svcStart = m.service_started_at ?? null;
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
      // Both halves of the countdown go, or a HELD figure survives the control that
      // removed the countdown and the rail keeps showing it: `countdown_paused_ms`
      // is read ahead of the instant, so clearing the instant alone would not be
      // enough. The same trap as `black` above, one field along.
      cdFrom = null;
      cdPaused = null;
      next = null;
      // `svcStart` deliberately SURVIVES. A cleared or blacked wall is not the end
      // of a service, and the elapsed zone is the preacher's own clock — taking it
      // away when the operator hits Esc would answer a question nobody asked.
    } else if (m.kind === 'stage_alert') {
      // `text: null` (or empty) clears it. An alert is an instruction, not a
      // state of the wall, so nothing here is retained or restored on reconnect.
      alert = (m.text || '').trim();
    } else if (m.kind === 'stage_next') {
      next = m.label || m.text ? { label: m.label || '', text: m.text || '' } : null;
    }
  }

  // How many times the socket has failed since it was last up.
  //
  // "connecting…" reads the same at two seconds and at ten minutes, and the
  // preacher holding the phone cannot tell a page that is about to work from one
  // that never will. After a few failed attempts it says so plainly instead.
  let attempts = 0;
  $: reach = connected ? 'live' : attempts > 3 ? "can't reach Relay — retrying" : 'connecting…';

  function connect(host) {
    if (closed) return;
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onopen = () => {
        connected = true;
        attempts = 0;
      };
      ws.onmessage = (e) => {
        try { apply(JSON.parse(e.data)); } catch { /* ignore */ }
      };
      ws.onclose = () => {
        connected = false;
        attempts += 1;
        if (!closed) setTimeout(() => connect(host), 1500);
      };
      ws.onerror = () => { try { ws.close(); } catch { /* onclose retries */ } };
    } catch {
      attempts += 1;
      if (!closed) setTimeout(() => connect(host), 1500);
    }
  }

  onMount(() => {
    loadZones();
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
    <span class="status" class:on={connected}><i></i>{reach}</span>
    <button class="ctl-toggle" class:active={showZones} on:click={() => (showZones = !showZones)} aria-label="Choose what this screen shows">
      Zones
    </button>
    <button class="ctl-toggle" class:active={showCtl} on:click={() => (showCtl = !showCtl)} aria-label="Control panel">
      {showCtl ? 'Done' : 'Control'}
    </button>
  </header>

  {#if alert}
    <!-- THE WHOLE SCREEN. A preacher reads this from a platform, mid-sentence,
         without looking for it. Outside the zone layout on purpose: an
         instruction that a switched-off zone could hide is not an instruction. -->
    <div class="alert" role="status" aria-live="assertive">{alert}</div>
  {/if}

  <!-- ══ ZONES ══ NOTHING MAY LEAVE THE SCREEN (docs/REBRAND.md §5).
       The reading takes what is LEFT (`flex: 1 1 0`); the rows and the rail are
       the only fixed sizes, and they are `flex-basis`, never `height` — a height
       is a floor a long passage pushes past, which is how a clock leaves the top
       of a monitor nobody is standing next to. -->
  <main class="stage" class:beside>
    {#if zones.reading}
      <section class="reading" aria-label="Reading">
        {#if visible && content}
          {#if content.reference}<div class="ref">{content.reference}{content.translation ? ' · ' + content.translation : ''}</div>{/if}
          {#if content.text}<div class="verse">{#if content.reference}“{content.text}”{:else}{content.text}{/if}</div>{/if}
        {:else}
          <div class="idle">— standby —</div>
        {/if}
      </section>
    {/if}

    {#if beside}
      <!-- THE RAIL IS ITS OWN CONTAINER (`container-type: size`), so every figure
           in it is a share of the RAIL and not of the frame. A figure sized
           against the frame is the bug this replaces: it looked right at one rail
           width and overflowed at every other. `--rows` is what keeps the stack
           inside its own height however many zones are switched on. -->
      <aside class="rail" style="--rows:{railRows}" aria-label="Figures">
        {#each figureList as f (f)}
          {#if f === 'countdown'}
            {#if cdFinished}
              <div class="railrow done"><span class="figv">{cdDone || '0:00'}</span></div>
            {:else}
              {#each cdPairs as p, i (i)}
                <div class="railrow" class:warn={cdWarn} style="--ch:{p.length}"><span class="figv">{p}</span></div>
              {/each}
            {/if}
          {:else if f === 'clock'}
            <div class="railrow" style="--ch:{clock.length || 5}"><span class="figv">{clock}</span></div>
          {:else}
            <div class="railrow" style="--ch:{elapsedText.length || 5}"><span class="figv">{elapsedText}</span></div>
          {/if}
        {/each}
      </aside>
    {/if}
  </main>

  {#if !beside && figureList.length}
    <!-- ACROSS THE BOTTOM. Also its own container, for the same reason. -->
    <div class="figrow" class:tall={!readingHasBody} class:only={!zones.reading}
      style="--figs:{figureList.length}" aria-label="Figures">
      {#each figureList as f (f)}
        {@const v = f === 'countdown' ? (cdFinished ? cdDone || '0:00' : cdText) : f === 'clock' ? clock : elapsedText}
        <div class="fig" class:warn={f === 'countdown' && cdWarn} style="--ch:{v.length || 5}">
          <span class="figk">{f === 'countdown' ? 'Countdown' : f === 'clock' ? 'Time' : 'Elapsed'}</span>
          <span class="figv">{v}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if zones.note && note}
    <div class="noterow"><span class="note-lbl">Note</span><span class="notetxt">{note}</span></div>
  {/if}

  {#if showZones}
    <section class="zonepanel" aria-label="Zones">
      <div class="zonegrid">
        {#each ZONES as z (z.key)}
          <button class="zonebtn" class:on={zones[z.key]} aria-pressed={zones[z.key]} on:click={() => toggleZone(z.key)}>
            {z.label}
          </button>
        {/each}
      </div>
      <div class="zonegrid">
        <button class="zonebtn" class:on={figures === 'bottom'} aria-pressed={figures === 'bottom'} on:click={() => setFigures('bottom')}>
          Figures across the bottom
        </button>
        <button class="zonebtn" class:on={figures === 'beside'} aria-pressed={figures === 'beside'} on:click={() => setFigures('beside')}>
          Figures beside the reading
        </button>
      </div>
      <p class="zonefoot">Kept on this device only. Nothing here changes any other screen.</p>
    </section>
  {/if}
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
          aria-label="Search for a verse to put on the screens"
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
  {#if zones.next && next}
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
  /* NOTHING LEAVES THE SCREEN (docs/REBRAND.md §5). The reading takes what is
     left and scrolls INSIDE itself, so the header — the connection state — and
     every row beneath cannot be pushed off by a long passage.

     The spec says clip. The READING scrolls instead, deliberately: on a platform
     monitor the two are the same because the reading is sized to fit, and on the
     preacher's phone, which is the other thing this page is, clipping would take
     the end of a passage away from the person reading it aloud. Everything else
     — the rail, the figure row, the note, the up-next — is clipped as the spec
     asks, because those are fixed-size rows and a fixed row that overflows is
     just a row nobody sized. */
  main.stage { flex: 1 1 0; display: flex; flex-direction: row; min-height: 0; min-width: 0; }
  .reading { flex: 1 1 0; display: flex; flex-direction: column; align-items: center; justify-content: center;
    text-align: center; padding: 24px; gap: 18px; min-height: 0; min-width: 0;
    overflow: auto; overscroll-behavior: contain; }
  /* THE RAIL IS ITS OWN CONTAINER. `size`, not `inline-size`, so the stack can be
     a share of the rail's HEIGHT as well — which is what stops three stacked
     pairs from running off the bottom when the countdown passes an hour. Its
     basis is a share of the frame and is a BASIS, never a height. */
  .rail { flex: 0 0 26%; max-width: 26%; min-width: 0; min-height: 0; overflow: hidden;
    container-type: size; display: flex; flex-direction: column;
    border-left: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  /* A FIGURE ON THIS RAIL WEARS NO PROMISE COLOUR. The clock, the elapsed time
     and the countdown were all `--v-amber`, and amber on this console means ON
     AIR and nothing else (rule 18, DECISIONS §21, `colourlaw.test.js`). Driven
     against the real backend, `stage.html` rendered "— standby —" with an amber
     clock beside it: the ON AIR colour, at the largest size on the page, over a
     page with nothing on air. It is not a template's saved default and no
     operator chose it — this page renders no template at all, so the colour was
     a stylesheet literal and nothing else.
     NOT the prototype's `--stg-mc` cyan either: cyan on this console promises A
     GUESS, and swapping one promise for another is the same defect in a
     different hue. A clock is a fact about time, so it takes the page's own ink.
     `--v-txt` is also BRIGHTER than amber on `--v-void`, so the figure a
     preacher reads from a platform did not get quieter. */
  .railrow { flex: 1 1 0; min-height: 0; overflow: hidden; display: grid; place-items: center;
    font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    color: var(--v-txt); line-height: 1; letter-spacing: .01em;
    /* A share of the rail in BOTH axes: wide enough to fill it, never taller than
       its own share of the stack. `--rows` counts the rows actually switched on and
       `--ch` how many characters this row holds.
       `--ch` IS LOAD-BEARING, and leaving it out was a real defect measured in a
       browser: `62cqw` fills a rail with a TWO-character pair and puts a clock
       ("02:14 AM", eight characters) at 205px in a 333px rail — clipped to about a
       character and a half, silently, because the row is `overflow: hidden`. The
       0.62 advance is the mono figure docs/REBRAND.md §3.4 already measured. */
    font-size: min(
      calc(88cqw / (var(--ch, 2) * 0.62)),
      calc(78cqh / var(--rows, 3))
    ); }
  .railrow + .railrow { border-top: 1px solid rgba(255,255,255,.06); }
  .railrow.warn { color: var(--v-red); }
  /* A finished countdown is the operator's own words, not a figure — prose, at a
     size that still fits the rail it is a share of. */
  .railrow.done { font-family: var(--f-body); letter-spacing: 0; line-height: 1.15;
    padding: 0 6cqw; text-align: center;
    font-size: min(16cqw, calc(70cqh / var(--rows, 1))); }
  /* ACROSS THE BOTTOM — a fixed BASIS, clipped, never a height. */
  .figrow { flex: 0 0 20%; min-height: 0; overflow: hidden; container-type: size;
    display: flex; border-top: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.02); }
  /* A pre-service countdown is the whole reason anyone is looking at this page, and
     a countdown cue has a label and no body. The figures take the room the reading
     is not using — a different BASIS, never a height, and still clipped. */
  .figrow.tall { flex-basis: 58%; }
  .figrow.only { flex: 1 1 0; }
  .fig { flex: 1 1 0; min-width: 0; min-height: 0; overflow: hidden;
    display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; }
  .fig + .fig { border-left: 1px solid rgba(255,255,255,.06); }
  .figk { font-family: var(--f-mono); font-size: 9px; font-weight: 700; letter-spacing: .16em;
    text-transform: uppercase; color: var(--v-faint); }
  /* The bottom row is the same three figures in the other layout. Same rule. */
  .fig .figv { font-family: var(--f-mono); font-variant-numeric: tabular-nums; font-weight: 700;
    color: var(--v-txt); line-height: 1;
    /* Same rule as the rail: the width a figure may take is its share of the row
       divided by the characters it actually has. */
    font-size: min(
      calc(92cqw / var(--figs, 1) / (var(--ch, 5) * 0.62)),
      58cqh
    ); }
  .fig.warn .figv { color: var(--v-red); }
  @media (prefers-reduced-motion: no-preference) {
    .fig.warn .figv, .railrow.warn { animation: cdwarn 2s ease-in-out infinite; }
  }
  .noterow { flex: 0 0 auto; flex-basis: auto; max-height: 22%; overflow: hidden;
    display: flex; align-items: baseline; gap: 10px; padding: 10px 18px;
    border-top: 1px solid rgba(255,176,0,.24); background: rgba(255,176,0,.08); color: var(--v-amber2);
    font-family: var(--f-body); font-size: clamp(14px, 2.6vw, 20px); line-height: 1.3; }
  .notetxt { min-width: 0; overflow: hidden; }
  /* The zone panel — one instrument, no native dialog (rule 41). */
  .zonepanel { flex: 0 0 auto; max-height: 46dvh; overflow-y: auto; padding: 14px 18px;
    display: flex; flex-direction: column; gap: 10px;
    border-top: 1px solid rgba(255,255,255,.1); background: rgba(255,255,255,.03); }
  .zonegrid { display: flex; flex-wrap: wrap; gap: 8px; }
  .zonebtn { flex: 1 1 auto; min-height: 44px; padding: 0 14px; cursor: pointer;
    font-family: var(--f-mono); font-size: 12px; font-weight: 700; letter-spacing: .08em;
    color: var(--v-dim); background: rgba(255,255,255,.04);
    border: 1px solid rgba(255,255,255,.14); border-radius: 8px; }
  .zonebtn.on { color: var(--v-amber); border-color: rgba(255,176,0,.45); background: rgba(255,176,0,.1); }
  .zonefoot { margin: 0; font-family: var(--f-mono); font-size: 11px; color: var(--v-faint); }
  .ref { font-family: var(--f-mono); font-size: clamp(13px, 3.5vw, 20px); letter-spacing: .18em; text-transform: uppercase; color: var(--v-amber); }
  .verse { font-family: var(--f-serif); font-size: clamp(26px, 7vw, 64px); line-height: 1.28; color: var(--v-txt); max-width: 16ch; }
  /* The DEFAULT resting state of the preacher's phone — the thing on screen before
     anything is fired, and therefore the text most likely to be looked at. It was
     2.25:1: the worst contrast in the product, in its least forgiving location. */
  .idle { font-family: var(--f-mono); color: var(--v-faint); font-size: 14px; letter-spacing: .1em; }
  /* The last minute — the same rule and the same red as the wall. Reduced motion
     gets a glow instead of a pulse; the colour is the same either way. */
  @media (prefers-reduced-motion: reduce) {
    .fig.warn .figv, .railrow.warn { text-shadow: 0 0 .25em rgba(244, 81, 91, .85); }
  }
  @keyframes cdwarn { 0%, 100% { opacity: 1; } 50% { opacity: .55; } }
  /* Operator's cue note — confidence-monitor only, never on the main output. */
  /* A WORD TO THE PREACHER — docs/REBRAND.md §5. The pulse is the point: a
     platform is a bright place and a flat red panel reads as part of the set. */
  .alert {
    /* FIXED, and above everything. This is read by somebody mid-sentence in front
       of a congregation; it does not share the screen with a clock. */
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 4cqw;
    text-align: center;
    font-family: var(--f-body);
    font-weight: 700;
    font-size: 8.5cqw;
    line-height: 1.15;
    color: #fff;
    text-shadow: 0 0.02em 0.06em rgba(0, 0, 0, 0.75);
    background: #c8121c;
    overflow: hidden;
  }
  @media (prefers-reduced-motion: no-preference) {
    .alert { animation: stagealert 1.4s ease-in-out infinite; }
  }
  @keyframes stagealert {
    0%, 100% { background: #c8121c; }
    50% { background: #7a0a11; }
  }
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
