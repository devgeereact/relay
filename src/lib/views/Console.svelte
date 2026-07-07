<script>
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import {
    capture,
    transcript,
    detections,
    templates,
    live,
    loadTemplates,
    listActiveTemplates,
    confirmDetection,
    dismissDetection,
    manualFire,
    openOutput,
    clearScreens,
    blackScreen,
    screenBlack,
    startCountdown,
    countdownRunning,
    setDetection,
    startCapture,
    stopCapture,
    navVerse,
  } from '../stores/capture.js';

  // Operator drives detection from the console: Listen = mic on (auto-drive
  // when AI detection is also armed). Errors surface, never freeze.
  let listenBusy = false;

  // Pre-service countdown — outputs tick MM:SS locally from a broadcast target.
  // Accidental-fire guard: the Start button ARMS on the first click ("Confirm?")
  // and only fires on the second (auto-disarms after 3s). No native confirm()
  // dialog — Tauri's webview doesn't reliably implement it.
  let cdMin = 5;
  let cdArmed = false;
  let cdArmT;
  async function beginCountdown() {
    if (countdownRunning()) {
      flash('A countdown is already running — clear the screen first');
      return;
    }
    if (!cdArmed) {
      cdArmed = true;
      clearTimeout(cdArmT);
      cdArmT = setTimeout(() => (cdArmed = false), 3000);
      return;
    }
    clearTimeout(cdArmT);
    cdArmed = false;
    const m = Number(cdMin) || 5;
    try {
      await startCountdown(m);
      flash(`Countdown started — ${m} min`);
    } catch (e) {
      flash(String(e));
    }
  }
  async function toggleListen() {
    listenBusy = true;
    try {
      if ($capture.capturing) await stopCapture();
      else await startCapture($capture.inputDevice || null);
    } catch {
      /* surfaced via audioError */
    }
    listenBusy = false;
  }

  let searchEl;
  let transcriptEl;
  $: dets = $detections; // pending suggestions

  // Auto-scroll the transcript to the latest word. afterUpdate is the correct
  // Svelte hook for DOM side-effects — calling tick() inside a reactive `$:`
  // block re-entrantly is a known infinite-loop trap (it froze the webview).
  afterUpdate(() => {
    if (transcriptEl) transcriptEl.scrollTop = transcriptEl.scrollHeight;
  });

  // Operator keyboard controls — always reachable (CLAUDE.md). Esc works even
  // while typing; the rest yield to text fields.
  function onKey(e) {
    const typing = e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA');
    if (e.key === 'Escape') {
      clearScreens();
      flash('Screens cleared');
      if (typing) e.target.blur();
      return;
    }
    if (typing) return;
    if (e.key === '/') {
      e.preventDefault();
      searchEl?.focus();
    } else if (e.key === ' ') {
      if (dets[0]) {
        e.preventDefault();
        confirmDetection(dets[0].reference);
        flash(`Now live: ${dets[0].reference}`);
      }
    }
  }
  onMount(async () => {
    await loadTemplates();
    await loadActive();
    window.addEventListener('keydown', onKey);
  });
  onDestroy(() => window.removeEventListener('keydown', onKey));

  async function openMainOutput() {
    const id = $templates.find((t) => t.name === 'Classic Serif')?.id ?? $templates[0]?.id;
    if (!id) {
      manualError = 'No templates yet — create one in the Templates tab first.';
      return;
    }
    try {
      await openOutput(id, 'Main screen');
      flash(`Output window opened`);
    } catch {
      manualError = 'Could not open the output window (needs the desktop app).';
    }
  }

  let manualRef = '';
  let manualError = '';
  let liveMsg = ''; // brief "now live" confirmation
  let liveMsgT;
  function flash(msg) {
    liveMsg = msg;
    clearTimeout(liveMsgT);
    liveMsgT = setTimeout(() => (liveMsg = ''), 2600);
  }
  // Turn a raw backend error into a plain sentence for a live operator.
  function humanError(e) {
    const s = String(e).replace(/^Error:\s*/, '');
    if (/could not parse|parse a reference/i.test(s)) return `Couldn't read "${manualRef.trim()}" as a scripture reference.`;
    return s;
  }
  async function fireManual() {
    const ref = manualRef.trim();
    if (!ref) return;
    try {
      await manualFire(ref);
      flash(`Now live: ${ref}`);
      manualRef = '';
      manualError = '';
    } catch (e) {
      manualError = humanError(e);
    }
  }

  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

  // Console Output = the ACTIVE template STYLES (max 4), each previewing the
  // live content in its own style, through the SAME TemplateRender as the real
  // output = true WYSIWYG. Which 4 are active is chosen in the Templates tab.
  let activeTpls = [];
  async function loadActive() {
    try {
      activeTpls = await listActiveTemplates();
    } catch {
      activeTpls = [];
    }
  }
  const ACCENTS = ['gold', 'cyan', 'amethyst', 'rose'];
  // What's currently on the screens, shaped for TemplateRender.
  $: liveContent = $live
    ? { reference: $live.reference, text: $live.text, translation: $live.translation, media_url: $live.media_url, media_kind: $live.media_kind, countdown_to: $live.countdown_to, countdown_done: $live.countdown_done }
    : null;
  // Per-content-type template override carried by the live content, if any.
  $: overrideTpl = (() => {
    if ($live && $live.template_json) {
      try {
        return JSON.parse($live.template_json);
      } catch {
        /* ignore */
      }
    }
    return null;
  })();
</script>

<div class="stx">
  <div class="stx-top">
    <!-- ── Intelligence Feed ─────────────────────────── -->
    <section class="tile feed">
      <div class="tile-head">
        <h3>Intelligence Feed</h3>
        <svg class="ic dim" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M3 3v18h18"/><path d="M18 17V9M13 17V5M8 17v-3"/></svg>
      </div>
      <!-- Live transcript — its own fixed box, auto-scrolls to the latest word -->
      <div class="tx-box">
        <div class="seg-top">
          <span class="lbl-gold">Live transcript</span>
          <span class="mono dim">{$capture.capturing ? ($capture.detectedLang ?? 'listening') : 'standby'}</span>
        </div>
        <div class="tx-stream" bind:this={transcriptEl}>
          {#if hasTranscript}
            {$transcript.finals.join(' ')}
            {#if $transcript.partial}<mark>{$transcript.partial}</mark><i class="caret"></i>{/if}
          {:else if $capture.capturing}
            <span class="dim">Waiting for speech…</span>
          {:else if !$capture.stt.loaded}
            <span class="dim">No speech model loaded — see Settings. Manual override still works.</span>
          {:else}
            <span class="dim">Start listening to transcribe live.</span>
          {/if}
        </div>
      </div>

      <!-- Suggestions — the scrollable pick-list -->
      <div class="feed-body">
        <!-- AI suggestion (top) -->
        {#if dets.length}
          {@const d = dets[0]}
          <div class="ai-card">
            <div class="ai-top">
              <span class="lbl-gold">AI suggestion</span>
              <span class="mono gold">{Math.round(d.confidence * 100)}% match</span>
            </div>
            <div class="ai-ref">{d.reference}</div>
            {#if d.text}<div class="ai-verse">“{d.text}”</div>{/if}
            <div class="ai-acts">
              <button class="btn-gold" on:click={() => { confirmDetection(d.reference); flash(`Now live: ${d.reference}`); }}>Push to stage</button>
              <button class="btn-x" title="Dismiss" aria-label="Dismiss suggestion" on:click={() => dismissDetection(d.reference)}>Dismiss</button>
            </div>
          </div>

          <!-- Further suggestions / cross references -->
          {#each dets.slice(1) as x (x.reference + x.at)}
            <div class="xref">
              <div class="xref-top"><span class="lbl-dim">Cross reference</span><span class="mono dim">{Math.round(x.confidence * 100)}%</span></div>
              <div class="xref-ref">{x.reference}</div>
              {#if x.text}<div class="xref-verse">“{x.text}”</div>{/if}
              <div class="xref-acts">
                <button class="btn-mini" on:click={() => confirmDetection(x.reference)}>Push</button>
                <button class="btn-mini ghost" on:click={() => dismissDetection(x.reference)}>Dismiss</button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="empty">
            {#if !$capture.detectionOn}Detection is off — manual override still fires.{:else}No suggestions yet.{/if}
          </div>
        {/if}
      </div>
    </section>

    <!-- ── Output Channels ───────────────────────────── -->
    <section class="tile channels">
      <div class="tile-head">
        <h2>Output</h2>
        <span class="mono dim">{activeTpls.length}/4 styles{$live ? ' · live' : ''}</span>
      </div>
      <div class="chan-grid">
        {#if activeTpls.length}
          {#each activeTpls as tpl, i (tpl.id)}
            {@const acc = ACCENTS[i % ACCENTS.length]}
            <div class="mon a-{acc}" class:on={$live}>
              <!-- Each monitor = one active template style, live content -->
              <div class="tpl"><TemplateRender template={overrideTpl ?? tpl} content={liveContent} /></div>
              {#if $screenBlack}<div class="mon-black"></div>{/if}

              <span class="mon-badge b-{acc}">{$live ? 'Live' : 'Style'} · {tpl.name}</span>

              <div class="mon-foot">
                {#if $live}
                  <span class="mono">{$live.reference}{$live.translation ? ' · ' + $live.translation : ''}</span>
                {:else}
                  <span class="mono dim tiny">{tpl.name}</span>
                {/if}
                {#if i === 0 && $live}
                  <span class="mon-nav">
                    <button class="nav-btn" title="Previous verse" aria-label="Previous verse" on:click={() => navVerse('previous')}>
                      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
                    </button>
                    <button class="nav-btn" title="Next verse" aria-label="Next verse" on:click={() => navVerse('next')}>
                      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
                    </button>
                  </span>
                {/if}
              </div>
            </div>
          {/each}
        {:else}
          <div class="chan-empty">No active styles — activate up to 4 templates in the <b>Templates</b> tab.</div>
        {/if}
      </div>
    </section>

    </div><!-- /stx-top -->

    <!-- ── Command bar ───────────────────────────────── -->
    <section class="tile entry">
      <div class="entry-row">
        <div class="search">
          <svg class="ic dim" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
          <input
            bind:this={searchEl}
            bind:value={manualRef}
            on:keydown={(e) => e.key === 'Enter' && fireManual()}
            placeholder="Search scripture or commands — ps 23, John 3:16-18"
            disabled={!$capture.available}
          />
        </div>
        <button class="btn-gold lg" on:click={fireManual} disabled={!$capture.available}>Push to stage</button>
      </div>
      {#if manualError}<div class="err">{manualError}</div>{/if}
      {#if liveMsg}<div class="livemsg"><span class="lm-dot"></span>{liveMsg}</div>{/if}

      <div class="entry-controls">
        <button class="ctl" class:rec={$capture.capturing} on:click={toggleListen}
          disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
          <span class="dot" style="background:{$capture.capturing ? 'var(--s-rose)' : 'var(--s-gold)'};"></span>
          {$capture.capturing ? 'Listening — Stop' : listenBusy ? 'Starting…' : 'Start listening'}
        </button>
        <button class="ctl" on:click={() => setDetection(!$capture.detectionOn)} disabled={!$capture.available}>
          <span class="dot" style="background:{$capture.detectionOn ? '#10b981' : 'var(--s-outline)'};"></span>
          Detection {$capture.detectionOn ? 'active' : 'off'}
        </button>
        <button class="ctl" on:click={() => { clearScreens(); flash('Screens cleared'); }} disabled={!$capture.available}>
          <span class="dot" style="background:var(--s-outline);"></span>Clear all<span class="ctl-k">Esc</span>
        </button>
        <button class="ctl" class:rec={$screenBlack} on:click={() => { blackScreen(); flash('Blackout'); }} disabled={!$capture.available}>
          <span class="dot" style="background:{$screenBlack ? 'var(--s-rose)' : '#000'};border:1px solid var(--s-outline)"></span>Black
        </button>
        <div class="ctl cd-ctl">
          <span class="dot" style="background:var(--s-cyan);"></span>Countdown
          <input class="cd-min" type="number" min="1" max="120" bind:value={cdMin} aria-label="Countdown minutes" disabled={!$capture.available} />
          <span class="cd-unit r-mono">min</span>
          <button class="cd-go" class:armed={cdArmed} on:click={beginCountdown} disabled={!$capture.available}>{cdArmed ? 'Confirm?' : 'Start'}</button>
        </div>
        <button class="ctl" on:click={openMainOutput} disabled={!$capture.available}>
          <span class="dot" style="background:var(--s-gold);"></span>Open output
        </button>
        <div class="hints">
          <span class="hint"><kbd>Space</kbd> push top</span>
          <span class="hint"><kbd>/</kbd> search</span>
        </div>
      </div>
    </section>

  {#if $capture.audioError}
    <div class="audioerr">Audio: {$capture.audioError}</div>
  {/if}
</div>

<style>
  /* Console — "Spiritual High-Tech". The local --s-* names now alias the global
     --v-* design tokens (src/app.css) wherever they match, so the Console shares
     one palette with the rest of the app. A few Console-specific tones (elevation
     steps above surf3, the salmon rose, glows without a --v- equivalent) stay as
     literals — appearance is preserved, the shared tokens are deduplicated. */
  .stx{
    --s-bg:var(--v-surf); --s-lowest:var(--v-bg); --s-low:var(--v-surf2); --s-cont:var(--v-surf3); --s-high:#2a2a2b; --s-var:#353436;
    --s-on:var(--v-txt); --s-onvar:#c8c6ca; --s-outline:#8b8a8e; --s-outvar:#47464a;
    --s-gold:var(--v-amber); --s-ongold:var(--v-amber-ink); --s-goldc:#ee9800; --s-gold-glow:var(--v-amber-glow);
    --s-cyan:var(--v-cyan); --s-cyan-glow:rgba(0,133,190,.42);
    --s-amethyst:var(--v-amethyst); --s-amethyst-glow:rgba(168,85,247,.38);
    --s-rose:var(--v-rose); --s-rose-glow:rgba(244,113,139,.42);
    --hair:rgba(255,255,255,.08); --hair2:rgba(255,255,255,.12);
    --f-serif:var(--f-serif); --f-ui:var(--f-body); --f-mono:var(--f-mono);
    color:var(--s-on);font-family:var(--f-body);
    /* Fill the scroll area exactly — the whole console stays fixed on screen;
       ONLY the Intelligence Feed scrolls internally. */
    height:100%;display:flex;flex-direction:column;gap:14px;min-height:0;
  }
  .stx .mono{font-family:var(--f-mono);font-variant-numeric:tabular-nums;letter-spacing:.04em}
  .stx .dim{color:var(--s-onvar)}
  .stx .gold{color:var(--s-gold)}
  .stx .tiny{font-size:9px}

  /* Top row (feed + output) fills the remaining height; command bar pinned below. */
  .stx-top{flex:1;min-height:0;display:grid;grid-template-columns:360px minmax(0,1fr);gap:14px}

  .tile{background:var(--s-low);border:1px solid var(--hair2);border-radius:12px;overflow:hidden;
    display:flex;flex-direction:column;min-height:0}
  .tile.feed{background:var(--s-lowest)}
  .tile.entry{flex:0 0 auto}
  @media (max-width:1040px){
    .stx{height:auto}
    .stx-top{grid-template-columns:1fr}
  }

  .tile-head{display:flex;align-items:center;justify-content:space-between;padding:14px 16px;
    border-bottom:1px solid var(--hair);flex:0 0 auto}
  .tile-head h2,.tile-head h3{font-family:var(--f-ui);font-size:11px;font-weight:700;letter-spacing:.16em;
    text-transform:uppercase;color:var(--s-on)}
  .tile-head h3{color:var(--s-onvar)}
  .ic{display:block}

  /* Feed */
  /* Live transcript: fixed box at the top of the feed, scrolls internally so the
     latest word is always visible without pushing the suggestions around. */
  .tx-box{flex:0 0 auto;padding:14px 16px 12px;border-bottom:1px solid var(--hair);background:var(--s-lowest)}
  .tx-stream{height:92px;overflow-y:auto;margin-top:9px;font-size:14px;line-height:1.6;color:var(--s-on);font-weight:500;
    scrollbar-width:thin;scrollbar-color:var(--s-high) transparent}
  .tx-stream::-webkit-scrollbar{width:6px}
  .tx-stream::-webkit-scrollbar-thumb{background:var(--s-high);border-radius:99px}
  .tx-stream mark{background:rgba(255,185,95,.16);color:var(--s-gold);border-radius:3px;padding:0 2px}

  .feed-body{flex:1;min-height:0;overflow-y:auto;padding:16px;display:flex;flex-direction:column;gap:16px}
  .seg-top{display:flex;align-items:center;justify-content:space-between}
  .lbl-gold{font-family:var(--f-mono);font-size:10px;font-weight:600;letter-spacing:.16em;text-transform:uppercase;color:var(--s-gold)}
  .lbl-dim{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.16em;text-transform:uppercase;color:var(--s-onvar)}
  .transcript{font-size:14px;line-height:1.6;color:var(--s-on);font-weight:500}
  .transcript mark{background:rgba(255,185,95,.16);color:var(--s-gold);border-radius:3px;padding:0 2px}
  .caret{display:inline-block;width:2px;height:14px;background:var(--s-gold);vertical-align:-2px;margin-left:1px;animation:blink 1.05s steps(1) infinite}
  @keyframes blink{50%{opacity:0}}

  .ai-card{background:var(--s-low);border:1px solid rgba(255,185,95,.28);border-radius:10px;padding:14px;
    box-shadow:0 0 20px -5px var(--s-gold-glow)}
  .ai-top{display:flex;align-items:center;justify-content:space-between;margin-bottom:10px}
  .ai-ref{font-family:var(--f-serif);font-size:19px;font-weight:600;letter-spacing:-.01em;color:var(--s-on)}
  .ai-verse{font-family:var(--f-serif);font-style:italic;font-size:13.5px;line-height:1.5;color:var(--s-onvar);margin:6px 0 13px}
  .ai-acts{display:flex;gap:8px;align-items:center}

  .xref{background:rgba(28,27,28,.6);border:1px solid var(--hair);border-radius:10px;padding:13px}
  .xref-top{display:flex;align-items:center;justify-content:space-between;margin-bottom:7px}
  .xref-ref{font-family:var(--f-serif);font-size:15px;font-weight:600;color:var(--s-on)}
  .xref-verse{font-family:var(--f-serif);font-style:italic;font-size:12.5px;color:var(--s-onvar);margin-top:4px;line-height:1.5}
  .xref-acts{display:flex;gap:7px;margin-top:11px}

  .empty{color:var(--s-outline);font-size:13px;padding:6px 2px}

  /* Buttons */
  .btn-gold{padding:9px 16px;border-radius:8px;border:0;cursor:pointer;font-family:var(--f-ui);
    font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--s-ongold);
    background:var(--s-gold);transition:.14s}
  .btn-gold:hover:not(:disabled){filter:brightness(1.06)}
  .btn-gold.lg{padding:0 22px;height:42px;font-size:12px;flex:0 0 auto}
  .btn-gold:disabled{opacity:.45;cursor:not-allowed}
  .btn-x{padding:8px 13px;border-radius:8px;background:transparent;border:1px solid var(--hair2);
    color:var(--s-onvar);font-family:var(--f-ui);font-size:11px;cursor:pointer;transition:.14s}
  .btn-x:hover{border-color:var(--s-rose);color:var(--s-rose)}
  .btn-mini{padding:5px 11px;border-radius:6px;border:0;cursor:pointer;font-family:var(--f-ui);font-size:11px;font-weight:600;
    color:var(--s-ongold);background:var(--s-gold);transition:.14s}
  .btn-mini:hover{filter:brightness(1.06)}
  .btn-mini.ghost{background:transparent;border:1px solid var(--hair2);color:var(--s-onvar)}
  .btn-mini.ghost:hover{border-color:var(--s-outline);color:var(--s-on)}

  /* Output channels */
  .chan-grid{flex:1;display:grid;grid-template-columns:1fr 1fr;gap:18px;padding:22px;overflow-y:auto;align-content:start}
  .mon{position:relative;aspect-ratio:16/9;border-radius:14px;overflow:hidden;background:#000;
    border:1px solid var(--hair2);transition:.18s}
  .mon-scrim{position:absolute;inset:0;background:
    radial-gradient(120% 130% at 50% 12%,rgba(255,255,255,.03),transparent 60%),
    linear-gradient(to top,rgba(0,0,0,.85),transparent 55%),
    radial-gradient(130% 120% at 50% 30%,#1a1815,#0a0a0b)}
  .mon.on.a-gold{border:2px solid var(--s-gold);box-shadow:0 0 26px -6px var(--s-gold-glow)}
  .mon.on.a-cyan{border:1px solid rgba(63,182,230,.5);box-shadow:0 0 26px -8px var(--s-cyan-glow)}
  .mon.on.a-amethyst{border:1px solid rgba(192,139,255,.45);box-shadow:0 0 26px -8px var(--s-amethyst-glow)}
  .mon.on.a-rose{border:1px solid rgba(255,157,148,.45);box-shadow:0 0 26px -8px var(--s-rose-glow)}
  .mon-badge{position:absolute;top:11px;left:11px;z-index:2;padding:3px 9px;border-radius:6px;
    font-family:var(--f-ui);font-size:9px;font-weight:700;letter-spacing:.09em;text-transform:uppercase}
  .b-gold{background:var(--s-gold);color:var(--s-ongold)}
  .b-cyan{background:var(--s-cyan);color:#06222e}
  .b-amethyst{background:var(--s-amethyst);color:#2a0d45}
  .b-rose{background:var(--s-rose);color:#3d0a08}
  .mon:not(.on) .mon-badge{background:var(--s-high);color:var(--s-onvar)}
  .mon-body{position:absolute;inset:0;z-index:1;display:flex;flex-direction:column;align-items:center;justify-content:center;padding:14px;text-align:center}
  .mon-verse{font-family:var(--f-serif);font-style:italic;font-size:13px;line-height:1.4;color:#efe9df;max-width:90%}
  .mon-clock{font-family:var(--f-mono);font-size:30px;font-weight:700;letter-spacing:-.02em;color:var(--s-cyan)}
  .mon-sub{font-size:9px;letter-spacing:.16em;text-transform:uppercase;color:var(--s-onvar);margin-top:4px}
  .mon-off{color:var(--s-outline);font-family:var(--f-mono);font-size:11px}
  .tpl{position:absolute;inset:0;overflow:hidden;background:#0a0a0b;border-radius:inherit}
  .mon-black{position:absolute;inset:0;z-index:3;background:#000;border-radius:inherit}
  .tpl-none{position:absolute;inset:0;display:grid;place-items:center;font-family:var(--f-mono);font-size:10px;color:var(--s-outline)}
  .chan-empty{grid-column:1 / -1;color:var(--s-outline);font-size:13px;line-height:1.6;padding:22px;text-align:center;
    border:1px dashed var(--hair2);border-radius:12px}
  .chan-empty b{color:var(--s-onvar)}
  .mon-foot{position:absolute;left:0;right:0;bottom:0;z-index:2;display:flex;align-items:center;justify-content:space-between;
    gap:8px;padding:9px 11px;background:linear-gradient(to top,rgba(0,0,0,.72),transparent)}
  .mon-foot .mono{font-size:10px;color:#fff;text-shadow:0 1px 3px rgba(0,0,0,.7)}
  .mon-nav{display:flex;gap:6px}
  .nav-btn{width:24px;height:24px;border-radius:6px;display:grid;place-items:center;cursor:pointer;
    background:rgba(0,0,0,.5);border:1px solid var(--hair2);color:#fff;transition:.14s}
  .nav-btn:hover{background:rgba(0,0,0,.7);border-color:var(--s-gold);color:var(--s-gold)}

  /* Command bar */
  .entry-row{display:flex;gap:12px;padding:14px 16px;border-bottom:1px solid var(--hair)}
  .search{flex:1;display:flex;align-items:center;gap:11px;background:var(--s-lowest);
    border:1px solid var(--hair);border-radius:10px;padding:0 14px;height:42px}
  .search input{flex:1;background:transparent;border:0;outline:none;color:var(--s-on);
    font-family:var(--f-mono);font-size:12.5px}
  .search input::placeholder{color:var(--s-outline)}
  .search:focus-within{border-color:rgba(255,185,95,.4);box-shadow:0 0 0 3px rgba(255,185,95,.08)}
  .err{color:var(--s-rose);font-size:11.5px;padding:0 16px;margin-top:8px}
  .livemsg{display:flex;align-items:center;gap:8px;color:#10b981;font-size:11.5px;padding:0 16px;margin-top:8px;font-weight:500}
  .lm-dot{width:7px;height:7px;border-radius:50%;background:#10b981;box-shadow:0 0 8px #10b981}

  .entry-controls{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px 16px}
  .ctl{display:flex;align-items:center;gap:9px;padding:9px 13px;border-radius:9px;background:var(--s-cont);
    border:1px solid var(--hair);color:var(--s-on);font-family:var(--f-ui);font-size:12px;cursor:pointer;transition:.14s}
  .ctl:hover:not(:disabled){background:var(--s-high);border-color:var(--hair2)}
  .ctl:disabled{opacity:.45;cursor:not-allowed}
  .ctl.rec{background:rgba(255,120,110,.12);border-color:rgba(255,120,110,.35);color:var(--s-rose)}
  .dot{width:8px;height:8px;border-radius:50%;flex:0 0 auto}
  /* countdown control — an input + Start, styled to sit inside the .ctl chip */
  .cd-ctl{cursor:default;gap:7px}
  .cd-min{width:46px;padding:3px 6px;border-radius:6px;border:1px solid var(--hair2);background:var(--s-bg);
    color:var(--s-on);font-family:var(--f-mono);font-size:12px;text-align:center}
  .cd-unit{font-size:9px;color:var(--s-outline);margin-left:-3px}
  .cd-go{padding:4px 11px;border-radius:6px;border:1px solid rgba(63,182,230,.4);background:rgba(63,182,230,.14);
    color:var(--s-cyan);font-family:var(--f-mono);font-size:10px;font-weight:700;letter-spacing:.04em;cursor:pointer;transition:.12s}
  .cd-go:hover:not(:disabled){background:rgba(63,182,230,.26)}
  .cd-go:disabled{opacity:.45;cursor:not-allowed}
  .cd-go.armed{background:rgba(245,166,35,.2);border-color:rgba(245,166,35,.5);color:var(--s-gold)}
  .ctl-k{font-family:var(--f-mono);font-size:9px;color:var(--s-outline);margin-left:2px}
  .hints{display:flex;gap:14px;margin-left:auto}
  .hint{display:flex;align-items:center;gap:6px;font-size:11px;color:var(--s-outline)}
  .hint kbd{font-family:var(--f-mono);font-size:9px;color:var(--s-onvar);background:var(--s-high);
    border:1px solid var(--hair2);border-bottom-width:2px;border-radius:4px;padding:2px 6px}

  .audioerr{margin-top:12px;background:rgba(147,0,10,.18);color:var(--s-rose);
    border:1px solid rgba(255,157,148,.3);border-radius:9px;padding:9px 12px;font-size:12px}

  /* Footer */
  .stx-foot{display:flex;align-items:center;justify-content:space-between;margin-top:12px;
    padding:11px 16px;border-top:1px solid var(--hair);color:var(--s-onvar)}
  .foot-l{display:flex;align-items:center;gap:16px}
  .brand{font-family:var(--f-ui);font-size:11px;font-weight:700;color:var(--s-on)}
  .foot-l .mono{font-size:10px}
  .sys{display:flex;align-items:center;gap:7px;font-family:var(--f-mono);font-size:10px;font-weight:700;
    letter-spacing:.1em;color:var(--s-outline)}
  .sys.live{color:var(--s-rose)}
  .sys-dot{width:6px;height:6px;border-radius:50%;background:currentColor}
  .sys.live .sys-dot{box-shadow:0 0 8px currentColor;animation:pulse 1.7s ease-in-out infinite}
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}

  /* Accessibility */
  .btn-gold:focus-visible,.btn-x:focus-visible,.btn-mini:focus-visible,.ctl:focus-visible,
  .nav-btn:focus-visible,.search:focus-within{outline:2px solid var(--s-gold);outline-offset:2px}
  @media (prefers-reduced-motion:reduce){.caret,.sys.live .sys-dot{animation:none}}
</style>
