<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import {
    capture,
    transcript,
    detections,
    templates,
    live,
    loadTemplates,
    confirmDetection,
    dismissDetection,
    manualFire,
    openOutput,
    clearScreens,
    setDetection,
    startCapture,
    stopCapture,
  } from '../stores/capture.js';

  // Operator drives detection from the console: Listen = mic on (auto-drive
  // when AI detection is also armed). Errors surface, never freeze.
  let listenBusy = false;
  async function toggleListen() {
    listenBusy = true;
    try {
      if ($capture.capturing) await stopCapture();
      else await startCapture(null);
    } catch {
      /* surfaced via audioError */
    }
    listenBusy = false;
  }

  let searchEl;
  let transcriptEl;
  $: dets = $detections; // pending suggestions

  // Auto-scroll the transcript to the latest word as it streams in.
  $: if (transcriptEl && ($transcript.finals.length || $transcript.partial)) {
    tick().then(() => {
      transcriptEl.scrollTop = transcriptEl.scrollHeight;
    });
  }

  // Operator keyboard controls — always reachable (CLAUDE.md). Esc works even
  // while typing; the rest yield to text fields.
  function onKey(e) {
    const typing = e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA');
    if (e.key === 'Escape') {
      clearScreens();
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
      }
    } else if (e.key === 'z' || e.key === 'Z') {
      clearScreens(); // undo last fire = clear screens
    }
  }
  onMount(() => {
    loadTemplates();
    window.addEventListener('keydown', onKey);
  });
  onDestroy(() => window.removeEventListener('keydown', onKey));

  async function openMainOutput() {
    const id = $templates.find((t) => t.name === 'Classic Serif')?.id ?? $templates[0]?.id;
    if (!id) return;
    try {
      await openOutput(id, 'Main screen');
    } catch {
      /* backend absent */
    }
  }

  let manualRef = '';
  let manualError = '';
  async function fireManual() {
    const ref = manualRef.trim();
    if (!ref) return;
    try {
      await manualFire(ref);
      manualRef = '';
      manualError = '';
    } catch (e) {
      manualError = String(e);
    }
  }

  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

  const channels = [
    { name: 'Main screen',   chip: 'var(--amber)',  badge: 'HDMI',       prev: 'prev-main' },
    { name: 'Stage display', chip: 'var(--teal)',   badge: 'NDI',        prev: 'prev-stage' },
    { name: 'Streaming',     chip: 'var(--violet)', badge: 'OBS',        prev: 'prev-stream' },
    { name: 'Lobby screen',  chip: 'var(--rose)',   badge: 'Kiosk',      prev: 'prev-lobby' },
  ];
</script>

<div class="layout">
  <div class="panel">
    <div class="panel-title">
      Live transcript
      {#if !$capture.stt.loaded}<span class="count">no model</span>
      {:else if $capture.capturing}<span class="count">listening · {$capture.detectedLang ?? '…'}</span>{/if}
    </div>
    <div class="transcript" bind:this={transcriptEl} style="height:132px; overflow-y:auto;">
      {#if hasTranscript}
        {$transcript.finals.join(' ')}
        {#if $transcript.partial}<mark>{$transcript.partial}</mark>{/if}
      {:else if $capture.capturing}
        <span style="color:var(--text-faint);">Waiting for speech…</span>
      {:else if !$capture.stt.loaded}
        <span style="color:var(--text-faint);">No speech model loaded — see Settings. Manual override still works.</span>
      {:else}
        <span style="color:var(--text-faint);">Start listening in Settings to transcribe live.</span>
      {/if}
    </div>

    <!-- Now live: what's currently on the screens -->
    <div class="panel-title" style="margin-top:16px;">Now live</div>
    {#if $live}
      <div class="detect-card is-live">
        <div class="detect-top">
          <div class="detect-ref">{$live.reference}{$live.translation ? ' · ' + $live.translation : ''}</div>
          <button class="btn-ghost" on:click={clearScreens}>Clear</button>
        </div>
        {#if $live.text}<div class="detect-verse">"{$live.text}"</div>{/if}
      </div>
    {:else}
      <div style="color:var(--text-faint); font-size:13px; padding:2px 0 4px;">Nothing on the screens.</div>
    {/if}

    <!-- Suggestions awaiting a decision (NOT recents) -->
    <div class="panel-title" style="margin-top:16px;">
      Suggestions
      {#if !$capture.detectionOn}<span class="count" style="color:var(--red);">detection off</span>
      {:else}<span class="count">{dets.length} pending</span>{/if}
    </div>
    <div class="detect-list">
      {#if dets.length}
        {#each dets as d (d.reference + d.at)}
          <div class="detect-card is-suggest">
            <div class="detect-top">
              <div class="detect-ref">{d.reference}</div>
              <div class="detect-tag">{d.method}</div>
            </div>
            <div class="detect-meter"><i style="width:{Math.round(d.confidence * 100)}%;"></i></div>
            <div class="detect-bottom">
              <span class="detect-conf">{d.confidence.toFixed(2)}{d.in_library ? '' : ' · not in library'}</span>
              <span style="display:flex; gap:6px;">
                <button class="btn-ghost" on:click={() => dismissDetection(d.reference)}>Dismiss</button>
                <button class="btn-confirm" on:click={() => confirmDetection(d.reference)}>Confirm</button>
              </span>
            </div>
            {#if d.text}<div class="detect-verse">"{d.text}"</div>{/if}
          </div>
        {/each}
      {:else}
        <div style="color:var(--text-faint); font-size:13px; padding:2px 0;">No pending suggestions.</div>
      {/if}
    </div>

    <div class="panel-title" style="margin-top:16px; margin-bottom:8px;">Manual override</div>
    <input
      class="search-input"
      type="text"
      bind:this={searchEl}
      bind:value={manualRef}
      on:keydown={(e) => e.key === 'Enter' && fireManual()}
      placeholder="Type a reference, e.g. John 3:16 — Enter to fire"
      disabled={!$capture.available}
    />
    {#if manualError}<div style="color:var(--red); font-size:11px; margin-top:6px;">{manualError}</div>{/if}
    <div class="kbd-row">
      <div class="kbd-hint"><span class="kbd">Space</span> confirm top suggestion</div>
      <div class="kbd-hint"><span class="kbd">Esc</span> clear all screens</div>
      <div class="kbd-hint"><span class="kbd">/</span> focus search</div>
      <div class="kbd-hint"><span class="kbd">Z</span> clear</div>
    </div>
  </div>

  <div>
    <div class="channels">
      {#each channels as c}
        <div class="channel">
          <div class="channel-head">
            <div class="channel-name"><span class="chip" style="background:{c.chip};"></span>{c.name}</div>
            <div class="channel-badge">{c.badge}</div>
          </div>
          <div class="channel-preview {c.prev}">
            {#if $live}
              {#if c.prev === 'prev-main'}
                <div><div class="verse">"{$live.text}"</div><div class="ref">{$live.reference}{$live.translation ? ' · ' + $live.translation : ''}</div></div>
              {:else if c.prev === 'prev-stage'}
                <div class="timer">● LIVE</div><div class="verse">{$live.reference}<br />"{$live.text}"</div>
              {:else if c.prev === 'prev-stream'}
                <div class="lower-third"><div class="ref">{$live.reference}</div><div class="verse">"{$live.text}"</div></div>
              {:else}
                <div class="mark">{$live.reference}</div><div class="verse">"{$live.text}"</div>
              {/if}
            {:else}
              <div style="color:var(--text-faint); font-family:var(--f-mono); font-size:11px;">— cleared —</div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
    {#if $capture.audioError}
      <div style="background:var(--red-soft); color:var(--red); border:1px solid rgba(217,105,95,0.3); border-radius:8px; padding:8px 11px; margin-bottom:10px; font-size:12px;">
        Audio: {$capture.audioError}
      </div>
    {/if}
    <div class="controls">
      <button
        class="ctrl-btn"
        class:primary={!$capture.capturing}
        on:click={toggleListen}
        disabled={!$capture.available || !$capture.stt.loaded || listenBusy}
      >
        {#if $capture.capturing}
          <span class="dot" style="background:var(--red);"></span>Listening — Stop
        {:else}
          <span class="dot" style="background:#1b1204;"></span>{listenBusy ? 'Starting…' : 'Start listening'}
        {/if}
      </button>
      <button
        class="ctrl-btn"
        on:click={() => setDetection(!$capture.detectionOn)}
        disabled={!$capture.available}
      >
        <span class="dot" style="background:{$capture.detectionOn ? 'var(--green)' : 'var(--text-faint)'};"></span>
        AI detection: {$capture.detectionOn ? 'On' : 'Off'}
      </button>
      <button class="ctrl-btn" on:click={clearScreens} disabled={!$capture.available}>
        <span class="dot" style="background:var(--text-faint);"></span>Clear all screens
        <span style="color:var(--text-faint); font-family:var(--f-mono); font-size:10px;">Esc</span>
      </button>
      <button class="ctrl-btn" on:click={openMainOutput} disabled={!$capture.available}>Open output screen</button>
    </div>
  </div>
</div>
