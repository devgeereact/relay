<script>
  import { onMount, onDestroy } from 'svelte';
  import {
    capture,
    transcript,
    detections,
    confirmDetection,
    dismissDetection,
    manualFire,
    openOutput,
    clearScreens,
  } from '../stores/capture.js';

  // Esc = clear all screens, one of the always-reachable operator controls.
  function onKey(e) {
    if (e.key === 'Escape') clearScreens();
  }
  onMount(() => window.addEventListener('keydown', onKey));
  onDestroy(() => window.removeEventListener('keydown', onKey));

  async function openMainOutput() {
    try {
      await openOutput('main', 'Main screen');
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
  const isLive = (d) => d.status === 'auto' || d.status === 'manual';
  const tagFor = (d) => (d.status === 'manual' ? 'Manual' : d.status === 'auto' ? 'Auto-fired' : 'Suggested');

  // Transcript + detection list are LIVE (Phase 4/5): transcript from the STT
  // engine, detections from direct-match over `detection://match`. Channel
  // previews are still static demo until the router/template phases.
  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

  // Confidence styling previews the Phase 6 gate: ≥0.90 reads as live/high,
  // below as a suggestion. Real auto-fire vs suggest gating lands in the router.
  const pct = (c) => Math.round(c * 100);

  // Operator override (search box) is a first-class control per CLAUDE.md —
  // it stays here at the top of the console, always reachable, never a fallback.

  // Shown only when no live detections yet — keeps the console legible at rest.
  const demoDetections = [
    { ref: 'John 3:16',   state: 'is-live',    tag: 'Auto-fired', pct: 96, conf: '0.96 · direct match',  action: 'undo' },
    { ref: 'Romans 8:28', state: 'is-suggest', tag: 'Suggested',  pct: 71, conf: '0.71 · paraphrase',    action: 'confirm' },
  ];

  const channels = [
    { name: 'Main screen',   chip: 'var(--amber)',  badge: 'HDMI',       prev: 'prev-main' },
    { name: 'Stage display', chip: 'var(--teal)',   badge: 'NDI',        prev: 'prev-stage' },
    { name: 'Streaming',     chip: 'var(--violet)', badge: 'NDI → OBS',  prev: 'prev-stream' },
    { name: 'Lobby screen',  chip: 'var(--rose)',   badge: 'Kiosk',      prev: 'prev-lobby' },
  ];
</script>

<div class="layout">
  <div class="panel">
    <div class="panel-title">
      Live transcript
      {#if !$capture.stt.loaded}<span class="count">no model</span>
      {:else if $capture.capturing}<span class="count">listening</span>{/if}
    </div>
    <div class="transcript">
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

    <div class="panel-title" style="margin-top:16px;">
      AI detection
      {#if $detections.length}<span class="count">{$detections.length} recent</span>
      {:else}<span class="count">2 demo</span>{/if}
    </div>
    <div class="detect-list">
      {#if $detections.length}
        {#each $detections as d (d.reference + d.at)}
          <div class="detect-card {isLive(d) ? 'is-live' : 'is-suggest'}">
            <div class="detect-top">
              <div class="detect-ref">{d.reference}</div>
              <div class="detect-tag">{tagFor(d)}</div>
            </div>
            <div class="detect-meter"><i style="width:{pct(d.confidence)}%;"></i></div>
            <div class="detect-bottom">
              <span class="detect-conf">{d.confidence.toFixed(2)} · {d.method}{d.in_library ? '' : ' · not in library'}</span>
              {#if isLive(d)}
                <button class="btn-ghost" on:click={() => dismissDetection(d.reference)}>Undo</button>
              {:else}
                <button class="btn-confirm" on:click={() => confirmDetection(d.reference)}>Confirm</button>
              {/if}
            </div>
            {#if d.text}
              <div class="detect-verse">"{d.text}"</div>
            {/if}
          </div>
        {/each}
      {:else}
        {#each demoDetections as d}
          <div class="detect-card {d.state}">
            <div class="detect-top"><div class="detect-ref">{d.ref}</div><div class="detect-tag">{d.tag}</div></div>
            <div class="detect-meter"><i style="width:{d.pct}%;"></i></div>
            <div class="detect-bottom">
              <span class="detect-conf">{d.conf}</span>
              {#if d.action === 'confirm'}
                <button class="btn-confirm">Confirm</button>
              {:else}
                <button class="btn-ghost">Undo</button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <div class="panel-title" style="margin-top:16px; margin-bottom:8px;">Manual override</div>
    <input
      class="search-input"
      type="text"
      bind:value={manualRef}
      on:keydown={(e) => e.key === 'Enter' && fireManual()}
      placeholder="Type a reference, e.g. John 3:16 — Enter to fire"
      disabled={!$capture.available}
    />
    {#if manualError}<div style="color:var(--red); font-size:11px; margin-top:6px;">{manualError}</div>{/if}
    <div class="kbd-row">
      <div class="kbd-hint"><span class="kbd">Space</span> confirm suggestion</div>
      <div class="kbd-hint"><span class="kbd">Esc</span> clear all screens</div>
      <div class="kbd-hint"><span class="kbd">/</span> focus search</div>
      <div class="kbd-hint"><span class="kbd">Z</span> undo last fire</div>
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
            {#if c.prev === 'prev-main'}
              <div><div class="verse">"For God so loved the world, that He gave His only begotten Son…"</div><div class="ref">John 3:16 · KJV</div></div>
            {:else if c.prev === 'prev-stage'}
              <div class="timer">SERMON · 24:10</div><div class="verse">John 3:16<br />"For God so loved…"</div>
            {:else if c.prev === 'prev-stream'}
              <div class="lower-third"><div class="verse">"For God so loved the world…"</div></div>
            {:else}
              <div class="mark">Grace Chapel</div><div class="verse">"For God so loved the world…"</div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
    <div class="controls">
      <button class="ctrl-btn primary"><span class="dot" style="background:#1b1204;"></span>AI detection: On</button>
      <button class="ctrl-btn" on:click={clearScreens} disabled={!$capture.available}><span class="dot" style="background:var(--text-faint);"></span>Clear all screens <span style="color:var(--text-faint); font-family:var(--f-mono); font-size:10px;">Esc</span></button>
      <button class="ctrl-btn" on:click={openMainOutput} disabled={!$capture.available}>Open output screen</button>
    </div>
  </div>
</div>
